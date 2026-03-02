# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

histoscript — a Rust TUI tool that lets users interactively select commands from their shell history and save them as a shell script with the correct shebang. Supports bash, zsh, and fish.

## Build & Run

```bash
cargo build              # debug build
cargo build --release    # release build
cargo run -- output.sh   # run with output file argument
cargo check              # type-check without building
cargo clippy             # lint
```

Requires Rust edition 2024.

## Architecture

Single-file application (`src/main.rs`) using ratatui + crossterm for the terminal UI.

Key components:
- **Shell detection & history parsing**: `detect_shell()` reads `$SHELL`, `history_file_path()` resolves the history file, `parse_history_lines()` handles shell-specific formats (zsh extended history `: timestamp:0;cmd`, fish `- cmd:` entries)
- **`App` struct**: holds history lines, selection state (`Vec<bool>`), cursor position, and scroll offset. Cursor starts at the bottom (most recent history).
- **`run()` function**: main event loop — renders a `Paragraph` widget with checkbox-style line items and a status bar, handles vim-style (`j`/`k`) and arrow key navigation
- **Output**: writes selected lines to the output file with shell-appropriate shebang from `shebang_for_shell()`
