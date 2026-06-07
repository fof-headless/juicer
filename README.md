# Juicer ⚡

**A free, self-contained native Mac app for product-demo animations.** Bring your HTML/React components and brand visuals into a 3D scene, keyframe them like in Blender, and render to MP4 — driven by you or by Claude over MCP.

No Blender. No Python. No browser engine doing the rendering. One `.app`.

---

## What changed (and why)

Earlier drafts leaned on Blender (heavy external dependency, GPLv3, a Python socket bridge) or on a browser/WebGL stack (heavy node_modules, not native). Both were wrong for this.

Juicer now has its **own native rendering engine** written in Rust with `wgpu` (→ Metal on macOS). For the actual use case — HTML/image panels and shapes moving through 3D space with keyframes and camera moves — you don't need Blender's renderer at all. You need a lean, fast, self-contained engine. That's what this is.

### Quality tiers

| Mode | Status | Renderer | For |
|---|---|---|---|
| **Lite** | ✅ **built** | Native wgpu (Metal) | HTML/image panels + shapes in 3D, keyframes, camera moves — Apple-keynote-style demos |
| Standard | planned | wgpu + glTF/PBR | Real 3D product models with materials |
| Pro | planned | Cycles/Blender bridge | Photoreal / path-traced cinematics |

You asked for all three eventually; **Lite is implemented now.**

---

## Architecture

```
┌─────────────────┐   stdio MCP    ┌────────────────────────────────────┐
│  Claude Desktop  │ ◄────────────► │            Juicer.app              │
└─────────────────┘  (juicer --mcp) │  ┌──────────────────────────────┐  │
                                    │  │ Tauri 2 (Rust)                │  │
                                    │  │  • Scene model (scene.rs)     │  │
                                    │  │  • Keyframe engine (anim.rs)  │  │
                                    │  │  • wgpu renderer (render/)    │  │
                                    │  │  • MCP server (mcp.rs)        │  │
                                    │  └──────────────────────────────┘  │
                                    │  React UI in native WebView         │
                                    └──────────────────────────────────┘
                                          │                    │
                              juicer-html-capture        juicer-encoder
                              (WKWebView → PNG)        (PNG seq → MP4, AVFoundation)
```

Everything in one process. Claude talks to the scene **directly** — no IPC hop, no socket bridge.

| Concern | Blender's subsystem | Juicer's native equivalent |
|---|---|---|
| 3D rendering | Eevee/OpenGL | `wgpu` (Metal) — `render/mod.rs` + `shader.wgsl` |
| Keyframes / F-curves | Animation system | `anim.rs` — tracks, easing, interpolation |
| Object/data model | DNA/RNA | `scene.rs` — elements, camera, lights |
| Python API + addons | bpy | MCP server — Claude is the scripting layer |
| HTML → texture | (none) | `juicer-html-capture` (native WKWebView) |
| Video output | ffmpeg | `juicer-encoder` (native AVFoundation, no ffmpeg) |

---

## Prerequisites

```bash
# Xcode CLI tools ONLY — you do NOT need the full Xcode app
xcode-select --install

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install tauri-cli --version "^2.0"

# Node + pnpm
brew install node pnpm
```

That's the whole toolchain. No Blender, no Python.

---

## Build & run

```bash
pnpm install

# Build the two native helper binaries (compiles in ~5s with swiftc)
pnpm --filter juicer-desktop helpers

# Dev mode (hot-reload UI + Rust)
pnpm dev

# Production: Juicer.app + DMG
pnpm build
```

> First run: `cargo tauri icon path/to/logo.png` generates app icons referenced in `tauri.conf.json`.

---

## Connect Claude Desktop

The MCP server *is* the app binary, run with `--mcp`. Add to
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

During dev, point at `target/debug/juicer` (run `pnpm dev` once to build it).

No Python, no addon install, no port juggling — the thing Blender-MCP makes painful.

---

## Using it with Claude

- *"Set up a dark demo layout titled 'Acme' with accent #6644ff"* → `arrange_demo_layout`
- *"Add a plane and put my pricing-card PNG on it at /tmp/card.png"*
- *"Keyframe the title: frame 1 at y=-2 opacity 0, frame 30 at y=0 opacity 1, ease-out"*
- *"Dolly the camera from z=8 to z=4 over the first 60 frames"* → `set_keyframe_camera`
- *"Render frames 1–300 to /tmp/demo.mp4"* → `render_animation`

### MCP tools

| Tool | Action |
|---|---|
| `get_scene` | Full scene JSON — elements, keyframes, camera, render settings |
| `add_element` | plane / box / sphere (plane + `image_path` = your HTML/brand visual) |
| `update_element` | Move, rotate, scale, recolor, opacity, swap image |
| `remove_element` | Delete |
| `set_keyframe` | Keyframe position/rotation/scale/opacity with easing |
| `set_camera` / `set_keyframe_camera` | Static or animated camera |
| `set_render_settings` | Resolution, fps, frame range, background |
| `render_frame` | Single PNG via wgpu |
| `render_animation` | Full MP4 via wgpu + native encoder |
| `arrange_demo_layout` | One-shot starter composition |

---

## HTML → 3D workflow

1. Paste your component's HTML in the **HTML → 3D Plane** panel
2. **Capture** → `juicer-html-capture` renders it via native WKWebView (full CSS3, fonts, gradients)
3. The PNG is applied as a GPU texture on a new plane
4. Position / keyframe it (panel or Claude)
5. **Render MP4**

---

## Project layout

```
apps/desktop/
  src/                      React UI (Tauri WebView)
  src-tauri/
    src/
      scene.rs              scene data model
      anim.rs               keyframe tracks + easing
      render/
        mod.rs              wgpu offscreen renderer
        mesh.rs             plane / box / sphere primitives
        shader.wgsl         vertex+fragment shader
      video.rs              frame sequence → MP4 orchestration
      mcp.rs                MCP stdio server (Claude)
      html_capture.rs       calls juicer-html-capture
      lib.rs                Tauri commands + app state
  tools/
    html-capture/main.swift native WKWebView → PNG
    encoder/main.swift       native AVFoundation → MP4
    build-helpers.sh
```

---

## Roadmap

- [ ] Real-time interactive viewport (render-to-surface in a child window, not on-demand)
- [ ] On-canvas transform gizmos
- [ ] Standard mode: glTF model import + PBR materials
- [ ] Pro mode: optional Cycles bridge for photoreal stills
- [ ] Timeline UI with draggable keyframes + curve editor
- [ ] Windows/Linux: swap WKWebView capture for headless-chromium, AVFoundation for ffmpeg
