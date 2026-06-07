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
use crate::video;
use std::sync::Arc;

pub async fn run_stdio_server(state: Arc<AppState>) -> Result<()> {
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
                let tool = req["params"]["name"].as_str().unwrap_or("");
                let args = req["params"]["arguments"].clone();
                let text = handle_tool(tool, &args, &state).await;
                json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "content": [{ "type": "text", "text": text }] }
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

async fn handle_tool(tool: &str, args: &Value, state: &Arc<AppState>) -> String {
    match tool {
        "get_scene" => {
            let scene = state.scene.lock().await;
            serde_json::to_string_pretty(&*scene).unwrap_or_default()
        }

        "add_element" => {
            let mut scene = state.scene.lock().await;
            let id = scene.alloc_id();
            let kind = parse_kind(args["type"].as_str());
            let name = args["name"].as_str().unwrap_or("Element").to_string();
            let mut el = Element::new(id.clone(), name, kind);
            apply_patch(&mut el, args);
            if let Some(p) = args.get("position").and_then(|v| v.as_array()) {
                el.position = vec3(p, el.position);
            }
            scene.elements.push(el);
            format!("Added element '{id}'")
        }

        "update_element" => {
            let mut scene = state.scene.lock().await;
            let name = args["name"].as_str().or(args["id"].as_str()).unwrap_or("");
            match scene.element_mut(name) {
                Some(el) => { apply_patch(el, args); format!("Updated '{name}'") }
                None => format!("Element '{name}' not found"),
            }
        }

        "remove_element" => {
            let mut scene = state.scene.lock().await;
            let name = args["name"].as_str().or(args["id"].as_str()).unwrap_or("");
            if scene.remove(name) { format!("Removed '{name}'") } else { format!("'{name}' not found") }
        }

        "set_keyframe" => {
            let mut scene = state.scene.lock().await;
            let name = args["name"].as_str().or(args["id"].as_str()).unwrap_or("");
            let frame = args["frame"].as_f64().unwrap_or(1.0) as f32;
            let property = args["property"].as_str().unwrap_or("position").to_string();
            let ease = parse_easing(args["easing"].as_str());
            let kv = match parse_keyvalue(&property, &args["value"]) {
                Ok(v) => v,
                Err(e) => return format!("Bad keyframe value: {e}"),
            };
            match scene.element_mut(name) {
                Some(el) => { el.track_mut(&property).insert(frame, kv, ease); format!("Keyframed '{name}' {property} @ frame {frame}") }
                None => format!("Element '{name}' not found"),
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
            "Camera updated".into()
        }

        "set_keyframe_camera" => {
            let mut scene = state.scene.lock().await;
            let frame = args["frame"].as_f64().unwrap_or(1.0) as f32;
            let property = args["property"].as_str().unwrap_or("position").to_string();
            let ease = parse_easing(args["easing"].as_str());
            let kv = match parse_keyvalue(&property, &args["value"]) {
                Ok(v) => v,
                Err(e) => return format!("Bad value: {e}"),
            };
            let track = if let Some(nt) = scene.camera.tracks.iter_mut().find(|t| t.property == property) {
                &mut nt.track
            } else {
                scene.camera.tracks.push(crate::scene::NamedTrack { property: property.clone(), track: Default::default() });
                &mut scene.camera.tracks.last_mut().unwrap().track
            };
            track.insert(frame, kv, ease);
            format!("Camera {property} keyframed @ {frame}")
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
            "Render settings updated".into()
        }

        "render_frame" => {
            let frame = args["frame"].as_f64().unwrap_or(1.0) as f32;
            let out = args["output_path"].as_str().unwrap_or("/tmp/juicer_render.png").to_string();
            let scene = state.scene.lock().await.clone();
            let mut rg = state.renderer.lock().await;
            if rg.is_none() {
                match Renderer::new() {
                    Ok(r) => *rg = Some(r),
                    Err(e) => return format!("GPU init failed: {e}"),
                }
            }
            match rg.as_mut().unwrap().render_to_png(&scene, frame, &out) {
                Ok(()) => format!("Rendered frame {frame} → {out}"),
                Err(e) => format!("Render error: {e}"),
            }
        }

        "render_animation" => {
            let out = args["output_path"].as_str().unwrap_or("/tmp/juicer_demo").to_string();
            let scene = state.scene.lock().await.clone();
            let mut rg = state.renderer.lock().await;
            if rg.is_none() {
                match Renderer::new() {
                    Ok(r) => *rg = Some(r),
                    Err(e) => return format!("GPU init failed: {e}"),
                }
            }
            match video::render_animation(rg.as_mut().unwrap(), &scene, &out) {
                Ok(path) => format!("Rendered animation → {path}"),
                Err(e) => format!("Render error: {e}"),
            }
        }

        "arrange_demo_layout" => arrange_demo_layout(args, state).await,

        _ => format!("Unknown tool: {tool}"),
    }
}

async fn arrange_demo_layout(args: &Value, state: &Arc<AppState>) -> String {
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

    format!(
        "Created '{title}' demo layout: ContentPlane (id {id_plane}), two accent bars in {color}.\n\n\
         Next:\n\
         1. Use the HTML importer (or capture_html tool) to render your brand HTML to a PNG.\n\
         2. update_element ContentPlane with image_path=<that png>.\n\
         3. set_keyframe on ContentPlane: frame 1 opacity 0 + position [0,-1,0], frame 30 opacity 1 + position [0,0.4,0].\n\
         4. render_animation to export MP4."
    )
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
                    "value": { "description": "[x,y,z] for position/rotation/scale, or a number for opacity" },
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
            "description": "Render a single frame to PNG using the native GPU renderer.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "frame": { "type": "number" },
                    "output_path": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "render_animation",
            "description": "Render the full frame range to an MP4 video using the native GPU renderer.",
            "inputSchema": {
                "type": "object",
                "properties": { "output_path": { "type": "string", "description": "Output path, .mp4 appended if missing" } }
            }
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
