# Juicer ⚡

**Native macOS app for Figma-style flat-with-shadows motion design — product launch / product demo videos — driven by Claude over MCP.**

Paste HTML / a screenshot / a design-system mockup into Claude, Claude builds the animated scene atomically over MCP, Claude takes preview screenshots and iterates, you export an MP4. No Blender. No browser-engine-driven renderer to wrestle with. One `.app`.

---

## What this is

A 2.5D layer compositor with a Blender-grade keyframe engine. The key idea: **the browser is the renderer**. CSS already does shadows, blurs, gradients, border-radius, transforms (including 3D perspective), text rendering, and z-order compositing better than anything we'd write from scratch — so we piggyback on it. One offscreen WKWebView, layers as DOM siblings, HTML layers isolated via `<iframe srcdoc>` so user CSS cannot leak between layers.

| Layer type | What it is |
|---|---|
| **HTML** | Your component's HTML/Tailwind, rendered in an isolated iframe |
| **Image** | PNG/JPG/WebP from disk |
| **Shape** | Rect or ellipse with fill (solid/linear/radial gradient), stroke, border-radius |
| **Text** | Styled text — family / size / weight / letter-spacing / color / align |

Every layer has:

- 2D transform: `x`, `y`, `rotation`, `scale_x`, `scale_y`
- Optional 2.5D tilt: `rotate_x`, `rotate_y`, `perspective` (for floating-card / Apple-keynote vibes)
- `opacity`, `blend_mode` (CSS mix-blend-mode)
- Effects: stacked box-shadows (Figma-style drop shadows), CSS-filter blur
- Per-property **keyframe tracks** with cubic-bezier handles (Blender-grade F-curves)

Claude controls all of it atomically via MCP. ~40 tools.

---

## Architecture

```
┌─────────────────┐   stdio MCP    ┌───────────────────────────────────────┐
│  Claude Desktop  │ ◄────────────► │            Juicer.app                 │
└─────────────────┘  (juicer --mcp) │  ┌─────────────────────────────────┐  │
                                    │  │ Tauri 2 (Rust)                  │  │
                                    │  │  • Scene model (scene.rs)       │  │
                                    │  │  • Keyframe engine (anim.rs)    │  │
                                    │  │  • Renderer IPC (renderer.rs)   │  │
                                    │  │  • MCP server (mcp.rs)          │  │
                                    │  └─────────────────────────────────┘  │
                                    │  React UI in native WebView           │
                                    │  (viewport = iframe of renderer.html) │
                                    └────────────────┬──────────────────────┘
                                                     │ stdio JSON
                                                     ▼
                            ┌────────────────────────────────────────────┐
                            │  juicer-frame-renderer (Swift, long-lived) │
                            │  one offscreen WKWebView + renderer.html;  │
                            │  applies per-frame styles, snapshots PNG   │
                            └────────────────┬───────────────────────────┘
                                             │
                                             ▼ frame PNGs
                                  juicer-encoder (AVFoundation → MP4)
```

The same `renderer.html` runs in two places:

1. **As an iframe in the Tauri UI** — that's your live preview, byte-identical to what the recorded MP4 will look like.
2. **In the offscreen WKWebView** held by `juicer-frame-renderer` — captures one PNG per frame for the MP4.

Single source of truth: a `scene.json` per project, autosaved on every mutation.

---

## Prerequisites (macOS only)

```bash
# Xcode CLI tools ONLY — you do NOT need the full Xcode app
xcode-select --install

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install tauri-cli --version "^2.0"

# Node + pnpm
brew install node pnpm
```

That's the whole toolchain.

---

## Build & run

```bash
pnpm install

# Build the three native helper binaries (compile in ~5s with swiftc)
pnpm --filter juicer-desktop helpers

# Dev mode (hot-reload UI + Rust)
pnpm dev

# Production: Juicer.app + DMG
pnpm build
```

> First run: `cargo tauri icon path/to/logo.png` generates app icons referenced in `tauri.conf.json`.

---

## Connect Claude Desktop

The MCP server is the app binary, run with `--mcp`. Add to
`~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "juicer": {
      "command": "/Applications/Juicer.app/Contents/MacOS/juicer",
      "args": ["--mcp"]
    }
  }
}
```

During dev, point at `apps/desktop/src-tauri/target/debug/juicer` (run `pnpm dev` once to build it).

---

## Using it with Claude

```
"Add a Tailwind pricing card as an HTML layer"
"Add a soft drop-shadow rectangle behind it"
"Animate the card y from -300 to 0 over frames 0..30 with an ease-back overshoot"
"Tilt the card rotate_y to 15° on frame 60"
"Render the MP4"
```

### MCP tools (highlights)

**Layer CRUD:** `add_html_layer`, `add_image_layer`, `add_shape_layer`, `add_text_layer`, `remove_layer`, `duplicate_layer`, `reorder_layer`, `list_layers`, `get_layer`, `rename_layer`

