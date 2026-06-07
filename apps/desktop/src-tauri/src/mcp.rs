/// MCP server running inside the Tauri process.
///
/// Claude Desktop connects via stdio. The server translates tool calls
/// into JSON commands forwarded to the Blender bridge.
///
/// Because Tauri owns the process, the MCP server runs as a Tokio task
/// reading stdin / writing stdout — no separate process needed.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use tauri::AppHandle;
use crate::BridgeState;

pub async fn run_mcp_server(bridge: BridgeState, _app: AppHandle) -> Result<()> {
    // MCP over stdio — read JSON-RPC lines from stdin, write to stdout
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // Send MCP initialize response capability advertisement
    let caps = json!({
        "jsonrpc": "2.0",
        "id": 0,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "juicer", "version": "0.1.0" }
        }
    });

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }

        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req["method"].as_str().unwrap_or("");

        let response = match method {
            "initialize" => caps.clone(),
            "tools/list" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": tool_definitions() }
                })
            }
            "tools/call" => {
                let tool = req["params"]["name"].as_str().unwrap_or("");
                let args = &req["params"]["arguments"];
                let result = handle_tool_call(tool, args, &bridge).await;
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": result }]
                    }
                })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
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

async fn handle_tool_call(tool: &str, args: &Value, bridge: &BridgeState) -> String {
    let cmd = match tool {
        "get_scene" => json!({ "op": "get_scene" }),

        "add_element" => json!({
            "op": "add_object",
            "name": args["name"].as_str().unwrap_or("Object"),
            "kind": args["type"].as_str().unwrap_or("box"),
            "location": args.get("position").cloned().unwrap_or(json!([0,0,0])),
            "rotation": args.get("rotation").cloned().unwrap_or(json!([0,0,0])),
            "scale": args.get("scale").cloned().unwrap_or(json!([1,1,1])),
            "color": args.get("color").cloned().unwrap_or(json!("#4488ff")),
            "image_path": args.get("image_path"),
            "text": args.get("textContent"),
            "width": args.get("width").cloned().unwrap_or(json!(2.0)),
            "height": args.get("height").cloned().unwrap_or(json!(2.0)),
        }),

        "update_element" => json!({
            "op": "update_object",
            "name": args["name"],
            "location": args.get("position"),
            "rotation": args.get("rotation"),
            "scale": args.get("scale"),
            "color": args.get("color"),
            "visible": args.get("visible"),
        }),

        "remove_element" => json!({
            "op": "remove_object",
            "name": args["name"],
        }),

        "set_keyframe" => json!({
            "op": "set_keyframe",
            "name": args["name"],
            "frame": args["frame"],
            "property": args["property"],
            "value": args["value"],
        }),

        "play_animation" => json!({ "op": "play" }),
        "pause_animation" => json!({ "op": "pause" }),
        "seek_animation" => json!({ "op": "seek", "frame": args["frame"] }),

        "render_frame" => json!({
            "op": "render_frame",
            "frame": args.get("frame").cloned().unwrap_or(json!(1)),
            "output_path": args.get("output_path").cloned().unwrap_or(json!("/tmp/juicer_render.png")),
        }),

        "render_animation" => json!({
            "op": "render_animation",
            "output_path": args.get("output_path").cloned().unwrap_or(json!("/tmp/juicer_")),
            "start": args.get("start").cloned().unwrap_or(json!(1)),
            "end": args.get("end").cloned().unwrap_or(json!(250)),
            "fps": args.get("fps").cloned().unwrap_or(json!(30)),
            "format": args.get("format").cloned().unwrap_or(json!("FFMPEG")),
        }),

        "set_scene_fps" => json!({
            "op": "set_fps",
            "fps": args["fps"],
        }),

        "set_render_resolution" => json!({
            "op": "set_resolution",
            "width": args["width"],
            "height": args["height"],
        }),

        "set_background" => json!({
            "op": "set_background",
            "color": args.get("color").cloned().unwrap_or(json!([0.05, 0.05, 0.1, 1.0])),
        }),

        "arrange_demo_layout" => {
            // High-level composite command — build multiple ops
            return build_demo_layout(args, bridge).await;
        }

        _ => return format!("Unknown tool: {tool}"),
    };

    let mut b = bridge.lock().await;
    match b.send_command(&cmd.to_string()).await {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default(),
        Err(e) => format!("Error: {e}"),
    }
}

