#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Build the Juicer fork of Blender: clone the pinned source, splice in our
# native `--juicer` module + CLI flag, and compile. Produces a `blender`
# binary that ingests Juicer scene.json directly (no Python).
#
# Requires a real build machine (NOT the ephemeral web container):
#   macOS: Xcode + cmake; or Linux: build-essential cmake ninja
#   ~40 GB free disk, the build pulls ~10 GB of precompiled libraries.
#
# Usage:
#   ./build-fork.sh           # clone + patch + build into ./build-blender
#   ./build-fork.sh patch     # only (re)apply our module + patches
#   ./build-fork.sh build     # only (re)compile (incremental; no clone/patch)
set -euo pipefail

PINNED=1957ef3271c5a38e4f561aaf1ff67d735d3abead
HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="${JUICER_BLENDER_SRC:-$HERE/blender}"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}"

clone() {
  if [ ! -d "$SRC/.git" ]; then
    echo "==> Cloning Blender @ $PINNED (GitHub mirror)"
    git clone https://github.com/blender/blender.git "$SRC"
    git -C "$SRC" checkout "$PINNED"
  fi
  echo "==> Fetching precompiled libraries (make update)"
  ( cd "$SRC" && make update )
}

patch_in() {
  echo "==> Splicing Juicer module into source tree"
  local mod="$SRC/source/blender/io/juicer"
  rm -rf "$mod"; mkdir -p "$mod/importer"
  cp "$HERE/module/IO_juicer.hh" "$mod/"
  cp "$HERE/module/CMakeLists.txt" "$mod/"
  cp "$HERE/module/importer/juicer_scene.cc" "$mod/importer/"

  # 1) Register the io/juicer subdir so the lib gets built.
  local io_cmake="$SRC/source/blender/io/CMakeLists.txt"
  grep -q "add_subdirectory(juicer)" "$io_cmake" || \
    printf '\nadd_subdirectory(juicer)\n' >> "$io_cmake"

  # 2) Link bf_io_juicer into the final `blender` binary by adding it to the
  #    creator's set(LIB ...) block (nothing else depends on our new lib, so
  #    without this it compiles but never links -> undefined symbol).
  local creator_cmake="$SRC/source/creator/CMakeLists.txt"
  python3 - "$creator_cmake" <<'PYEOF'
import sys, re
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
if "bf_io_juicer" not in text:
    # Insert into the first set(LIB ...) block, right after the opencolorio dep.
    anchor = "PRIVATE bf::dependencies::opencolorio\n"
    if anchor in text:
        text = text.replace(anchor, anchor + "  PRIVATE bf_io_juicer\n", 1)
    else:
        # Fallback: insert before the first closing paren of a set(LIB ...) block.
        text = re.sub(r"(set\(LIB\b.*?)(\n\))",
                      r"\1\n  PRIVATE bf_io_juicer\2", text, count=1, flags=re.S)
    open(path, "w", encoding="utf-8").write(text)
    print("  + added bf_io_juicer to creator set(LIB)")
else:
    print("  bf_io_juicer already in creator set(LIB)")
PYEOF

  # 3) Inject the CLI handler + registration into creator_args.cc.
  python3 "$HERE/patches/inject_cli.py" "$SRC/source/creator/creator_args.cc" \
          "$HERE/patches/creator_args.snippet.cc"
  echo "==> Patch complete"
}

build() {
  echo "==> Configuring (CMake) and building with $JOBS jobs"
  cmake -S "$SRC" -B "$HERE/build-blender" -G Ninja \
        -DWITH_PYTHON=ON -DWITH_CYCLES=ON -DWITH_FFMPEG=ON \
        -DCMAKE_BUILD_TYPE=Release
  cmake --build "$HERE/build-blender" --parallel "$JOBS"
  echo "==> Built: $HERE/build-blender/bin/blender"
}

case "${1:-all}" in
  patch) patch_in ;;
  build) build ;;
  all)   clone; patch_in; build ;;
  *)     echo "usage: $0 [all|patch|build]"; exit 1 ;;
esac
