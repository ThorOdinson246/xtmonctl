# xtmonctl

`xtmonctl` is a Linux command-line and terminal UI tool for controlling the brightness of external monitors through `ddcutil` and DDC/CI.

It is aimed at people who live in the terminal and want a straightforward way to manage monitor brightness without opening a desktop settings panel every time. The most natural audience is Arch Linux, Debian, Ubuntu, Fedora, and similar desktop users who already install command-line tools, use external monitors, and are comfortable with package-manager setup steps when hardware access is involved.

## What It Is Good For

- External monitor brightness control from the terminal
- Keyboard-driven brightness adjustments in a TUI
- Shell scripting and automation with plain text or JSON output
- Managing multi-monitor setups with friendly aliases
- Linux desktop users who want a small native binary instead of a Python toolchain

## What It Does Not Guarantee

`xtmonctl` does not work with every screen on every Linux system.

It depends on:

- Linux
- `ddcutil`
- a monitor that supports DDC/CI
- a cable and GPU path that actually passes DDC/CI traffic
- permission to access I2C devices

Internal laptop panels usually do not work through DDC/CI. Some adapters, docks, KVMs, HDMI splitters, and unusual GPU drivers can also break DDC/CI support even when the monitor itself is capable.

## Support Matrix

Officially targeted environment:

- OS: Linux
- Monitor type: external monitors with DDC/CI support
- Interface: HDMI, DisplayPort, DVI, VGA where DDC/CI is exposed correctly
- Shell use: supported
- TUI use: supported

Best-supported distributions:

- Arch Linux and Arch-based distributions
- Debian and Ubuntu
- Fedora

Not currently targeted:

- macOS
- Windows
- internal laptop brightness control

## Features

- Interactive TUI
- Scriptable CLI
- Multi-monitor support
- Honest percentage reporting when the monitor max is not `100`
- Alias-aware monitor lookup
- YAML configuration
- JSON output for scripting
- Release binaries for direct installation

## Installation

### Option 1: Install the latest release binary

This is the simplest way to install it like a normal terminal program.

```bash
curl -fsSL https://github.com/ThorOdinson246/xmonctl-rs/releases/latest/download/xtmonctl-x86_64-unknown-linux-gnu.tar.gz -o xtmonctl.tar.gz
tar -xzf xtmonctl.tar.gz
install -Dm755 xtmonctl "$HOME/.local/bin/xtmonctl"
```

Make sure `$HOME/.local/bin` is on your `PATH`.

### Option 2: Use the interactive installer

```bash
./scripts/install.sh
./scripts/install.sh --from-release
```

`--from-release` prefers the latest GitHub release binary. Without it, the script installs from the local source tree with Cargo.

### Option 3: Install from source with Cargo

```bash
cargo install --path .
```

## Distribution-Specific Setup

### Arch Linux

Install system requirements:

```bash
sudo pacman -S ddcutil rustup
rustup default stable
rustup component add rustfmt clippy
```

Build and install locally:

```bash
cargo install --path .
```

There is also an Arch packaging helper at [packaging/arch/PKGBUILD](</mnt/686c9079-54d3-48e6-b3f8-474d3f6d2175/OSource/xtmonctl-rs/packaging/arch/PKGBUILD>) if you want to turn it into a package with `makepkg`.

### Debian and Ubuntu

Install system requirements:

```bash
sudo apt update
sudo apt install -y ddcutil curl build-essential pkg-config dpkg-dev
```

Install from release:

```bash
./scripts/install.sh --from-release
```

Build a local `.deb` package:

```bash
./packaging/debian/build-deb.sh
sudo apt install ./xtmonctl_0.1.0_amd64.deb
```

### Fedora

Install system requirements:

```bash
sudo dnf install -y ddcutil rust cargo
```

Then install from source:

```bash
cargo install --path .
```

## Required System Access

For non-root use, your system usually needs:

- the `i2c-dev` kernel module
- udev rules allowing access to `/dev/i2c-*`
- a session restart after group or permission changes

The installer script helps configure these, but the exact setup depends on your distribution.

## Usage

### Start the TUI

```bash
xtmonctl
```

### CLI Commands

```bash
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

### JSON Output

Examples:

```bash
xtmonctl list --json
xtmonctl get 1 --json
xtmonctl alias list --json
```

This is useful when integrating `xtmonctl` into shell scripts, window manager hooks, or custom desktop widgets.

## Configuration

Default config path:

```text
~/.config/xtmonctl/config.yaml
```

Override it with:

```bash
xtmonctl --config /path/to/config.yaml list
```

Example config:

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

## Release and Packaging

### GitHub Releases

Pushing a tag like `v0.1.0` triggers the release workflow, which uploads:

- `xtmonctl-x86_64-unknown-linux-gnu.tar.gz`

### Arch Packaging

Use:

```bash
cd packaging/arch
makepkg -si
```

### Debian Packaging

Use:

```bash
./packaging/debian/build-deb.sh
sudo apt install ./xtmonctl_0.1.0_amd64.deb
```

## Troubleshooting

### No monitors detected

Check:

- that the monitor supports DDC/CI
- that DDC/CI is enabled in the monitor menu
- that your cable or dock passes DDC/CI
- that `ddcutil detect` works directly

### Permission denied

You probably need I2C access configured for your user. Re-run:

```bash
./scripts/install.sh
```

### The monitor is listed but brightness reads fail

That usually means the monitor is visible but DDC/CI communication is unreliable on the current cable, dock, GPU output, or adapter path.

## Development

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

## Current Status

`xtmonctl` is usable now for Linux users with supported external monitors, but it should still be treated as an early-stage hardware utility rather than a guaranteed universal monitor tool. If your setup is a common Linux desktop with standard HDMI or DisplayPort external displays, it should be a good fit. If your setup relies on unusual docks, KVMs, laptop internal panels, or proprietary GPU edge cases, expect some trial and error.
