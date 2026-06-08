//! MCP server (stdio) for Claude Desktop.
//!
//! Operates directly on the native scene — no IPC, no Blender, no Python.
//! Run via `juicer --mcp`. Claude adds elements, sets keyframes, renders.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

use crate::scene::Element;
use crate::{apply_patch, parse_easing, parse_keyvalue, parse_kind, AppState};
use crate::render::Renderer;
use crate::project::Project;
use crate::{html_capture, video};
use std::sync::Arc;

/// Tools that change the scene and should trigger an autosave afterward.
fn is_mutation(tool: &str) -> bool {
    matches!(
        tool,
        "add_element" | "update_element" | "remove_element" | "set_keyframe"
            | "set_camera" | "set_keyframe_camera" | "set_render_settings"
            | "arrange_demo_layout" | "capture_html"
    )
}

pub async fn run_stdio_server(state: Arc<AppState>) -> Result<()> {
    // Everything has a home on disk from the first call.
    state.ensure_default_project().await;

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req["method"].as_str().unwrap_or("");

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "juicer", "version": "0.1.0" }
                }
            }),
            "notifications/initialized" => continue,
            "tools/list" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "tools": tool_definitions() }
            }),
            "tools/call" => {
                let tool = req["params"]["name"].as_str().unwrap_or("").to_string();
                let args = req["params"]["arguments"].clone();
                let content = handle_tool(&tool, &args, &state).await;
                // Persist the scene after any mutation (guards from handle_tool
                // are dropped by now, so this won't deadlock).
                if is_mutation(&tool) {
                    state.autosave().await;
                }
                json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "content": content }
                })
            }
            _ => json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": "Method not found" }
            }),
        };

        let mut out = serde_json::to_string(&response)?;
        out.push('\n');
        stdout.write_all(out.as_bytes())?;
        stdout.flush()?;
    }
    Ok(())
}

fn text_content(s: impl Into<String>) -> Vec<Value> {
    vec![json!({ "type": "text", "text": s.into() })]
}

