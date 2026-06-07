# Juicer ⚡

**A free, web-based product demo animation studio** — build animations from your HTML/React components and brand assets in 3D space, with Blender-like keyframe control, and let Claude drive it through MCP.

Think of it as the love child of Blender's animation system, the browser's HTML rendering, and Premiere Pro's timeline — except it's free, code-first, and AI-controllable.

---

## Why this exists

You build product UI in HTML/React. When you need a demo video, you either:
- screen-record (rigid, can't restyle, can't re-compose), or
- rebuild everything in Premiere/After Effects (expensive, disconnected from your real components).

Juicer lets you take the *actual visual elements* of your product, drop them into a 3D scene as planes/objects, keyframe them on a real timeline, and let Claude rearrange and animate them in 3D space — then record the result to video.

---

## Architecture

```
┌─────────────────┐     stdio (MCP)      ┌──────────────────┐
│  Claude Desktop  │ ◄──────────────────► │   MCP Server     │
└─────────────────┘                       │  (apps/mcp-server)│
                                          └────────┬─────────┘
                                                   │ WebSocket :3001
                                                   ▼
                                          ┌──────────────────┐
                                          │   Editor (web)   │
                                          │  (apps/editor)   │
                                          │                  │
                                          │  React Three     │
                                          │  Fiber viewport  │
                                          │  + Theatre.js    │
                                          │    timeline      │
                                          └──────────────────┘
```

| Layer | Tech | Role |
|---|---|---|
| Editor UI | React + Vite | Panels, outliner, properties (Blender-style) |
| 3D Viewport | React Three Fiber (Three.js) | Render HTML planes & primitives in 3D |
| **Timeline / keyframes** | **Theatre.js Studio** | The Blender-like animation editor — keyframes, curves, sequencing, all in the browser |
| HTML → 3D | SVG `foreignObject` → CanvasTexture | Renders your HTML as a texture on a 3D plane |
| Video export | Canvas `captureStream` + MediaRecorder | Records the viewport to WebM |
| AI control | `@modelcontextprotocol/sdk` | Claude adds/moves/animates elements |

### Why Theatre.js?
The hardest part of "Blender for the web" is a real keyframe editor. **Theatre.js Studio** already is exactly that — it ships a dockable timeline with keyframes, easing curves, and a sequence player that works on any JS object. We bind each scene element to a Theatre object, so Claude (or you) just sets values at times and it interpolates. We didn't reinvent the timeline; we reused the best one that exists.

---

## What we reused (your "clone Blender, reuse components" idea)

Cloning Blender's C++ source wouldn't help here — it's a desktop OpenGL app, not web. Instead we reuse the **web-native equivalents** of each Blender subsystem:

| Blender subsystem | Web equivalent we use |
|---|---|
| Viewport / OpenGL | Three.js / React Three Fiber |
| Dope sheet / Graph editor | Theatre.js Studio |
| Outliner | Custom React tree (`apps/editor/src/outliner`) |
| Properties (N-panel) | Custom React panel (`apps/editor/src/properties`) |
| Transform gizmos | drei `TransformControls` |
| Python API + addons | MCP server (Claude as the scripting layer) |
| Render/output | MediaRecorder canvas capture |

---

## Getting started

```bash
# Install (uses pnpm workspaces — npm/yarn also work)
pnpm install

# Run editor + MCP server together
pnpm dev

# Or separately:
pnpm editor   # → http://localhost:5173
pnpm mcp      # → MCP stdio + WebSocket :3001
```

Open http://localhost:5173. The Theatre.js Studio timeline appears at the bottom/side of the screen automatically.

---

## Connecting Claude Desktop (the part that's a pain with blender-mcp — made simple here)

Add this to your Claude Desktop config:

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "juicer": {
      "command": "node",
      "args": ["/absolute/path/to/juicer/apps/mcp-server/dist/index.js"]
    }
  }
}
```

Then `pnpm --filter mcp-server build` once. Restart Claude Desktop.

That's it — no Python, no Blender addon install, no port juggling. The MCP server boots its own WebSocket bridge; the editor auto-connects when you open it (green badge bottom-right).

> See [`claude_desktop_config.example.json`](./claude_desktop_config.example.json) for a copy-paste version.

---

## Using it with Claude

Once connected, ask Claude things like:

- *"Add my pricing card HTML as a plane and place it center-stage"*
- *"Arrange a product demo layout titled 'Juicer' with a purple accent"*
- *"Move the title up and fade it in over the first 2 seconds"*
- *"What's in the scene right now?"*

Claude calls these MCP tools:

| Tool | What it does |
|---|---|
| `get_scene` | Read current scene state |
| `add_element` | Add box / sphere / text / **html-plane** / image-plane |
| `update_element` | Move, restyle, change content |
| `remove_element` | Delete |
| `select_element` | Highlight + focus properties |
| `play_animation` / `pause_animation` / `seek_animation` | Drive the timeline |
| `arrange_demo_layout` | One-shot cinematic demo composition |
| `create_intro_animation` | Guided keyframe intro |

---

## Workflow

1. **Import** — paste HTML, drop an image, or drop a `.html` file into the left panel.
2. **Compose** — drag elements in 3D with the gizmo, or let Claude arrange them.
3. **Animate** — in the Theatre.js Studio panel, click the ◆ next to any property to keyframe it. Scrub the playhead, change values, keyframe again.
4. **Record** — set FPS + duration in the Export panel, hit Record. It plays the timeline and saves a WebM.

---

## Roadmap / not-yet-done

- [ ] Live React component embedding (currently HTML-string → texture; full DOM-in-3D via `@react-three/drei` `<Html>` is wired but not keyframe-recordable yet)
- [ ] Camera keyframing as a first-class Theatre object
- [ ] GIF / MP4 export (currently WebM; transcode via ffmpeg.wasm)
- [ ] Per-element easing presets exposed in properties panel
- [ ] Scene save/load to JSON

---

## License

Free and open. Build your demos, keep your money.
