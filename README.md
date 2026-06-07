# Juicer ⚡

**Native Mac app for product demo animations.** Uses Blender's Eevee GPU renderer as the actual rendering engine — not a browser canvas, not WebGL. Real 3D, real keyframes, real render quality. Free.

---

## The concept

You build product UI in HTML/React. You want a demo video. You don't want to:
- screen-record (rigid, can't restyle or recompose)
- rebuild in Premiere/After Effects (expensive, disconnected from your real components)
- deal with Blender's painful MCP addon setup

Juicer takes your **actual HTML components**, captures them pixel-perfect via macOS native WebKit, maps them as GPU textures onto Blender planes, and gives you a clean UI to position, keyframe, and render — all controlled by Claude Desktop through MCP.

---

## Architecture

```
┌─────────────────┐    stdio MCP     ┌──────────────────────────────────┐
│  Claude Desktop  │ ◄──────────────► │         Juicer.app               │
└─────────────────┘                   │  ┌────────────────────────────┐  │
                                      │  │  Tauri (Rust native shell)  │  │
                                      │  │  • Manages Blender process  │  │
                                      │  │  • Runs MCP server inline   │  │
                                      │  │  • HTML→PNG via WebKit      │  │
                                      │  └──────────┬─────────────────┘  │
                                      │             │ TCP :6789           │
                                      │  ┌──────────▼─────────────────┐  │
                                      │  │  Blender (headless)         │  │
                                      │  │  juicer_bridge.py           │  │
                                      │  │  • Eevee GPU renderer       │  │
                                      │  │  • Real keyframe system     │  │
                                      │  │  • MP4 / PNG sequence out   │  │
                                      │  └────────────────────────────┘  │
                                      └──────────────────────────────────┘
```

| Layer | What it is |
|---|---|
| **Juicer.app** | Tauri 2 native Mac app — no Electron, no bundled Chromium, ~8MB overhead |
| **UI** | React (in macOS WebView) — outliner, properties, HTML importer, keyframe panel |
| **Blender bridge** | Python TCP server running *inside* Blender's interpreter |
| **Renderer** | Blender Eevee (GPU, real-time quality) or Cycles (path tracing) |
| **HTML capture** | Swift binary using WKWebView offscreen — full CSS3, custom fonts, zero deps |
| **MCP** | Built into the Tauri process (stdin/stdout JSON-RPC, no separate server) |

---

## Prerequisites

```bash
# 1. Xcode CLI tools (NO full Xcode needed — just the CLI)
xcode-select --install

# 2. Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 3. Tauri CLI
cargo install tauri-cli --version "^2.0"

# 4. Node + pnpm
brew install node pnpm

# 5. Blender 4.0+ (Eevee Next renderer)
brew install --cask blender
# or download from blender.org
```

---

## Build & run

```bash
# Install JS deps
pnpm install

# Build the HTML capture helper (Swift, compiles in ~5s)
swiftc apps/desktop/tools/html-capture/main.swift \
    -o apps/desktop/src-tauri/bin/juicer-html-capture \
    -framework WebKit -framework AppKit

# Dev mode (hot-reload UI + Rust backend)
pnpm dev

# Production build → Juicer.app + DMG
pnpm build
```

---

## Connect Claude Desktop

The MCP server is built into Juicer.app itself — no separate process. Add to Claude Desktop config:

**`~/Library/Application Support/Claude/claude_desktop_config.json`**
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

> During dev, use the compiled binary path: `target/release/juicer`

---

## Using with Claude

With Blender connected and Juicer running, ask Claude:

- *"Set up a dark product demo layout with my brand color #6644ff and title 'Acme'"*
- *"Add a plane at position 0,0,0 and apply this HTML to it: `<div...>`"*
- *"Set a keyframe on Title at frame 1 with location [0, -3, 0] and at frame 30 with location [0, 0, 0]"*
- *"Render the animation from frame 1 to 300 at 30fps to /tmp/demo.mp4"*

### MCP tools

| Tool | What Blender does |
|---|---|
| `get_scene` | Returns all Blender objects with transforms + keyframes |
| `add_element` | `bpy.ops.mesh.primitive_*_add`, text objects, image planes |
| `update_element` | Set location/rotation/scale/visibility/material |
| `remove_element` | `bpy.data.objects.remove` |
| `set_keyframe` | `obj.keyframe_insert(data_path=..., frame=...)` — Blender's native keyframe system |
| `play_animation` / `seek_animation` | Advance Blender's scene frame |
| `render_frame` | `bpy.ops.render.render(write_still=True)` via Eevee |
| `render_animation` | `bpy.ops.render.render(animation=True)` → MP4 |
| `arrange_demo_layout` | Multi-op: background, title, content plane, accent geometry |

---

## HTML → Blender workflow

1. Paste your component's rendered HTML in the "HTML → 3D Plane" panel
2. Click **Capture** — the Swift helper renders it at your chosen resolution using macOS WebKit
3. The PNG is automatically applied as a texture on a new Blender plane
4. Position it in 3D using the Properties panel or via Claude
5. Keyframe the position/opacity for animation
6. Render

The capture is done in a native `WKWebView` offscreen window — it supports full CSS including `backdrop-filter`, CSS variables, custom Google Fonts (if loaded), SVG, etc.

---

## Why Tauri, not Electron

| | Electron | Tauri |
|---|---|---|
| Bundle size | ~85MB (bundled Chromium) | ~8MB (uses system WebView) |
| Memory | ~150MB at idle | ~20MB at idle |
| macOS WebKit | No | **Yes — uses native Safari engine** |
| Rust backend | No | **Yes — direct process management, no Node overhead** |

---

## Roadmap

- [ ] Embed Blender's OOTB viewport directly (requires window-mode Blender build — possible via XPC on Mac)
- [ ] Camera keyframing as a first-class MCP tool
- [ ] Eevee real-time preview streamed to the viewport panel (render to shared memory)
- [ ] Scene save/load to `.juicer` JSON
- [ ] ProRes / Apple Animation codec export (via Blender's ffmpeg backend)
- [ ] Multiple sheets / sequences (intro → demo → outro)
