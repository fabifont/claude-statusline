# claude-statusline

Minimal offline Claude Code status line formatter written in Rust.

## Features

- 5-hour rate limit percentage and remaining time
- 7-day rate limit percentage and remaining time
- Context-window usage percentage
- Peak-hours indicator with remaining time in the configured window
- Current model name
- Session cost in USD
- Ordered, configurable items through TOML
- External command items for custom segments (any language)
- ANSI color support (optional)
- Built-in preview tools for configuration and rendering

## Install

### Option 1: Cargo

```bash
cargo install --git https://github.com/fabifont/claude-statusline
```

### Option 2: GitHub release artifact

1. Download the latest Linux `x86_64` GNU archive from Releases.
2. Extract and move the binary to a location in `PATH`, for example:

```bash
tar -xzf claude-statusline-x86_64-unknown-linux-gnu.tar.gz
install -Dm755 claude-statusline ~/.local/bin/claude-statusline
```

### Option 3: Build from source

```bash
cargo build --release
install -Dm755 target/release/claude-statusline ~/.local/bin/claude-statusline
```

### PATH note

Claude Code should call `claude-statusline` directly. Ensure your install location is on `PATH`.

Examples:

- Cargo installs to `~/.cargo/bin`
- Local manual installs are often in `~/.local/bin`

## Configuration

Copy the example configuration:

```bash
mkdir -p ~/.config/claude-statusline
cp statusline.toml.example ~/.config/claude-statusline/config.toml
```

Config lookup order:

1. `CLAUDE_STATUSLINE_CONFIG`
2. `~/.claude/statusline.toml`
3. `~/.config/claude-statusline/config.toml`
4. Fallback path for local development

Notes:

- Item order in `items` controls rendered output order.
- `colors_enabled = false` disables ANSI escapes for plain terminals.
- `timezone` must be a valid IANA timezone, such as `UTC` or `Europe/Rome`.
- `kind = "command"` runs external commands and renders stdout as a segment.

### External command items

Use `kind = "command"` to extend the status line without recompiling.

Behavior:

- The command is executed directly (`command` + `args`), so any language/runtime works.
- Nothing is rendered if the command fails, times out, or prints an empty result.
- Output is trimmed and multi-line output is collapsed into one line.
- Default timeout is `120ms` if `timeout_ms` is omitted.

Example with your CAVEMAN flag logic:

```toml
[[items]]
kind = "command"
command = "sh"
args = [
  "-c",
  '''
caveman_text=""
caveman_flag="$HOME/.claude/.caveman-active"
if [ -f "$caveman_flag" ]; then
  caveman_mode=$(cat "$caveman_flag" 2>/dev/null)
  if [ "$caveman_mode" = "full" ] || [ -z "$caveman_mode" ]; then
    caveman_text=$'\033[38;5;172m[CAVEMAN]\033[0m'
  else
    caveman_suffix=$(echo "$caveman_mode" | tr '[:lower:]' '[:upper:]')
    caveman_text=$'\033[38;5;172m[CAVEMAN:'"${caveman_suffix}"$']\033[0m'
  fi
fi
printf "%s" "$caveman_text"
'''
]
timeout_ms = 150
```

You can also call a script/binary directly, for example `command = "python3"` with a script path in `args`.

## Preview mode

Use preview mode to validate and tune configuration without Claude input payloads:

```bash
# default sample preview
claude-statusline --preview

# explicit modes
claude-statusline --preview sample
claude-statusline --preview validate
claude-statusline --preview config
claude-statusline --preview explain
claude-statusline --preview colors
```

## Claude Code setup

### Manual snippet

```json
{
  "statusLine": {
    "type": "command",
    "command": "claude-statusline"
  }
}
```

### Automatic setup command

The binary can update `~/.claude/settings.json` for you:

```bash
claude-statusline --auto-setup
```

Behavior:

- If `statusLine` exists, it is replaced with exactly `type` and `command`.
- If `statusLine` is missing, it is created.
- `statusLine.command` is always set to `claude-statusline`.
- If `~/.claude/statusline.toml` is missing, a default config is created.
- If `~/.claude/statusline.toml` exists without the claude-statusline project header, it is replaced.
- Keep the header line `# claude-statusline-project: fabifont/claude-statusline` to prevent replacement on setup.
- Other keys in `settings.json` are preserved.

Note:

- This is intentionally an explicit command, not an automatic install hook, to avoid silently changing user Claude config during package installation.

### Why command name only

Using `"command": "claude-statusline"` works consistently across Cargo and local installs, as long as the install directory is in `PATH`.

If Claude Code cannot find the command, update your shell profile to include the appropriate folder.

## Versioning

This project follows Semantic Versioning (`MAJOR.MINOR.PATCH`).

- Example versions: `0.1.0`, `0.2.0`, `1.0.0`
- Tag format: `vX.Y.Z` (recommended convention for GitHub Releases)

`vX.Y.Z` is not required by Rust itself, but it is the most common and practical GitHub release tag convention.

## Release helper

Use the helper script to bump release version files:

```bash
./scripts/release-helper.sh 0.2.0
```

The helper updates the version in `Cargo.toml`.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

## Release

- CI workflow: `.github/workflows/ci.yml`
- Tag release workflow: `.github/workflows/release.yml`
- Release artifact: `claude-statusline-x86_64-unknown-linux-gnu.tar.gz`

## Updating

- Cargo install update:

```bash
cargo install --git https://github.com/fabifont/claude-statusline --force
```

## Uninstall

```bash
cargo uninstall claude-statusline
rm -f ~/.local/bin/claude-statusline
rm -f ~/.config/claude-statusline/config.toml
```
