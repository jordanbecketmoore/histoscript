use std::env;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::DefaultTerminal;

// Returns the basename of $SHELL (e.g. "bash", "zsh", "fish").
fn detect_shell() -> String {
    env::var("SHELL")
        .unwrap_or_default()
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

// Resolves the history file path for the given shell.
// Respects $HISTFILE for bash/zsh; falls back to well-known defaults.
fn history_file_path(shell: &str) -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let home = PathBuf::from(home);

    match shell {
        "bash" => {
            if let Ok(histfile) = env::var("HISTFILE") {
                Some(PathBuf::from(histfile))
            } else {
                Some(home.join(".bash_history"))
            }
        }
        "zsh" => {
            if let Ok(histfile) = env::var("HISTFILE") {
                Some(PathBuf::from(histfile))
            } else {
                Some(home.join(".zsh_history"))
            }
        }
        "fish" => Some(home.join(".local/share/fish/fish_history")),
        _ => None,
    }
}

// Parses raw history file contents into a list of command strings,
// stripping shell-specific metadata (zsh timestamps, fish YAML fields).
fn parse_history_lines(history: &str, shell: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for line in history.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match shell {
            "zsh" => {
                // Zsh extended history format: ": timestamp:0;command"
                if let Some(rest) = trimmed.strip_prefix(": ") {
                    if let Some(idx) = rest.find(';') {
                        lines.push(rest[idx + 1..].to_string());
                        continue;
                    }
                }
            }
            "fish" => {
                // Fish history format: "- cmd: command" entries
                if let Some(cmd) = trimmed.strip_prefix("- cmd: ") {
                    lines.push(cmd.to_string());
                    continue;
                }
                // Skip other fish history metadata lines (when:, paths:, etc.)
                if trimmed.starts_with("when: ") || trimmed.starts_with("paths:") || trimmed.starts_with("- ") {
                    continue;
                }
            }
            _ => {}
        }
        lines.push(trimmed.to_string());
    }
    lines
}

// Returns the shebang line for the given shell name.
fn shebang_for_shell(shell: &str) -> &str {
    match shell {
        "sh" => "#!/bin/sh",
        "zsh" => "#!/bin/zsh",
        "fish" => "#!/usr/bin/env fish",
        _ => "#!/bin/bash",
    }
}

// Holds all TUI state: the history lines, per-line selection flags,
// the cursor position, and the current scroll offset.
struct App {
    lines: Vec<String>,
    selected: Vec<bool>,
    cursor: usize,
    scroll_offset: usize,
}

impl App {
    // Creates a new App with the cursor starting at the most recent (bottom) entry.
    fn new(lines: Vec<String>) -> Self {
        let len = lines.len();
        App {
            selected: vec![false; len],
            cursor: if len > 0 { len - 1 } else { 0 },
            scroll_offset: 0,
            lines,
        }
    }

    fn move_cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_cursor_down(&mut self) {
        if self.cursor + 1 < self.lines.len() {
            self.cursor += 1;
        }
    }

    fn toggle_selection(&mut self) {
        if !self.lines.is_empty() {
            self.selected[self.cursor] = !self.selected[self.cursor];
        }
    }

    fn page_up(&mut self, page_size: usize) {
        self.cursor = self.cursor.saturating_sub(page_size);
    }

    fn page_down(&mut self, page_size: usize) {
        if !self.lines.is_empty() {
            self.cursor = (self.cursor + page_size).min(self.lines.len() - 1);
        }
    }

    fn jump_to_top(&mut self) {
        self.cursor = 0;
    }

    fn jump_to_bottom(&mut self) {
        if !self.lines.is_empty() {
            self.cursor = self.lines.len() - 1;
        }
    }

    // Returns the subset of lines that have been checked, in order.
    fn selected_lines(&self) -> Vec<&str> {
        self.lines
            .iter()
            .zip(self.selected.iter())
            .filter(|(_, sel)| **sel)
            .map(|(line, _)| line.as_str())
            .collect()
    }

    // Adjusts scroll_offset so the cursor row is always within the visible window.
    fn ensure_cursor_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + visible_height {
            self.scroll_offset = self.cursor - visible_height + 1;
        }
    }
}

