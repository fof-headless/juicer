# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> **Juicer v0.2 — pivot complete.** The codebase is now a Figma-style 2.5D layer
> compositor with a Blender-grade keyframe engine, rendered through one
> offscreen WKWebView. The old 3D wgpu renderer and `blender.rs` are gone.
> Full design rationale: `/Users/shreyanshkushwaha/.claude/plans/so-everything-built-till-synthetic-octopus.md`

## What this is (target architecture, post-pivot)

A self-contained macOS app for **product-demo / product-launch motion design**:

- **Figma-style 2D layers** (rect/ellipse/text/image/HTML) composited in z-order, with optional **2.5D tilts** (`rotateX`/`rotateY`/`perspective`) per layer.
- **Blender-grade animation engine** — per-property keyframe tracks with cubic-bezier handles. The reference for the model is Blender's F-curves; the actual Blender source in `blender-fork/` is kept as a **UX/code-quality reference only** (gizmos, viewport, F-curve editor, action editor patterns), not as a build target.
- **The browser is the renderer.** One offscreen `WKWebView` holds a tiny `renderer.html` shell; every layer is a positioned DOM node. HTML layers are wrapped in `<iframe srcdoc>` so user CSS cannot leak between layers. Per frame, Rust evaluates all tracks at time T, ships a `RenderState` blob over stdio to a long-lived Swift helper (`juicer-frame-renderer`), helper applies styles via JS, captures a PNG.
- **Claude drives via MCP** — atomic per-property tools (~25 of them). Claude can `add_html_layer`, `set_shadow`, `set_keyframe(..., bezier)`, `render_frame(15)` for an inline preview, iterate.
- **Same renderer.html is the viewport iframe in the React UI**, so live preview is byte-identical to recorded MP4.

## Pivot status

All six phases complete. End-to-end verified: an MCP `add_shape_layer` →
`set_shadow` → `set_keyframe` (ease-back overshoot) → `render_frame(15)` pipeline
produces a pixel-correct 1920×1080 PNG at the expected x-position with the
shadow applied. ~40 MCP tools registered.

When picking up where this left off, start by skimming the **Layer model**
section below, then look at `dispatch_tool` in `lib.rs:79` — every MCP tool and
Tauri command flows through there.

## Build & dev

The monorepo is a pnpm workspace. Top-level scripts in `package.json` proxy into `apps/desktop`:

```bash
pnpm install
pnpm --filter juicer-desktop helpers   # build the Swift helpers into src-tauri/bin/
pnpm dev                                # helpers + tauri dev (hot-reload UI + Rust)
pnpm build                              # production: Juicer.app + DMG
pnpm ui                                 # vite dev server only (no Rust)
```

Swift helpers MUST exist in `apps/desktop/src-tauri/bin/` before `tauri dev|build` — they are listed as bundled `resources` in `tauri.conf.json` and `cargo build` will fail otherwise. `pnpm dev`/`pnpm build` chain them automatically. There are three helpers now: `juicer-html-capture` (one-shot), `juicer-frame-renderer` (long-lived WKWebView for per-frame snapshots), `juicer-encoder` (PNG sequence → MP4).

Helpers are built with `swiftc` (Xcode CLI tools only — not the full Xcode app).

No test suite, no linter config, no CI. `cargo check` / `tsc` are the verification steps.

## Entry-point split: GUI vs MCP

`apps/desktop/src-tauri/src/lib.rs::run()` checks `--mcp` in argv:

- **no flag** → Tauri GUI, React UI in a WKWebView, all `#[tauri::command]`s wired up.
- **`--mcp`** → pure stdio MCP server (`mcp::run_stdio_server`), no window, same `AppState`.

The same binary is both. Claude Desktop is configured to spawn `juicer --mcp`. When editing scene mutations, **both call paths must stay in sync** — they share `apply_property`, `parse_*` helpers (in `lib.rs`). Divergence has bitten us; reuse helpers, don't duplicate logic.

## Architecture (target, post-pivot)

