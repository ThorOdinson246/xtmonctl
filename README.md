# xtmonctl

External monitor brightness control for Linux via `ddcutil`.

## Features

- Interactive TUI
- CLI commands for automation
- Multi-monitor support
- YAML configuration
- Honest brightness percentages for monitors whose raw max is not `100`

## Installation

```bash
./scripts/install.sh
```

## Usage

```bash
xtmonctl
xtmonctl list
xtmonctl get 1
xtmonctl set 1 70
xtmonctl set 1 +10
xtmonctl all 40
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

## Development

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```