async fn handle_tool(tool: &str, args: &Value, state: &Arc<AppState>) -> Vec<Value> {
    match tool {
        "get_scene" => {
            let scene = state.scene.lock().await;
            text_content(serde_json::to_string_pretty(&*scene).unwrap_or_default())
        }

        "add_element" => {
            // Copy any external image into the project's assets/ (lock project
            // before scene to keep a consistent project → scene lock order).
            let imported = if let Some(src) = args["image_path"].as_str() {
                let proj = state.project.lock().await;
                proj.as_ref().and_then(|p| p.import_asset(src).ok())
            } else {
                None
            };
            let mut scene = state.scene.lock().await;
            let id = scene.alloc_id();
            let kind = parse_kind(args["type"].as_str());
            let name = args["name"].as_str().unwrap_or("Element").to_string();
            let mut el = Element::new(id.clone(), name, kind);
            apply_patch(&mut el, args);
            if let Some(p) = args.get("position").and_then(|v| v.as_array()) {
                el.position = vec3(p, el.position);
            }
            if let Some(path) = imported {
                el.image_path = Some(path);
            }
            scene.elements.push(el);
            text_content(format!("Added element '{id}'"))
        }

        "update_element" => {
            let mut scene = state.scene.lock().await;
            let name = args["name"].as_str().or(args["id"].as_str()).unwrap_or("");
            match scene.element_mut(name) {
                Some(el) => { apply_patch(el, args); text_content(format!("Updated '{name}'")) }
                None => text_content(format!("Element '{name}' not found")),
            }
        }

        "remove_element" => {
            let mut scene = state.scene.lock().await;
            let name = args["name"].as_str().or(args["id"].as_str()).unwrap_or("");
            text_content(if scene.remove(name) { format!("Removed '{name}'") } else { format!("'{name}' not found") })
        }

        "set_keyframe" => {
            let mut scene = state.scene.lock().await;
            let name = args["name"].as_str().or(args["id"].as_str()).unwrap_or("");
            let frame = args["frame"].as_f64().unwrap_or(1.0) as f32;
            let property = args["property"].as_str().unwrap_or("position").to_string();
            let ease = parse_easing(args["easing"].as_str());
            let kv = match parse_keyvalue(&property, &args["value"]) {
                Ok(v) => v,
                Err(e) => return text_content(format!("Bad keyframe value: {e}")),
            };
            match scene.element_mut(name) {
                Some(el) => { el.track_mut(&property).insert(frame, kv, ease); text_content(format!("Keyframed '{name}' {property} @ frame {frame}")) }
                None => text_content(format!("Element '{name}' not found")),
            }
        }

        "set_camera" => {
            let mut scene = state.scene.lock().await;
            if let Some(p) = args.get("position").and_then(|v| v.as_array()) {
                scene.camera.position = vec3(p, scene.camera.position);
            }
            if let Some(t) = args.get("target").and_then(|v| v.as_array()) {
                scene.camera.target = vec3(t, scene.camera.target);
            }
            if let Some(f) = args["fov_deg"].as_f64() { scene.camera.fov_deg = f as f32; }
            text_content("Camera updated")
        }

        "set_keyframe_camera" => {
            let mut scene = state.scene.lock().await;
            let frame = args["frame"].as_f64().unwrap_or(1.0) as f32;
            let property = args["property"].as_str().unwrap_or("position").to_string();
            let ease = parse_easing(args["easing"].as_str());
            let kv = match parse_keyvalue(&property, &args["value"]) {
                Ok(v) => v,
                Err(e) => return text_content(format!("Bad value: {e}")),
            };
            let track = if let Some(nt) = scene.camera.tracks.iter_mut().find(|t| t.property == property) {
                &mut nt.track
            } else {
                scene.camera.tracks.push(crate::scene::NamedTrack { property: property.clone(), track: Default::default() });
                &mut scene.camera.tracks.last_mut().unwrap().track
            };
            track.insert(frame, kv, ease);
            text_content(format!("Camera {property} keyframed @ {frame}"))
        }

        "set_render_settings" => {
            let mut scene = state.scene.lock().await;
            if let Some(w) = args["width"].as_u64() { scene.render.width = w as u32; }
            if let Some(h) = args["height"].as_u64() { scene.render.height = h as u32; }
            if let Some(f) = args["fps"].as_u64() { scene.render.fps = f as u32; }
            if let Some(s) = args["frame_start"].as_u64() { scene.render.frame_start = s as u32; }
            if let Some(e) = args["frame_end"].as_u64() { scene.render.frame_end = e as u32; }
            if let Some(bg) = args.get("background").and_then(|v| v.as_array()) {
                if bg.len() == 4 {
                    scene.render.background = [
                        bg[0].as_f64().unwrap_or(0.0) as f32,
                        bg[1].as_f64().unwrap_or(0.0) as f32,
                        bg[2].as_f64().unwrap_or(0.0) as f32,
                        bg[3].as_f64().unwrap_or(1.0) as f32,
                    ];
                }
            }
            text_content("Render settings updated")
        }

        "render_frame" => {
            let frame = args["frame"].as_f64().unwrap_or(1.0) as f32;
            // Default the saved PNG into the project's renders/ folder.
            let out = match args["output_path"].as_str() {
                Some(p) => Some(p.to_string()),
                None => {
                    let proj = state.project.lock().await;
                    proj.as_ref().map(|p| p.render_path(&format!("frame_{:05}", frame as u32), "png"))
                }
            };
            let scene = state.scene.lock().await.clone();
            let mut rg = state.renderer.lock().await;
            if rg.is_none() {
                match Renderer::new() {
                    Ok(r) => *rg = Some(r),
                    Err(e) => return text_content(format!("GPU init failed: {e}")),
                }
            }
            let r = rg.as_mut().unwrap();
            // Always render to bytes so we can embed the image in the response.
            let png_bytes = match r.render_to_png_bytes(&scene, frame) {
                Ok(b) => b,
                Err(e) => return text_content(format!("Render error: {e}")),
            };
            // Also write to disk.
            if let Some(path) = &out {
                if let Err(e) = r.render_to_png(&scene, frame, path) {
                    return text_content(format!("Render error writing to '{path}': {e}"));
                }
            }
            let save_msg = out.as_deref().map(|p| format!(" Saved to {p}.")).unwrap_or_default();
            let b64 = base64_encode(&png_bytes);
            vec![
                json!({ "type": "text", "text": format!("Rendered frame {frame}.{save_msg}") }),
                json!({ "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": b64 } }),
            ]
        }

        "render_animation" => {
            // Default the MP4 into the project's renders/ folder.
            let out = match args["output_path"].as_str() {
                Some(p) => p.to_string(),
                None => {
                    let proj = state.project.lock().await;
                    match proj.as_ref() {
                        Some(p) => p.render_path("output", "mp4"),
                        None => "/tmp/juicer_demo".to_string(),
                    }
                }
            };
            let scene = state.scene.lock().await.clone();
            let mut rg = state.renderer.lock().await;
            if rg.is_none() {
                match Renderer::new() {
                    Ok(r) => *rg = Some(r),
                    Err(e) => return text_content(format!("GPU init failed: {e}")),
                }
            }
            match video::render_animation(rg.as_mut().unwrap(), &scene, &out) {
                Ok(path) => text_content(format!("Rendered animation → {path}")),
                Err(e) => text_content(format!("Render error: {e}")),
            }
        }

        "capture_html" => capture_html(args, state).await,

        "create_project" => {
            let name = args["name"].as_str().unwrap_or("Untitled");
            match Project::create(name) {
                Ok(p) => {
                    *state.scene.lock().await = crate::scene::Scene::default();
                    let _ = p.save_scene(&*state.scene.lock().await);
                    let root = p.root.to_string_lossy().to_string();
                    *state.project.lock().await = Some(p);
                    text_content(format!(
                        "Created project '{name}' at {root}\n\
                         Scenes, captured HTML (assets/), and renders/ all live here."
                    ))
                }
                Err(e) => text_content(format!("Could not create project: {e}")),
            }
        }

        "open_project" => {
            let path = args["path"].as_str().unwrap_or("");
            match Project::open(path) {
                Ok((p, scene)) => {
                    if let Some(loaded) = scene {
                        *state.scene.lock().await = loaded;
                    }
                    let root = p.root.to_string_lossy().to_string();
                    *state.project.lock().await = Some(p);
                    text_content(format!("Opened project at {root}"))
                }
                Err(e) => text_content(format!("Could not open project: {e}")),
            }
        }

        "save_project" => {
            let proj = state.project.lock().await;
            match proj.as_ref() {
                Some(p) => {
                    match p.save_scene(&*state.scene.lock().await) {
                        Ok(()) => text_content(format!("Saved → {}", p.scene_path().to_string_lossy())),
                        Err(e) => text_content(format!("Save failed: {e}")),
                    }
                }
                None => text_content("No active project. Use create_project first."),
            }
        }

        "get_project" => {
            let proj = state.project.lock().await;
            match proj.as_ref() {
                Some(p) => text_content(format!(
                    "Active project '{}'\nroot: {}\nassets: {}\nrenders: {}",
                    p.name,
                    p.root.to_string_lossy(),
                    p.assets_dir().to_string_lossy(),
                    p.renders_dir().to_string_lossy(),
                )),
                None => text_content("No active project."),
            }
        }

        "arrange_demo_layout" => arrange_demo_layout(args, state).await,

        _ => text_content(format!("Unknown tool: {tool}")),
    }
}