```
apps/desktop/src-tauri/src/
  main.rs           thin shim → juicer_lib::run()
  lib.rs            AppState, #[tauri::command]s, --mcp dispatch, apply_property dispatcher
  mcp.rs            JSON-RPC stdio server; is_mutation() drives autosave
  scene.rs          Scene, Layer, LayerKind, Transform2_5D, Effects, Fill, BlendMode — 2D data model
  anim.rs           Track/Keyframe with cubic-bezier easing + color interpolation; evaluate_at()
  renderer.rs       (NEW) Rust ↔ juicer-frame-renderer IPC over stdio
  html_capture.rs   wrap_html() + one-shot capture via juicer-html-capture (reused for iframe srcdoc)
  video.rs          frame-PNG sequence → MP4 via juicer-encoder
  project.rs        ~/Movies/Juicer/<name>/ folders, scene.json autosave, asset import

apps/desktop/src-tauri/resources/
  tailwind.js       vendored Tailwind v4 (offline)
  renderer.html     (NEW) layer DOM scaffolding loaded by juicer-frame-renderer
  runtime.js        (NEW) applyState/updateLayer bridge inside renderer.html

apps/desktop/tools/
  html-capture/     juicer-html-capture (one-shot WKWebView → PNG)
  frame-renderer/   (NEW) juicer-frame-renderer (long-lived WKWebView, JSON-RPC over stdio)
  encoder/          juicer-encoder (AVFoundation → MP4)
```

**Shared state lives in `AppState` (`Mutex<Scene>`, `Mutex<Option<Renderer>>`, `Mutex<Option<Project>>`).** Lock order is **project → scene** to avoid deadlock — `autosave()` documents this; do not hold the scene lock when calling `autosave()` or any project op.

**Autosave triggers off `mcp::is_mutation()`** — any new MCP tool that mutates scene state must be added there, or its changes won't persist. The Tauri command path autosaves via explicit calls in each handler.

## Layer model (the heart of the new data model)

Defined in `scene.rs`. A `Scene` is `{ layers: Vec<Layer>, canvas: Canvas, duration_frames }`. Each `Layer` has:

- `id`, `name`, `kind` (Html | Image | Shape | Text)
- `width`, `height` (on-canvas px before transform)
- `transform`: `Transform2_5D` (x, y, rotation, scaleX, scaleY, rotateX, rotateY, perspective, transform_origin)
- `opacity`, `visible`, `blend_mode`
- `effects`: `Vec<BoxShadow>` + `filter_blur`
- `tracks`: `Vec<NamedTrack>` — keyframes per animatable property name

**Animatable property names** (every one a track key Claude can keyframe):
`x`, `y`, `rotation`, `scale_x`, `scale_y`, `rotate_x`, `rotate_y`, `perspective`, `opacity`, `width`, `height`, `border_radius`, `shadow_offset_x`, `shadow_offset_y`, `shadow_blur`, `shadow_spread`, `shadow_color`, `filter_blur`, `fill_color`, `text_content` (step only), `font_size`.

## Animation engine

`anim.rs` has Tracks of Keyframes. Each `Keyframe` carries a `value: KeyValue` and an `easing: Easing` (the easing belongs to the *outgoing* key — same convention as before the pivot).

- `KeyValue`: `Scalar(f32) | Vec3([f32;3]) | Color([f32;4] rgba 0..1) | Str(String)`
- `Easing`: `Linear | Step | Bezier { p1: [f32;2], p2: [f32;2] }`. Named eases (ease-in/out/in-out) are bezier presets.
- Color lerp is **OKLCH-interpolated**, not RGB. (RGB lerps look muddy through gray midpoints.)
- `Track::sample(frame) -> Option<KeyValue>` is the eval API.
- `Scene::evaluate_at(frame) -> RenderState` walks every layer's tracks and produces a per-layer style snapshot. This snapshot is what gets pushed to the renderer.

## Renderer pipeline (post-Phase 2)

```
Rust scene.json ─evaluate_at─▶ RenderState (per-layer style snapshot)
                                       │ stdio JSON
                                       ▼
                          juicer-frame-renderer (Swift, long-lived)
                                       │ WKWebView holds renderer.html
                                       │ JS runtime applies styles
                                       ▼
                            snapshot → base64 PNG
                                       │
                                       ▼
                            juicer-encoder → MP4
```

The Tauri main window embeds the *same* `renderer.html` in the viewport area as an iframe — so live preview = recorded output, byte for byte.

## Projects on disk

Every scene lives in `~/Movies/Juicer/<name>/` with `scene.json`, `assets/`, `renders/`. On startup `ensure_default_project()` opens-or-creates `Default` and loads its scene. `import_asset` copies external files into `assets/`. `render_frame`/`render_animation` default outputs into `renders/`.

## HTML layers — isolation

