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

  # 1) Register the io/juicer subdir + link it into the io aggregate.
  local io_cmake="$SRC/source/blender/io/CMakeLists.txt"
  grep -q "add_subdirectory(juicer)" "$io_cmake" || \
    printf '\nadd_subdirectory(juicer)\n' >> "$io_cmake"

  # 2) Link bf_io_juicer into the creator (final binary).
  local creator_cmake="$SRC/source/creator/CMakeLists.txt"
  grep -q "bf_io_juicer" "$creator_cmake" || \
    sed -i.bak 's/\(list(APPEND LIB[[:space:]]*\)/\1\n    bf_io_juicer/' "$creator_cmake" || true

  # 3) Inject the CLI handler + include + registration into creator_args.cc.
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
  all)   clone; patch_in; build ;;
  *)     echo "usage: $0 [all|patch]"; exit 1 ;;
esac
