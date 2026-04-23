#!/usr/bin/env bash

set -euo pipefail

if ! command -v dpkg-deb >/dev/null 2>&1; then
    echo "dpkg-deb is required. Install it with: sudo apt install dpkg-dev" >&2
    exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="$(awk -F'"' '/^version = / { print $2; exit }' "${ROOT_DIR}/Cargo.toml")"
ARCH="${ARCH:-amd64}"
PACKAGE_NAME="xtmonctl"
WORK_DIR="$(mktemp -d)"
PACKAGE_DIR="${WORK_DIR}/${PACKAGE_NAME}_${VERSION}_${ARCH}"

cleanup() {
    rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

cd "${ROOT_DIR}"
CARGO_BUILD_RUSTC="${HOME}/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc" \
    "${HOME}/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo" build --release --locked

mkdir -p "${PACKAGE_DIR}/DEBIAN" \
         "${PACKAGE_DIR}/usr/bin" \
         "${PACKAGE_DIR}/usr/share/doc/${PACKAGE_NAME}"

cat > "${PACKAGE_DIR}/DEBIAN/control" <<EOF
Package: ${PACKAGE_NAME}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: thorodinson246 <mukeshpoudel246@gmail.com>
Depends: ddcutil
Description: External monitor brightness control for Linux via ddcutil
 A keyboard-friendly CLI and terminal UI for external monitor brightness
 control using DDC/CI.
EOF

install -Dm755 "target/release/xtmonctl" "${PACKAGE_DIR}/usr/bin/xtmonctl"
install -Dm644 "README.md" "${PACKAGE_DIR}/usr/share/doc/${PACKAGE_NAME}/README.md"

dpkg-deb --build "${PACKAGE_DIR}"
mv "${PACKAGE_DIR}.deb" "${ROOT_DIR}/"

echo "Built ${ROOT_DIR}/${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"
