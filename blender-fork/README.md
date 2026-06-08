# Juicer — Blender fork

This is the **Pro render path**: a fork of Blender that ingests a Juicer
`scene.json` **natively in C++** and renders it with Blender's real keyframe
(F-curve) engine and Cycles/Eevee. **No Python / bpy** — a new CLI flag drives
Blender's own compiled subsystems directly.

```
blender --background --juicer scene.json -o /path/out_ -a
```

## Why a fork (and what that honestly costs)

You asked to use Blender's *real, mastered* code rather than reimplement it.
Blender can't be embedded as a library and its animation engine can't be
extracted from the program, so the only faithful way is to **modify Blender's
source and add our own entry point**. That's what this is.

- **Pinned to** Blender commit `1957ef3271c5a38e4f561aaf1ff67d735d3abead`
  (v5.03 alpha), GitHub mirror `github.com/blender/blender`.
- **License:** linking Blender makes this GPLv2-or-later. (You said non-commercial,
  fine — just know the fork is GPL.)
- **Build cost:** a full Blender build needs a real machine (~40 GB disk, pulls
  ~10 GB of precompiled libs, 30–90 min on first build). It **cannot** be built
  in the ephemeral web container, so the binary is produced on your Mac / a CI
  runner, then bundled into Juicer.app.

## What the fork adds (small, surgical surface)

Everything we add lives in **one new module** plus three injected lines —
nothing in Blender's existing code is rewritten, so rebases onto newer Blender
stay cheap.

| File | Role |
|---|---|
| `module/IO_juicer.hh` | public entry `import_and_render()` |
| `module/importer/juicer_scene.cc` | parses scene.json → builds objects, textures HTML PNGs onto planes, inserts **real F-curve keyframes**, sets camera + render settings, calls `RE_RenderAnim` |
| `module/CMakeLists.txt` | builds `bf_io_juicer`, links blenkernel/animrig/render |
| `patches/creator_args.snippet.cc` | the `--juicer` CLI handler |
| `patches/inject_cli.py` | idempotently splices the flag into `creator_args.cc` |

Keyframe easing maps almost 1:1 — Juicer's cubic ease → Blender
`BEZT_IPO_CUBIC` + `BEZT_IPO_EASE_IN/OUT/IN_OUT`; linear → `BEZT_IPO_LIN`;
step → `BEZT_IPO_CONST`.

## Build

```bash
cd blender-fork
./build-fork.sh          # clone pinned source, splice module, fetch libs, compile
# → build-blender/bin/blender
```

Point Juicer at it (until it's bundled):

```bash
export JUICER_BLENDER=/abs/path/blender-fork/build-blender/bin/blender
```

The Tauri backend (`src-tauri/src/blender.rs`) auto-discovers it via that env
var, the app bundle, or `build-blender/bin/blender`.

## Status — honest

The module is written against the **real pinned APIs** (verified signatures for
`BKE_object_add`, `BKE_mesh_new_nomain`, `animrig::insert_vert_fcurve`,
`RE_RenderAnim`, etc.), but it has **not been compiled yet** — that needs a
build machine. Expect first-compile iteration on a few spots, all isolated to
`juicer_scene.cc` and flagged with `// see build notes`:

1. **F-curve creation in 5.x** — Actions are now layered (slots); the
   `action_fcurve_ensure*` call is the most likely first-compile fixup.
2. **Material node graph** — image-texture → BSDF (emission for `unlit`) and the
   opacity → alpha F-curve socket path need the node API wired.
3. **Box/sphere primitives** and **camera aim** (track-to target) — stubbed.

Run `./build-fork.sh` on a real machine and paste the first compile errors back;
each maps to one of the points above and is a quick fix.
