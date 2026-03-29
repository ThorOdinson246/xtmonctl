# xtmonctl

External monitor brightness control for Linux via `ddcutil`.

## Features

- Interactive TUI
- CLI commands for automation
- Multi-monitor support
- YAML configuration
- Honest brightness percentages for monitors whose raw max is not `100`
- Alias-aware monitor lookup through config entries
- Visible error reporting in the TUI during refresh and brightness changes

## Installation

```bash
./scripts/install.sh
./scripts/install.sh --from-release
```

For a local developer install:

```bash
cargo install --path .
```

For a tagged release binary:

```bash
curl -fsSL https://github.com/ThorOdinson246/xmonctl-rs/releases/latest/download/xtmonctl-x86_64-unknown-linux-gnu.tar.gz -o xtmonctl.tar.gz
tar -xzf xtmonctl.tar.gz
install -Dm755 xtmonctl "$HOME/.local/bin/xtmonctl"
```

## Usage

```bash
xtmonctl
xtmonctl list
xtmonctl list --json
xtmonctl get 1
xtmonctl get "Main Monitor"
xtmonctl set 1 70
xtmonctl set 1 +10
xtmonctl all 40
xtmonctl alias list
xtmonctl alias set 1 "Main Monitor"
xtmonctl alias clear 1
xtmonctl config path
```

## Configuration

Config file:

```text
~/.config/xtmonctl/config.yaml
```

Example:

```yaml
monitors:
  i2c-4:
    alias: Main Monitor
    last_brightness_percent: 70
default_step_percent: 5
large_step_percent: 10
detection_timeout_secs: 15
command_timeout_secs: 5
```

## Distribution

- Push a tag like `v0.1.0` to trigger the release workflow.
- The workflow uploads `xtmonctl-x86_64-unknown-linux-gnu.tar.gz` to the GitHub release.
- `./scripts/install.sh --from-release` downloads the latest release artifact when one exists.

## Development

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```
