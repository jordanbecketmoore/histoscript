use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

fn detect_shell() -> String {
    // Get the parent process's shell from the SHELL environment variable
    env::var("SHELL")
        .unwrap_or_default()
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

fn history_file_path(shell: &str) -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let home = PathBuf::from(home);

    match shell {
        "bash" => {
            // Prefer HISTFILE if set, otherwise default
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

fn main() {
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
            eprintln!("Error: could not read history file '{}': {e}", history_path.display());
            process::exit(1);
        }
    };

    println!("Detected shell: {shell}");
    println!("History file: {}", history_path.display());
    println!("History size: {} bytes, {} lines", history.len(), history.lines().count());
}