/// Capture HTML (with Tailwind/fonts/icons auto-injected) to a PNG in the
/// project's assets/ folder, and optionally add it to the scene as a plane.
async fn capture_html(args: &Value, state: &Arc<AppState>) -> Vec<Value> {
    let html = args["html"].as_str().unwrap_or("");
    if html.trim().is_empty() {
        return text_content("capture_html needs an 'html' string.");
    }
    let name = args["name"].as_str().unwrap_or("capture").to_string();
    let width = args["width"].as_u64().unwrap_or(1200) as u32;
    let height = args["height"].as_u64().unwrap_or(800) as u32;
    let font = args["font"].as_str();
    let libraries: Vec<String> = args["libraries"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let add_plane = args["add_plane"].as_bool().unwrap_or(true);

    // Output path inside the project's assets/.
    let out = {
        let proj = state.project.lock().await;
        match proj.as_ref() {
            Some(p) => p.asset_path(&name, "png"),
            None => std::env::temp_dir().join(format!("{name}.png")).to_string_lossy().to_string(),
        }
    };

    let wrapped = html_capture::wrap_html(html, &libraries, font);
    if let Err(e) = html_capture::capture_html_to_png(&wrapped, width, height, &out).await {
        return text_content(format!("HTML capture failed: {e}"));
    }

    if add_plane {
        // Add a plane sized to the capture's aspect ratio, textured with the PNG.
        let aspect = width as f32 / height.max(1) as f32;
        let mut scene = state.scene.lock().await;
        let id = scene.alloc_id();
        let mut el = Element::new(id.clone(), name.clone(), crate::scene::ElementKind::Plane);
        el.image_path = Some(out.clone());
        el.width = 3.0;
        el.height = 3.0 / aspect.max(0.01);
        el.unlit = true;
        scene.elements.push(el);
        return text_content(format!(
            "Captured '{name}' → {out}\nAdded plane '{id}' (image_path set, {:.2} aspect). \
             Keyframe or reposition it next.",
            aspect
        ));
    }

    text_content(format!(
        "Captured '{name}' → {out}\nUse add_element type=plane image_path={out} to place it."
    ))
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[(n >> 18) & 63] as char);
        out.push(CHARS[(n >> 12) & 63] as char);
        out.push(if chunk.len() > 1 { CHARS[(n >> 6) & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[n & 63] as char } else { '=' });
    }
    out
}

