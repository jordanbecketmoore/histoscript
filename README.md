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

## Enable In-Memory History
`histoscript` reads your shell's history from its history file, which may not contain the in-memory history of your current shell. To use `histoscript` to write scripts from your current terminal session, you'll need to enable history auto-flushing in your shell

### bash 
Bash accumulates history in memory and writes it to ~/.bash_history on exit. To make it append after every command, add the following to your ~/.bashrc:

```bash 
PROMPT_COMMAND='history -a'
```

`PROMPT_COMMAND` is a special variable that Bash runs before displaying each prompt. Setting it to history -a ("append") ensures every command is written to disk immediately after it runs.

If you already have a PROMPT_COMMAND set, append to it rather than replacing it:

```bash
PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND; }history -a"
```

### zsh 
Zsh provides a built-in option for this. Add the following to your ~/.zshrc:
```zsh
# Location of history file
HISTFILE=~/.zsh_history
# Maximum lines kept in memory
export HISTSIZE=100000
# Maximum lines saved to $HISTFILE
export SAVEHIST=100000
# Like INC_APPEND_HISTORY + re-read history whenever accessing it
setopt SHARE_HISTORY
```

### fish 

Fish shell already writes history to disk incrementally by default — no configuration is needed. Every command is saved to ~/.local/share/fish/fish_history immediately after it runs.

If for some reason history saving has been disabled in your Fish config, you can re-enable it by removing any builtin history clear or set -U fish_history overrides from ~/.config/fish/config.fish, or by running:

```fish
set -e fish_history_max  # reset to default if overridden
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