async fn build_demo_layout(args: &Value, bridge: &BridgeState) -> String {
    let color = args["accentColor"].as_str().unwrap_or("#6644ff");
    let title = args["title"].as_str().unwrap_or("Your Product");
    let _bg_style = args["backgroundStyle"].as_str().unwrap_or("dark");

    let ops: Vec<Value> = vec![
        // Dark background plane
        json!({ "op": "set_background", "color": [0.05, 0.05, 0.08, 1.0] }),
        // Title text
        json!({ "op": "add_object", "kind": "text", "name": "Title",
                 "text": title, "location": [0, 1.5, 0], "rotation": [0,0,0],
                 "scale": [1,1,1], "color": color }),
        // Content plane (empty, user fills with HTML capture)
        json!({ "op": "add_object", "kind": "plane", "name": "ContentPlane",
                 "location": [0, 0, 0], "rotation": [1.5708, 0, 0],
                 "scale": [3.5, 2.2, 1], "color": "#1a1a2e" }),
        // Decorative accent box left
        json!({ "op": "add_object", "kind": "box", "name": "AccentL",
                 "location": [-2.5, 0, -0.1], "rotation": [0,0,0],
                 "scale": [0.05, 2.0, 0.05], "color": color }),
        // Decorative accent box right
        json!({ "op": "add_object", "kind": "box", "name": "AccentR",
                 "location": [2.5, 0, -0.1], "rotation": [0,0,0],
                 "scale": [0.05, 2.0, 0.05], "color": color }),
    ];

    let mut results = Vec::new();
    for op in ops {
        let mut b = bridge.lock().await;
        match b.send_command(&op.to_string()).await {
            Ok(v) => results.push(v),
            Err(e) => results.push(json!({ "error": e.to_string() })),
        }
    }

    format!(
        "Created demo layout: title '{title}', content plane, accent bars (color: {color}).\n\nNext steps:\n- Use capture_html to render your brand HTML to a PNG, then use add_element with image_path to map it onto ContentPlane\n- Set keyframes with set_keyframe to animate position/opacity\n- Render with render_animation\n\nResults: {}",
        serde_json::to_string_pretty(&results).unwrap_or_default()
    )
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "get_scene",
            "description": "Get all objects in the current Blender scene — names, types, positions, keyframes.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "add_element",
            "description": "Add a 3D object to the scene. Types: box, sphere, plane, cylinder, text, light. Use 'plane' with image_path to display an image (e.g. captured HTML).",
            "inputSchema": {
                "type": "object",
                "required": ["type", "name"],
                "properties": {
                    "type": { "type": "string", "enum": ["box", "sphere", "plane", "cylinder", "text", "light"] },
                    "name": { "type": "string" },
                    "position": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "[x, y, z] in Blender world units" },
                    "rotation": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "[rx, ry, rz] in radians" },
                    "scale": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "color": { "type": "string", "description": "Hex color e.g. #ff4488" },
                    "image_path": { "type": "string", "description": "Absolute path to PNG/JPG to use as texture on a plane" },
                    "textContent": { "type": "string", "description": "Text string for text objects" },
                    "width": { "type": "number" },
                    "height": { "type": "number" }
                }
            }
        }),
        json!({
            "name": "update_element",
            "description": "Move, scale, rotate, recolor, or hide an existing scene object by Blender object name.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "position": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "rotation": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "scale": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 },
                    "color": { "type": "string" },
                    "visible": { "type": "boolean" }
                }
            }
        }),
        json!({
            "name": "remove_element",
            "description": "Delete a scene object by name.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": { "name": { "type": "string" } }
            }
        }),
        json!({
            "name": "set_keyframe",
            "description": "Set a keyframe on an object property at a specific frame. This uses Blender's native keyframe system.",
            "inputSchema": {
                "type": "object",
                "required": ["name", "frame", "property"],
                "properties": {
                    "name": { "type": "string", "description": "Object name" },
                    "frame": { "type": "number", "description": "Frame number (e.g. 1, 30, 60)" },
                    "property": { "type": "string", "enum": ["location", "rotation_euler", "scale", "color", "alpha"], "description": "Which property to keyframe" },
                    "value": { "description": "Value to set. Array of 3 for location/rotation/scale, number for alpha." }
                }
            }
        }),
        json!({
            "name": "play_animation",
            "description": "Start playing the animation timeline in the Blender viewport.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "pause_animation",
            "description": "Pause animation playback.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "seek_animation",
            "description": "Jump to a specific frame.",
            "inputSchema": {
                "type": "object",
                "required": ["frame"],
                "properties": { "frame": { "type": "number" } }
            }
        }),
        json!({
            "name": "render_frame",
            "description": "Render a single frame using Blender's Eevee GPU renderer. Returns path to PNG.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "frame": { "type": "number", "description": "Frame to render (default: current frame)" },
                    "output_path": { "type": "string", "description": "Output PNG path (default: /tmp/juicer_render.png)" }
                }
            }
        }),
        json!({
            "name": "render_animation",
            "description": "Render the full animation to video using Blender's Eevee GPU renderer. Outputs MP4/WebM/ProRes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "output_path": { "type": "string", "description": "Output path prefix e.g. /tmp/juicer_demo" },
                    "start": { "type": "number", "description": "Start frame (default: 1)" },
                    "end": { "type": "number", "description": "End frame (default: 250 = ~8s at 30fps)" },
                    "fps": { "type": "number", "description": "Frames per second (default: 30)" },
                    "format": { "type": "string", "enum": ["FFMPEG", "PNG"], "description": "FFMPEG = MP4 video, PNG = image sequence" }
                }
            }
        }),
        json!({
            "name": "set_render_resolution",
            "description": "Set the render output resolution.",
            "inputSchema": {
                "type": "object",
                "required": ["width", "height"],
                "properties": {
                    "width": { "type": "number", "description": "e.g. 1920" },
                    "height": { "type": "number", "description": "e.g. 1080" }
                }
            }
        }),
        json!({
            "name": "set_background",
            "description": "Set the scene background color (RGBA 0-1).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "array", "items": { "type": "number" }, "minItems": 4, "maxItems": 4, "description": "[r, g, b, a] each 0-1" }
                }
            }
        }),
        json!({
            "name": "arrange_demo_layout",
            "description": "One-shot command: creates a cinematic product demo layout with title, content plane, and accent elements. Best starting point for a new demo.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "subtitle": { "type": "string" },
                    "accentColor": { "type": "string", "description": "Brand accent color hex" },
                    "backgroundStyle": { "type": "string", "enum": ["dark", "light", "gradient"] }
                }
            }
        }),
    ]
}