// Main event loop. Draws the history list and status bar, handles keyboard input.
// Returns Ok(true) when the user confirms (Enter), Ok(false) when they quit.
fn run(terminal: &mut DefaultTerminal, app: &mut App, execute_mode: bool) -> io::Result<bool> {
    // Scroll to the bottom so the most recent history is visible on launch.
    let size = terminal.size()?;
    let visible_height = size.height.saturating_sub(4) as usize; // borders + status bar
    if app.lines.len() > visible_height {
        app.scroll_offset = app.lines.len() - visible_height;
    }

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            // Split the screen: scrollable list on top, one-line status bar on the bottom.
            let chunks = Layout::vertical([
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

            let list_area = chunks[0];
            let inner_height = list_area.height.saturating_sub(2) as usize; // block borders

            app.ensure_cursor_visible(inner_height);

            // Build the visible slice of lines as styled ratatui spans.
            let visible_lines: Vec<Line> = (app.scroll_offset
                ..app.lines.len().min(app.scroll_offset + inner_height))
                .map(|i| {
                    let checkbox = if app.selected[i] { "[x] " } else { "[ ] " };
                    let is_cursor = i == app.cursor;

                    // Cursor + selected: dark background, green, bold.
                    // Cursor only: dark background, bold.
                    // Selected only: green.
                    // Neither: default.
                    let style = if is_cursor && app.selected[i] {
                        Style::default()
                            .bg(Color::DarkGray)
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else if is_cursor {
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    } else if app.selected[i] {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default()
                    };

                    Line::from(vec![
                        Span::styled(checkbox, style),
                        Span::styled(&app.lines[i], style),
                    ])
                })
                .collect();

            let selected_count = app.selected.iter().filter(|&&s| s).count();
            let title = format!(
                " History ({}/{} selected) ",
                selected_count,
                app.lines.len()
            );
            let list_widget = Paragraph::new(visible_lines)
                .block(Block::default().borders(Borders::ALL).title(title));

            frame.render_widget(list_widget, list_area);

            // Status bar: key binding hints, adapts label based on execute_mode.
            let status = Line::from(vec![
                Span::styled(
                    " Space",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(": toggle  "),
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(if execute_mode { ": run commands  " } else { ": write script  " }),
                Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("/"),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": quit  "),
                Span::styled("j/k", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": navigate"),
            ]);
            let status_widget =
                Paragraph::new(status).style(Style::default().bg(Color::DarkGray));
            frame.render_widget(status_widget, chunks[1]);
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
                KeyCode::Up | KeyCode::Char('k') => app.move_cursor_up(),
                KeyCode::Down | KeyCode::Char('j') => app.move_cursor_down(),
                KeyCode::Char(' ') => app.toggle_selection(),
                KeyCode::Enter => return Ok(true),
                KeyCode::PageUp => {
                    let size = terminal.size()?;
                    let page = size.height.saturating_sub(4) as usize;
                    app.page_up(page);
                }
                KeyCode::PageDown => {
                    let size = terminal.size()?;
                    let page = size.height.saturating_sub(4) as usize;
                    app.page_down(page);
                }
                KeyCode::Home => app.jump_to_top(),
                KeyCode::End => app.jump_to_bottom(),
                _ => {}
            }
        }
    }
}

