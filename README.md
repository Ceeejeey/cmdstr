# cmdstr

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**cmdstr** is a smart command storage and recall tool for the terminal. It automatically captures every command you run, stores it in a local SQLite database, and lets you search, tag, annotate, bookmark, and re-run commands with ease. It also ships with a hacker-themed interactive TUI for browsing and managing your command history.

Never lose that complicated `ffmpeg` incantation or that perfect `docker` command again.

---

## Features

- **Auto-capture** — every command, exit code, duration, working directory, and hostname
- **Full-text search** — fuzzy search commands, tags, and notes
- **Tagging** — organise commands with tags (`cmdstr tag <id> docker,ffmpeg`)
- **Annotations** — add notes or bookmark favourites
- **Statistics** — total commands, unique commands, failure rate, top tags, most frequent commands
- **Export** — JSON or CSV export of your entire history
- **Direct execution** — tag a command and run it later with `cmdstr run <tag>` or just `cmdstr <tag>`
- **Interactive TUI** — hacker-themed terminal UI to browse, search, run, tag, and manage commands
- **Shell integration** — bash, zsh, and fish hooks for automatic capture (install via `cmdstr install`)
- **XDG-compliant** — data stored at `$XDG_DATA_HOME/cmdstr/commands.db`

---

## Installation

### From PPA (Ubuntu / Debian)

```bash
sudo add-apt-repository ppa:ceeejeey/cmdstr
sudo apt update
sudo apt install cmdstr
```

### From source

```bash
git clone https://github.com/ceeejeey/cmdstr.git
cd cmdstr
cargo build --release
sudo cp target/release/cmdstr /usr/local/bin/
```

### Install shell hooks (recommended)

```bash
cmdstr install
```

This auto-detects your shell and adds the capture hook to `~/.bashrc`, `~/.zshrc`, or `~/.config/fish/config.fish`. Restart your shell or source the file to start capturing.

---

## Quick Start

```bash
# Show help
cmdstr --help

# Search your command history
cmdstr search docker

# Tag a command (find its ID via search, then tag it)
cmdstr search nginx
# -> 01ARZ3N... │ ✓ 2026-05-17T12:00:00 │ sudo systemctl start nginx
cmdstr tag 01ARZ3N nginx,systemd

# Run a tagged command
cmdstr run nginx
# Or just:
cmdstr nginx

# Launch the interactive TUI
cmdstr tui

# See your stats
cmdstr stats
```

---

## Commands

### `capture` — Capture a command

Called automatically by shell hooks. Records a command, its exit code, duration, working directory, and session ID.

```bash
cmdstr capture "docker ps -a" 0 234 /home/user abc123-session
```

### `add` — Manually add a command

```bash
cmdstr add "sudo systemctl restart nginx" --tag nginx,systemd --note "Restart after config change"
cmdstr add "curl -s https://api.example.com" --tag api,testing
```

| Flag | Description |
|------|-------------|
| `-t, --tag` | Comma-separated tags |
| `-n, --note` | Annotation note |
| `--exit-code` | Exit code (default: 0) |
| `--duration` | Duration in ms (default: 0) |

### `search` / `list` — Search and list commands

```bash
# Basic search
cmdstr search docker
cmdstr list docker

# Filter by tag
cmdstr search --tag nginx

# Last 24 hours
cmdstr search --last 24

# Only failed commands
cmdstr search --failed

# Most frequent commands
cmdstr search --freq

# Bookmarked commands only
cmdstr search --bookmarks

# JSON output
cmdstr search docker --json

# Limit results
cmdstr search docker --limit 10
```

| Flag | Description |
|------|-------------|
| `query` | Search term (fuzzy match) |
| `-t, --tag` | Filter by tag name |
| `--last` | Only commands from last N hours |
| `--failed` | Only failed commands (exit code ≠ 0) |
| `--freq` | Show most frequent commands |
| `--bookmarks` | Bookmarked commands only |
| `-l, --limit` | Max results (default: 30) |
| `--json` | JSON output |

