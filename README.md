# histoscript

A terminal UI for interactively building shell scripts from your shell history.

Browse your history, select the commands you want, and save them as a ready-to-run script — complete with the correct shebang for your shell.

## Usage

```
histoscript <output-script>
```

This opens an interactive TUI displaying your shell history. Select the lines you want, then press Enter to write them to the output file.

Example:

```
histoscript deploy.sh
```

## Key Bindings

| Key | Action |
|---|---|
| `j` / `Down` | Move cursor down |
| `k` / `Up` | Move cursor up |
| `Space` | Toggle selection on current line |
| `Enter` | Write selected lines to output file and exit |
| `q` / `Esc` | Quit without writing |
| `Page Up` / `Page Down` | Scroll by page |
| `Home` / `End` | Jump to top / bottom |

## Supported Shells

- **bash** — reads `$HISTFILE` or `~/.bash_history`
- **zsh** — reads `$HISTFILE` or `~/.zsh_history` (handles extended history format)
- **fish** — reads `~/.local/share/fish/fish_history`

The output script automatically gets the appropriate shebang (`#!/bin/bash`, `#!/bin/zsh`, or `#!/usr/bin/env fish`).

## Building

Requires Rust (edition 2024).

```
cargo build --release
```