fn print_help(program: &str) {
    println!(
        "\
histoscript — turn shell history into a script

Browse your shell history in an interactive TUI, select the commands
you want, and save them as an executable shell script with the correct
shebang line. Supports bash, zsh, and fish.

USAGE:
    {program} [OPTIONS] <output-script>
    {program} -x

ARGS:
    <output-script>    Path for the generated shell script (not used with -x)

OPTIONS:
    -s, --shell <shell>      Override the output script shell (sh, bash, zsh, fish)
    -m, --make-executable    chmod +x the output script after writing
    -x, --execute            Run selected commands immediately instead of writing a script"
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2
        || args[1] == "-h"
        || args[1] == "--help"
    {
        print_help(&args[0]);
        process::exit(if args.len() < 2 { 1 } else { 0 });
    }
    if args[1] == "-V" || args[1] == "--version" {
        println!("histoscript {}", env!("CARGO_PKG_VERSION"));
        process::exit(0);
    }

    let make_executable = args.iter().any(|a| a == "--make-executable" || a == "-m");
    let execute_mode = args.iter().any(|a| a == "--execute" || a == "-x");

    // Parse -s/--shell <value> or --shell=<value>.
    let shell_override: Option<String> = {
        let mut val = None;
        let mut iter = args.iter().skip(1);
        while let Some(arg) = iter.next() {
            if arg == "-s" || arg == "--shell" {
                val = iter.next().cloned();
                break;
            } else if let Some(v) = arg.strip_prefix("--shell=") {
                val = Some(v.to_string());
                break;
            }
        }
        val
    };
    if let Some(ref s) = shell_override {
        match s.as_str() {
            "sh" | "bash" | "zsh" | "fish" => {}
            other => {
                eprintln!("Error: unsupported shell '{other}' for -s/--shell (supported: sh, bash, zsh, fish)");
                process::exit(1);
            }
        }
    }

    // Find the first non-flag, non-option-value positional argument as the output path.
    let output_path = {
        let mut val = None;
        let mut iter = args.iter().skip(1).peekable();
        while let Some(arg) = iter.next() {
            if arg == "-s" || arg == "--shell" {
                iter.next(); // skip the shell value
                continue;
            }
            if arg.starts_with("--shell=") || arg.starts_with('-') {
                continue;
            }
            val = Some(arg);
            break;
        }
        val
    };

    if !execute_mode && output_path.is_none() {
        eprintln!("Error: missing <output-script> argument (or use -x to run commands directly)");
        process::exit(1);
    }

    let shell = detect_shell();
    if shell.is_empty() {
        eprintln!("Error: could not detect shell (SHELL environment variable not set)");
        process::exit(1);
    }

    let history_path = match history_file_path(&shell) {
        Some(p) => p,
        None => {
            eprintln!("Error: unsupported shell '{shell}' (supported: bash, zsh, fish)");
            process::exit(1);
        }
    };

    let history = match fs::read_to_string(&history_path) {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!(
                "Error: could not read history file '{}': {e}",
                history_path.display()
            );
            process::exit(1);
        }
    };

    let lines = parse_history_lines(&history, &shell);
    if lines.is_empty() {
        eprintln!("Error: no history lines found");
        process::exit(1);
    }

    let mut app = App::new(lines);

    // Setup terminal: raw mode + alternate screen so we don't clobber the user's shell.
    terminal::enable_raw_mode().expect("failed to enable raw mode");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).expect("failed to enter alternate screen");
    let mut terminal = ratatui::init();

    let result = run(&mut terminal, &mut app, execute_mode);

    // Restore the terminal before processing the result.
    ratatui::restore();
    execute!(io::stdout(), LeaveAlternateScreen).ok();
    terminal::disable_raw_mode().ok();

    match result {
        Ok(true) => {
            let selected = app.selected_lines();
            if selected.is_empty() {
                println!("No lines selected, nothing done.");
            } else if execute_mode {
                // Run each selected command through the user's shell.
                for cmd in &selected {
                    let status = process::Command::new(&shell)
                        .arg("-c")
                        .arg(cmd)
                        .status();
                    match status {
                        Ok(s) if !s.success() => {
                            eprintln!("Command exited with status {s}: {cmd}");
                        }
                        Err(e) => {
                            eprintln!("Failed to run command '{cmd}': {e}");
                        }
                        _ => {}
                    }
                }
            } else {
                // Write selected commands to the output file with the appropriate shebang.
                let output_path = output_path.unwrap();
                let output_shell = shell_override.as_deref().unwrap_or(&shell);
                let shebang = shebang_for_shell(output_shell);
                let mut content = String::from(shebang);
                content.push('\n');
                for line in &selected {
                    content.push_str(line);
                    content.push('\n');
                }
                match fs::write(output_path, &content) {
                    Ok(()) => {
                        if make_executable {
                            match fs::metadata(output_path) {
                                Ok(meta) => {
                                    let mut perms = meta.permissions();
                                    perms.set_mode(perms.mode() | 0o111);
                                    if let Err(e) = fs::set_permissions(output_path, perms) {
                                        eprintln!("Error setting permissions on {output_path}: {e}");
                                        process::exit(1);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error reading metadata for {output_path}: {e}");
                                    process::exit(1);
                                }
                            }
                        }
                        println!("Wrote {} lines to {}", selected.len(), output_path);
                    }
                    Err(e) => {
                        eprintln!("Error writing to {output_path}: {e}");
                        process::exit(1);
                    }
                }
            }
        }
        Ok(false) => {
            println!("Quit without writing.");
        }
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}
