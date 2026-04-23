#!/usr/bin/env bash
#
# xtmonctl installation script
#

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

AUTO_YES=false
FROM_RELEASE=false
REPO_SLUG="${REPO_SLUG:-ThorOdinson246/xmonctl-rs}"

for arg in "$@"; do
    case "$arg" in
        --yes|-y)
            AUTO_YES=true
            ;;
        --from-release)
            FROM_RELEASE=true
            ;;
    esac
done

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

confirm() {
    if [ "$AUTO_YES" = true ]; then
        return 0
    fi
    read -r -p "$1 [y/N] " reply
    [[ "$reply" =~ ^[Yy]$ ]]
}

detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        echo "$ID"
    else
        echo "unknown"
    fi
}

install_ddcutil() {
    case "$(detect_distro)" in
        ubuntu|debian|linuxmint|pop)
            sudo apt update
            sudo apt install -y ddcutil
            ;;
        fedora)
            sudo dnf install -y ddcutil
            ;;
        arch|manjaro|endeavouros)
            sudo pacman -S --noconfirm ddcutil
            ;;
        *)
            log_warn "Install ddcutil manually for your distribution."
            ;;
    esac
}

install_rust() {
    if command -v cargo >/dev/null 2>&1; then
        log_success "Rust toolchain already installed."
        return 0
    fi

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup-init.sh
    sh /tmp/rustup-init.sh -y
    . "$HOME/.cargo/env"
    rustup component add rustfmt clippy
}

setup_i2c_module() {
    sudo modprobe i2c-dev
    echo "i2c-dev" | sudo tee /etc/modules-load.d/ddcutil.conf >/dev/null
}

setup_udev_rules() {
    local rules_file="/etc/udev/rules.d/60-ddcutil-i2c.rules"
    if [ ! -f "$rules_file" ]; then
        cat <<'EOF' | sudo tee "$rules_file" >/dev/null
SUBSYSTEM=="i2c-dev", KERNEL=="i2c-[0-9]*", TAG+="uaccess"
SUBSYSTEM=="drm", KERNEL=="card[0-9]*", TAG+="uaccess"
EOF
        sudo udevadm control --reload-rules
        sudo udevadm trigger
    fi
}

install_xtmonctl() {
    . "$HOME/.cargo/env"
    if [ "$FROM_RELEASE" = true ]; then
        install_release_binary
    else
        cargo install --path .
    fi
}

install_release_binary() {
    local arch="x86_64-unknown-linux-gnu"
    local url
    url="https://github.com/${REPO_SLUG}/releases/latest/download/xtmonctl-${arch}.tar.gz"

    if ! curl -fsI "$url" >/dev/null 2>&1; then
        log_warn "No release artifact found for ${arch}. Falling back to cargo install."
        cargo install --path .
        return 0
    fi

    local temp_dir
    temp_dir="$(mktemp -d)"
    curl -fsSL "$url" -o "${temp_dir}/xtmonctl.tar.gz"
    tar -xzf "${temp_dir}/xtmonctl.tar.gz" -C "${temp_dir}"
    install -Dm755 "${temp_dir}/xtmonctl" "$HOME/.local/bin/xtmonctl"
    log_success "Installed xtmonctl to $HOME/.local/bin/xtmonctl"
}

main() {
    if confirm "Install or update ddcutil?"; then
        install_ddcutil
    fi
    if confirm "Install Rust toolchain if needed?"; then
        install_rust
    fi
    if confirm "Load i2c-dev and configure it for boot?"; then
        setup_i2c_module
    fi
    if confirm "Install udev rules for monitor control?"; then
        setup_udev_rules
    fi
    if confirm "Install xtmonctl from this directory?"; then
        install_xtmonctl
    fi

    log_info "Usage:"
    echo "  xtmonctl"
    echo "  xtmonctl list"
    echo "  xtmonctl set 1 70"
    echo "  ./scripts/install.sh --from-release"
}

main "$@"