**Per-property setters:** `set_transform`, `set_opacity`, `set_size`, `set_border_radius`, `set_shadow`, `clear_shadows`, `set_blur`, `set_fill`, `set_stroke`, `set_text`, `set_font`, `set_html`, `set_image_src`, `set_blend_mode`, `set_visible`

**Keyframing:** `set_keyframe(id, frame, property, value, easing | bezier)` — easings: `linear` / `step` / `ease-in` / `ease-out` / `ease-in-out` / `ease-back`, or pass `bezier: [p1x, p1y, p2x, p2y]` for any custom Blender-style F-curve. `remove_keyframe`, `clear_track`, `copy_track`.

**Animatable properties:** `x`, `y`, `rotation`, `scale_x`, `scale_y`, `rotate_x`, `rotate_y`, `perspective`, `opacity`, `width`, `height`, `border_radius`, `shadow_offset_x/y`, `shadow_blur`, `shadow_spread`, `shadow_color`, `filter_blur`, `fill_color`, `text_content` (step-only), `font_size`.

**Canvas / timeline:** `set_canvas`, `set_duration`

**Render:** `render_frame(frame)` returns an inline PNG + saves to disk; `render_animation` produces an MP4. `evaluate_at(frame)` returns the resolved per-layer CSS without rendering.

**Project:** `create_project`, `open_project`, `save_project`, `get_project`

---

## Projects on disk

```
~/Movies/Juicer/<name>/
  scene.json     ← the whole scene; auto-saved after every mutation
  assets/        ← imported images / captured HTML PNGs
  renders/       ← frame PNGs and exported MP4s
```

On first launch Juicer opens-or-creates a **Default** project, so there's always a home on disk and a restart restores your scene. Ask Claude to *"create a project called Acme"* to start a clean one.

---

## Why layers can't break each other

The thing you'd worry about: a Tailwind layer with `*{color:red}` clobbering another layer's styling. Doesn't happen. Each HTML layer is rendered inside an `<iframe srcdoc>` with its own browsing context. Styles inside the iframe stay inside the iframe. Outside layers (image, shape, text) are siblings in the parent DOM. They can't reach each other.

---

## Tailwind & web libraries (HTML layers)

`add_html_layer` injects Tailwind, Google Fonts (Inter by default), Lucide, Animate.css, and Font Awesome automatically into the iframe srcdoc. Paste raw component markup with Tailwind classes and it renders correctly. Tailwind v4 browser build is vendored at `apps/desktop/src-tauri/resources/tailwind.js`, so captures work fully offline.

---

## Project layout

```
apps/desktop/
  src/                         React UI (Tauri WebView)
    components/                Toolbar, Sidebar (layers), Properties, Viewport, Timeline, HtmlImporter
    store/scene.ts             Zustand layer-based store
  src-tauri/
    src/
      scene.rs                 Scene + Layer (2.5D) data model, evaluate_at()
      anim.rs                  Keyframe tracks + cubic-bezier easing + OKLCH color lerp
      renderer.rs              Rust ↔ juicer-frame-renderer IPC over stdio
      mcp.rs                   MCP stdio server; wraps the shared dispatcher
      lib.rs                   AppState + dispatch_tool (single source of truth for both MCP and UI)
      html_capture.rs          wrap_html + one-shot WKWebView capture
      video.rs                 Frame PNGs → MP4 via juicer-encoder
      project.rs               ~/Movies/Juicer/<name>/ + autosave
    resources/
      tailwind.js              vendored Tailwind (offline)
      renderer.html            DOM shell loaded by the helper + UI iframe
      runtime.js               applyState bridge inside renderer.html
  tools/
    html-capture/main.swift    juicer-html-capture (one-shot HTML → transparent PNG)
    frame-renderer/main.swift  juicer-frame-renderer (long-lived WKWebView + JSON-RPC)
    encoder/main.swift         juicer-encoder (PNG sequence → MP4 via AVFoundation)

blender-fork/                  Reference-only — see blender-fork/README.md (NOT built)
```

---

## Roadmap

- [x] Projects on disk + autosave
- [x] HTML / Image / Shape / Text layers, 2.5D transforms, drop shadows, blur, gradients
- [x] Blender-grade cubic-bezier F-curve easing
- [x] OKLCH color interpolation (no muddy RGB midpoints)
- [x] Live preview iframe = recorded MP4 (single render path)
- [x] Atomic MCP control surface (~40 tools)
- [ ] On-canvas transform gizmos (move/rotate/scale handles)
- [ ] F-curve graph editor in the timeline
- [ ] Path / vector layers + boolean ops
- [ ] Windows/Linux: swap WKWebView for headless-chromium, AVFoundation for ffmpeg
