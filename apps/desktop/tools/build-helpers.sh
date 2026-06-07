#!/usr/bin/env bash
# Build the native macOS helper binaries (HTML capture + video encoder).
# Requires Xcode CLI tools only (swiftc) — NOT the full Xcode app.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HERE/../src-tauri/bin"
mkdir -p "$BIN_DIR"

echo "→ Building juicer-html-capture…"
swiftc "$HERE/html-capture/main.swift" \
    -O \
    -o "$BIN_DIR/juicer-html-capture" \
    -framework WebKit -framework AppKit

echo "→ Building juicer-encoder…"
swiftc "$HERE/encoder/main.swift" \
    -O \
    -o "$BIN_DIR/juicer-encoder" \
    -framework AVFoundation -framework AppKit -framework CoreMedia

echo "✓ Helpers built into $BIN_DIR"
ls -la "$BIN_DIR"