async fn arrange_demo_layout(args: &Value, state: &Arc<AppState>) -> Vec<Value> {
    let color = args["accentColor"].as_str().unwrap_or("#6644ff").to_string();
    let title = args["title"].as_str().unwrap_or("Your Product").to_string();

    let mut scene = state.scene.lock().await;
    scene.render.background = [0.03, 0.03, 0.06, 1.0];

    // Content plane (user maps captured HTML onto this)
    let id_plane = scene.alloc_id();
    let mut plane = Element::new(id_plane.clone(), "ContentPlane".into(), crate::scene::ElementKind::Plane);
    plane.position = [0.0, 0.4, 0.0];
    plane.width = 4.0;
    plane.height = 2.4;
    plane.color = "#10101e".into();
    scene.elements.push(plane);

    // Left accent bar
    let id_l = scene.alloc_id();
    let mut barl = Element::new(id_l, "AccentL".into(), crate::scene::ElementKind::Box);
    barl.position = [-2.4, 0.4, -0.1];
    barl.scale = [0.06, 2.4, 0.06];
    barl.color = color.clone();
    scene.elements.push(barl);

    // Right accent bar
    let id_r = scene.alloc_id();
    let mut barr = Element::new(id_r, "AccentR".into(), crate::scene::ElementKind::Box);
    barr.position = [2.4, 0.4, -0.1];
    barr.scale = [0.06, 2.4, 0.06];
    barr.color = color.clone();
    scene.elements.push(barr);

    text_content(format!(
        "Created '{title}' demo layout: ContentPlane (id {id_plane}), two accent bars in {color}.\n\n\
         Next:\n\
         1. Use the HTML importer (or capture_html tool) to render your brand HTML to a PNG.\n\
         2. update_element ContentPlane with image_path=<that png>.\n\
         3. set_keyframe on ContentPlane: frame 1 opacity 0 + position [0,-1,0], frame 30 opacity 1 + position [0,0.4,0].\n\
         4. render_animation to export MP4."
    ))
}