Each HTML layer is rendered inside an `<iframe srcdoc="...">` so user CSS (Tailwind, custom `<style>` tags) cannot bleed onto other layers. The srcdoc is built by `html_capture::wrap_html` (injects vendored Tailwind, fonts, Lucide, etc. — same code already in use for one-shot captures). Tailwind v4 browser build is vendored at `apps/desktop/src-tauri/resources/tailwind.js` for fully-offline capture.

## Blender fork — reference only

`blender-fork/` is kept in-repo as a **read-only reference**. The user studies it for production-tool UX patterns (gizmos, viewport navigation, F-curve graph editor, action editor). It is NOT built and NOT shipped. `blender.rs` (the binary discovery + invocation code) is deleted in Phase 6.

## When adding an MCP tool

1. Add to `mcp::tool_definitions()` (JSON schema) and dispatch in the request match.
2. If it mutates scene state, add tool name to `is_mutation()` so autosave fires.
3. Reuse `apply_property` and `parse_*` from `lib.rs` — the Tauri command path uses them too.
4. Mirror as a `#[tauri::command]` so the UI inspector can use the same setter.

## Connecting Claude Desktop

Production config points at `/Applications/Juicer.app/Contents/MacOS/juicer --mcp`. During dev, point at `apps/desktop/src-tauri/target/debug/juicer --mcp` (run `pnpm dev` once to compile). See README for the full `claude_desktop_config.json` snippet.

## Critical files for orientation

- `apps/desktop/src-tauri/src/lib.rs` — `AppState`, `dispatch_tool` (single source of truth for MCP + UI ops), Tauri entry point. `dispatch_tool` is at the top — every tool routes through here.
- `apps/desktop/src-tauri/src/mcp.rs` — `tool_definitions()` JSON schemas and the JSON-RPC framing. Tool *implementations* live in `lib.rs`, NOT here.
- `apps/desktop/src-tauri/src/scene.rs` — Layer model + `evaluate_at(frame)` that produces the per-layer CSS snapshot.
- `apps/desktop/src-tauri/src/anim.rs` — Bezier easing (Newton-Raphson y-for-x), OKLCH color interpolation, Track + sample.
- `apps/desktop/src-tauri/src/renderer.rs` — IPC to `juicer-frame-renderer`. Spawns the child, writes JSON lines, reads JSON lines, base64-decodes the PNG.
- `apps/desktop/src-tauri/resources/renderer.html` + `runtime.js` — the actual render shell. ALSO symlinked into `apps/desktop/public/` so vite serves them at `/renderer.html` for the UI iframe.
- `apps/desktop/tools/frame-renderer/main.swift` — the long-lived helper. `callAsyncJavaScript` awaits the JS promise from `window.__juicer.renderFrame(payload)`, then snapshots, then `CGImage`-resizes to exact canvas pixels (needed because WKSnapshot's `snapshotWidth` is in *points*, not pixels — Retina would otherwise produce 2× frames).

## Gotchas worth remembering

- **`requestAnimationFrame` does NOT fire reliably in an offscreen WKWebView** (no display tied to it). Use `setTimeout` for yield-and-resume. See `waitTwoFrames` in `runtime.js`.
- **`WKSnapshotConfiguration.snapshotWidth` is in points, not pixels.** With Retina, naive snapshot returns 2× dims. We downscale via `CGContext`/`ImageIO` to pixel-exact dims in `frame-renderer/main.swift`.
- **`NSBitmapImageRep.representation(using:.png)` mysteriously double-scales** even with explicit pixel dims. Use `CGImageDestination` (ImageIO) instead.
- **Lock order is project → scene.** Don't hold the scene lock when calling `autosave()` or any project op (deadlock).
- **`is_mutation()` in `lib.rs`** must include any new mutating tool name, or autosave won't fire for it.

## Picking up where I left off

1. `git log --oneline -5` to see the most recent commit (look for `feat(juicer-v0.2): …`).
2. `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` — should be clean.
3. `pnpm exec tsc --noEmit` in `apps/desktop/` — should be clean.
4. `bash apps/desktop/tools/build-helpers.sh` — rebuilds the three Swift helpers into `src-tauri/bin/`.
5. `pnpm dev` — launches the app. Default project at `~/Movies/Juicer/Default/`.
6. To smoke-test MCP without Claude Desktop: pipe JSON-RPC into `apps/desktop/src-tauri/target/debug/juicer --mcp` (see commits / git log for the exact sequence used to verify the pivot).