### `tag` — Tag a stored command

```bash
cmdstr tag <command-id> docker,compose,dev
```

Tags are **comma-separated** and stored in lowercase.

### `annotate` / `note` — Add a note or bookmark

```bash
cmdstr annotate <command-id> "Restart the web server after config changes"
cmdstr note <command-id> "My favourite command" --bookmark
```

| Flag | Description |
|------|-------------|
| `--bookmark` | Also mark as bookmark |

### `stats` — Show statistics

```bash
cmdstr stats
cmdstr stats --json
```

Displays:
- Total commands captured
- Unique commands
- Bookmarks
- Failure rate
- Commands today
- Top tags
- Most frequent commands

### `install` — Install shell hooks

```bash
# Auto-detect shell
cmdstr install

# Specify shell
cmdstr install --shell zsh

# Custom binary path
cmdstr install --bin-path /usr/local/bin/cmdstr
```

| Flag | Description |
|------|-------------|
| `-s, --shell` | Shell type: `bash`, `zsh`, or `fish` |
| `--bin-path` | Custom path to cmdstr binary |

### `export` — Export to JSON or CSV

```bash
# JSON to stdout
cmdstr export

# CSV to file
cmdstr export --format csv --output history.csv

# JSON to file
cmdstr export --output history.json
```

| Flag | Description |
|------|-------------|
| `-f, --format` | Output format: `json` or `csv` (default: json) |
| `-o, --output` | Output file path (stdout if not set) |

### `run` — Run a tagged command

```bash
cmdstr run docker
```

Looks up the most recent command tagged with `docker` and executes it via `sh -c`.

**Shorthand:** you can omit `run` and just pass the tag directly:

```bash
cmdstr docker
```

### `tui` / `dashboard` — Interactive TUI

```bash
cmdstr tui
```

Launches a hacker-themed terminal UI for browsing, searching, running, tagging, and managing your command history interactively.

The TUI shows a split view: a **command list** on top with `▸` pointer, `✓`/`✗` status, and `★` bookmark marker, and an **output panel** below displaying captured command output with a scroll bar, alternating line colours, and a `5:42`-style line counter.

#### Navigation & Selection

| Key | Action |
|-----|--------|
| `↑` / `↓` or `j` / `k` | Navigate command list |
| `g` | Go to top of list |
| `G` | Go to bottom of list |
| `Ctrl+↑` / `Ctrl+↓` | Scroll output up / down |
| `PageUp` / `PageDown` | Scroll output by 5 lines |
| `?` / `F1` | Show help popup (dismiss with `Esc`) |

The help bar at the bottom shows a compact summary of available actions.

#### Running Commands

| Key | Action |
|-----|--------|
| `Enter` | Run the selected command |
| `r` | Type and run an arbitrary command |
| `S` | Toggle **sudo mode** (red theme, all commands prefixed with `sudo`) |

Commands run in a real PTY via `script(1)` — interactive prompts (like `apt upgrade`) work fully, and all output is captured and displayed in the output panel after the command finishes.

#### Sudo & Password Popup

When sudo mode is toggled (`S`), the UI switches to a **red-on-black hacker theme**. Pressing `Enter` on a command while in sudo mode:

1. Shows a password popup with masked input (`••••`)
2. **Caches sudo credentials** via `sudo -S -v` (password sent over a secure pipe)
3. Runs the command with full `sudo` privileges
4. Captures the output for display

#### Managing Commands

| Key | Action |
|-----|--------|
| `/` or `s` | Search / filter commands |
| `t` | Tag the selected command |
| `n` / `a` | Add a note to the selected command |
| `b` | Toggle bookmark (`★`) |
| `d` | Delete the selected command |
| `w` | Manually add a command to the store (capture without running) |

All actions save immediately to the SQLite database.

#### Quitting