fn vec3(arr: &[Value], fallback: [f32; 3]) -> [f32; 3] {
    if arr.len() != 3 { return fallback; }
    [
        arr[0].as_f64().unwrap_or(fallback[0] as f64) as f32,
        arr[1].as_f64().unwrap_or(fallback[1] as f64) as f32,
        arr[2].as_f64().unwrap_or(fallback[2] as f64) as f32,
    ]
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "get_scene",
            "description": "Get the full Juicer scene — all elements, transforms, keyframes, camera, and render settings.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "add_element",
            "description": "Add an element. Types: plane (flat quad for HTML/images), box, sphere. Use plane + image_path to show captured HTML/brand visuals.",
            "inputSchema": {
                "type": "object",
                "required": ["type", "name"],
                "properties": {
                    "type": { "type": "string", "enum": ["plane", "box", "sphere"] },
                    "name": { "type": "string" },
                    "position": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "rotation": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "Euler XYZ radians" },
                    "scale": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "color": { "type": "string", "description": "Hex #rrggbb" },
                    "opacity": { "type": "number", "minimum": 0, "maximum": 1 },
                    "image_path": { "type": "string", "description": "PNG/JPG to texture a plane" },
                    "width": { "type": "number" },
                    "height": { "type": "number" }
                }
            }
        }),
        json!({
            "name": "update_element",
            "description": "Modify an existing element by name: move, rotate, scale, recolor, set opacity/visibility, swap image_path.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "position": { "type": "array", "items": { "type": "number" } },
                    "rotation": { "type": "array", "items": { "type": "number" } },
                    "scale": { "type": "array", "items": { "type": "number" } },
                    "color": { "type": "string" },
                    "opacity": { "type": "number" },
                    "visible": { "type": "boolean" },
                    "image_path": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "remove_element",
            "description": "Delete an element by name.",
            "inputSchema": { "type": "object", "required": ["name"], "properties": { "name": { "type": "string" } } }
        }),
        json!({
            "name": "set_keyframe",
            "description": "Insert a keyframe on an element. Animate position/rotation/scale (vec3) or opacity (number) over frames. Easing: linear, ease-in, ease-out, ease-in-out, step.",
            "inputSchema": {
                "type": "object",
                "required": ["name", "frame", "property", "value"],
                "properties": {
                    "name": { "type": "string" },
                    "frame": { "type": "number" },
                    "property": { "type": "string", "enum": ["position", "rotation", "scale", "opacity"] },
                    "value": {
                        "description": "[x,y,z] for position/rotation/scale, or a number for opacity",
                        "oneOf": [
                            { "type": "number" },
                            { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }
                        ]
                    },
                    "easing": { "type": "string", "enum": ["linear", "ease-in", "ease-out", "ease-in-out", "step"] }
                }
            }
        }),
        json!({
            "name": "set_camera",
            "description": "Set the camera position, look-at target, and field of view.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "position": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "target": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "fov_deg": { "type": "number" }
                }
            }
        }),
        json!({
            "name": "set_keyframe_camera",
            "description": "Keyframe the camera for cinematic moves. property: position or target.",
            "inputSchema": {
                "type": "object",
                "required": ["frame", "property", "value"],
                "properties": {
                    "frame": { "type": "number" },
                    "property": { "type": "string", "enum": ["position", "target"] },
                    "value": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "easing": { "type": "string", "enum": ["linear", "ease-in", "ease-out", "ease-in-out", "step"] }
                }
            }
        }),
        json!({
            "name": "set_render_settings",
            "description": "Set resolution, fps, frame range, and background color.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "width": { "type": "number" },
                    "height": { "type": "number" },
                    "fps": { "type": "number" },
                    "frame_start": { "type": "number" },
                    "frame_end": { "type": "number" },
                    "background": { "type": "array", "items": { "type": "number" }, "minItems": 4, "maxItems": 4, "description": "[r,g,b,a] 0-1" }
                }
            }
        }),
        json!({
            "name": "render_frame",
            "description": "Render a single frame with the native GPU renderer. Returns the image inline AND saves a PNG to the project's renders/ folder (override with output_path).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "frame": { "type": "number" },
                    "output_path": { "type": "string", "description": "Optional. Defaults to <project>/renders/frame_NNNNN.png" }
                }
            }
        }),
        json!({
            "name": "render_animation",
            "description": "Render the full frame range to an MP4. Saves to the project's renders/ folder by default (override with output_path).",
            "inputSchema": {
                "type": "object",
                "properties": { "output_path": { "type": "string", "description": "Optional. Defaults to <project>/renders/output.mp4" } }
            }
        }),
        json!({
            "name": "capture_html",
            "description": "Render HTML/CSS to a transparent PNG via native WebKit and (by default) add it as a plane in the scene. Tailwind, Google Fonts, Lucide icons, Animate.css and Font Awesome are auto-injected — send raw component markup with Tailwind classes and it just works. Saves into the project's assets/ folder. This is how you bring brand visuals / UI into 3D.",
            "inputSchema": {
                "type": "object",
                "required": ["html"],
                "properties": {
                    "html": { "type": "string", "description": "HTML fragment (Tailwind classes OK) or a full document." },
                    "name": { "type": "string", "description": "Asset/element name (default 'capture')." },
                    "width": { "type": "number", "description": "Capture width px (default 1200)." },
                    "height": { "type": "number", "description": "Capture height px (default 800)." },
                    "font": { "type": "string", "description": "Google Font family to load (default 'Inter')." },
                    "libraries": { "type": "array", "items": { "type": "string" }, "description": "Extra CSS/JS CDN URLs to inject." },
                    "add_plane": { "type": "boolean", "description": "Add a textured plane to the scene (default true)." }
                }
            }
        }),
        json!({
            "name": "create_project",
            "description": "Create a new project folder (~/Movies/Juicer/<name>) with assets/ and renders/, and make it active. Scenes auto-save here. Start here for a new demo.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": { "name": { "type": "string" } }
            }
        }),
        json!({
            "name": "open_project",
            "description": "Open an existing project folder by absolute path and load its scene.json.",
            "inputSchema": {
                "type": "object",
                "required": ["path"],
                "properties": { "path": { "type": "string" } }
            }
        }),
        json!({
            "name": "save_project",
            "description": "Explicitly save the current scene to the active project's scene.json (scenes also auto-save after every change).",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_project",
            "description": "Show the active project's name and folder paths (assets/, renders/).",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "arrange_demo_layout",
            "description": "One-shot: build a starter product-demo composition (content plane + accent bars + dark background). Best first call for a new demo.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "accentColor": { "type": "string" }
                }
            }
        }),
    ]
}
