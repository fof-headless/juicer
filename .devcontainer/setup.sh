#!/usr/bin/env bash
# Codespace bootstrap for building the Juicer Blender fork on Linux.
# Installs the toolchain + the system -dev packages Blender's precompiled-lib
# build needs on Ubuntu. Runs once when the Codespace is created.
set -euo pipefail

echo "==> Installing build toolchain + Blender Linux build deps"
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  build-essential cmake ninja-build git git-lfs subversion \
  libx11-dev libxxf86vm-dev libxcursor-dev libxi-dev libxrandr-dev \
  libxinerama-dev libxkbcommon-dev libegl-dev \
  libwayland-dev wayland-protocols libdbus-1-dev linux-libc-dev \
  libssl-dev zlib1g-dev

git lfs install --skip-repo || true

echo "==> Rust toolchain (for the Tauri side)"
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi

echo "==> Done. To build the fork:"
echo "    cd blender-fork && ./build-fork.sh"