| Key | Action |
|-----|--------|
| `q` | Quit the TUI |
| `Ctrl+C` | Quit from any mode |
| `Esc` | Cancel current input / close popup |

#### How Command Execution Works

The TUI suspends itself before running external commands, with visual transition cues:

1. **Brief in-TUI status** — shows `Running: <cmd>` for 100ms so you see what's about to run
2. **Suspends the TUI** — leaves alternate screen, disables raw mode, shows cursor
3. **Transition banner** — displays a green `━━━ cmdstr ── Running: <cmd> ───` banner on the main screen
4. **Executes via `script(1)`** — the command runs inside a real PTY so interactive apps (`apt`, `vim`, `htop`) work fully, while output is captured to a temp file
5. **Post-command cleanup** — `stty sane` restores any terminal state changes the command may have left
6. **Done banner** — shows `━━━ cmdstr ── Done (exit: N) ───` for at least 300ms so fast commands don't blink past
7. **Resumes the TUI** — re-enters alternate screen, re-enables raw mode, hides cursor
8. **Displays captured output** — ANSI escape codes are stripped, `\r` / `\r\n` line endings are normalised, and `script(1)` recording headers/footers are removed

This approach handles everything from `echo hello` to `apt upgrade` and `vim` without crashing or corrupting the TUI state.

---

## Shell Integration

### Manual setup

If you prefer not to use `cmdstr install`, source the hook scripts directly:

**Bash:**
```bash
source /path/to/cmdstr/shell/bash.sh
```

**Zsh:**
```zsh
source /path/to/cmdstr/shell/zsh.sh
```

**Fish:**
```fish
source /path/to/cmdstr/shell/fish.fish
```

### How it works

The shell hooks intercept every command via `preexec`/`precmd` (bash/zsh) or `fish_preexec`/`fish_postexec` events. They measure:
- The exact command string
- Start and end timestamps (nanosecond precision)
- Exit code
- Current working directory
- A per-session unique ID

This data is passed to `cmdstr capture` which stores it in the SQLite database. The hook runs **asynchronously** (`2>/dev/null || true`) so it never blocks or interferes with your terminal.

---

## Data Storage

cmdstr follows the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/latest/):

| Path | Purpose |
|------|---------|
| `$XDG_DATA_HOME/cmdstr/commands.db` | SQLite database |
| `$XDG_CONFIG_HOME/cmdstr/config.toml` | Configuration (future use) |

Defaults:
- `$XDG_DATA_HOME` → `~/.local/share/`
- `$XDG_CONFIG_HOME` → `~/.config/`

### Database schema

```
commands       — id (ULID), command, cwd, exit_code, duration_ms, session_id, hostname, captured_at
tags           — id, name (unique)
command_tags   — command_id → commands.id, tag_id → tags.id
annotations    — command_id → commands.id, note, is_bookmark
command_freq   — command_hash, command, count
```

---

## Use Cases

### Recover that one command you ran yesterday

```bash
cmdstr search --last 48
```

### Organise complex one-liners

```bash
cmdstr search ffmpeg
cmdstr tag <id> ffmpeg,video,convert
# Later:
cmdstr ffmpeg
```

### Debug recurring failures

```bash
cmdstr search --failed
```

### Track your most-used tools

```bash
cmdstr search --freq
```

### Interactive exploration

```bash
cmdstr tui
```

---

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Debian package (requires cargo-deb)
cargo install cargo-deb
cargo deb

# For PPA upload (requires devscripts and dh-cargo)
# Install build dependencies:
sudo apt install devscripts debhelper dh-cargo cargo rustc

# Build source package:
debuild -S -sa
```

---

## Uninstall

```bash
# Remove the binary
sudo rm /usr/local/bin/cmdstr

# Remove the database and config
rm -rf ~/.local/share/cmdstr
rm -rf ~/.config/cmdstr

# Remove shell hooks (edit your .bashrc/.zshrc/config.fish and delete the cmdstr block)
```

---

## Licence

MIT — see [LICENCE](LICENSE).
