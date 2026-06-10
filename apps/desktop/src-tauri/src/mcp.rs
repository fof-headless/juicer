//! MCP server (stdio) for Claude Desktop.
//!
//! Wraps the shared `dispatch_tool` from `lib.rs` with the JSON-RPC framing
//! Claude Desktop expects. Every Juicer operation Claude can perform is defined
//! once in `dispatch_tool` and exposed via `tool_definitions()` below.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use crate::{dispatch_tool, is_mutation, AppState};

pub async fn run_stdio_server(state: Arc<AppState>) -> Result<()> {
    state.ensure_default_project().await;

    let stdin = io::stdin();
    let mut stdout = io::stdout();

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
            "initialize" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "juicer", "version": "0.2.0" }
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
                let content = match dispatch_tool(&tool, &args, &state).await {
                    Ok(v) => render_result_for_mcp(&tool, v),
                    Err(e) => vec![text(format!("Error: {e}"))],
                };
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

fn text(s: impl Into<String>) -> Value {
    json!({ "type": "text", "text": s.into() })
}

/// MCP wants a list of `content` entries (text/image). For most tools we just
/// stringify the JSON result. `render_frame` specifically gets both a text
/// summary AND an inline image entry so Claude can see what was rendered.
fn render_result_for_mcp(tool: &str, result: Value) -> Vec<Value> {
    if tool == "render_frame" {
        let path = result.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let frame = result.get("frame").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let b64 = result.get("image_base64").and_then(|v| v.as_str()).map(String::from);
        let mut out = vec![text(format!("Rendered frame {frame} → {path}"))];
        if let Some(b64) = b64 {
            out.push(json!({
                "type": "image",
                "source": { "type": "base64", "media_type": "image/png", "data": b64 }
            }));
        }
        return out;
    }
    if tool == "get_scene" || tool == "get_layer" || tool == "list_layers" || tool == "evaluate_at" {
        return vec![text(serde_json::to_string_pretty(&result).unwrap_or_default())];
    }
    vec![text(serde_json::to_string(&result).unwrap_or_default())]
}

// ── Tool schemas ──────────────────────────────────────────────────────────────

fn tool_definitions() -> Vec<Value> {
    // A few helper schemas used in multiple tools.
    let layer_id = json!({ "type": "string", "description": "Layer id (e.g. 'layer_3') or name." });
    let optional_number = json!({ "type": "number" });

    vec![
        // ── Layer CRUD ────────────────────────────────────────────────────────
        json!({
            "name": "add_html_layer",
            "description": "Add an HTML/CSS layer. The HTML is rendered in an isolated iframe (srcdoc) so your Tailwind/custom CSS cannot leak to other layers. Tailwind, Google Fonts, Lucide, Animate.css and Font Awesome are auto-injected — paste raw component markup.",
            "inputSchema": {
                "type": "object", "required": ["html"],
                "properties": {
                    "html": { "type": "string" },
                    "name": { "type": "string" },
                    "x": optional_number, "y": optional_number,
                    "width": optional_number, "height": optional_number,
                    "opacity": optional_number, "rotation": optional_number
                }
            }
        }),
        json!({
            "name": "add_image_layer",
            "description": "Add a static image layer (PNG/JPG/GIF/WebP). The file is copied into the project's assets/ folder.",
            "inputSchema": {
                "type": "object", "required": ["src_path"],
                "properties": {
                    "src_path": { "type": "string", "description": "Absolute path to the image file." },
                    "name": { "type": "string" },
                    "x": optional_number, "y": optional_number,
                    "width": optional_number, "height": optional_number,
                    "opacity": optional_number, "rotation": optional_number
                }
            }
        }),
        json!({
            "name": "add_shape_layer",
            "description": "Add a vector shape layer (rect or ellipse) with fill, optional stroke, and border-radius. Use this for backgrounds, accent bars, soft drop-shadow rectangles behind other layers, etc.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shape": { "type": "string", "enum": ["rect", "ellipse"] },
                    "fill": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["solid", "linear-gradient", "radial-gradient"] },
                            "color": { "type": "string", "description": "For type=solid. e.g. '#6644ff'." },
                            "angle": { "type": "number", "description": "Degrees, for linear-gradient." },
                            "stops": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "offset": { "type": "number", "minimum": 0, "maximum": 1 },
                                        "color": { "type": "string" }
                                    }
                                }
                            }
                        }
                    },
                    "stroke": {
                        "type": "object",
                        "properties": { "color": { "type": "string" }, "width": { "type": "number" } }
                    },
                    "border_radius": { "type": "number" },
                    "name": { "type": "string" },
                    "x": optional_number, "y": optional_number,
                    "width": optional_number, "height": optional_number,
                    "opacity": optional_number, "rotation": optional_number
                }
            }
        }),
        json!({
            "name": "add_text_layer",
            "description": "Add a single-line or multi-line text layer with full font control.",
            "inputSchema": {
                "type": "object", "required": ["text"],
                "properties": {
                    "text": { "type": "string" },
                    "font": { "type": "string", "description": "Family name, e.g. 'Inter'." },
                    "size": { "type": "number" },
                    "weight": { "type": "number", "description": "100..900" },
                    "italic": { "type": "boolean" },
                    "color": { "type": "string" },
                    "align": { "type": "string", "enum": ["left", "center", "right"] },
                    "name": { "type": "string" },
                    "x": optional_number, "y": optional_number,
                    "width": optional_number, "height": optional_number,
                    "opacity": optional_number, "rotation": optional_number
                }
            }
        }),
        json!({
            "name": "remove_layer",
            "description": "Delete a layer by id or name.",
            "inputSchema": { "type": "object", "required": ["id"], "properties": { "id": layer_id } }
        }),
        json!({
            "name": "duplicate_layer",
            "description": "Duplicate a layer (inserted just above the original with a 20px offset). Returns the new layer's id.",
            "inputSchema": { "type": "object", "required": ["id"], "properties": { "id": layer_id } }
        }),
        json!({
            "name": "reorder_layer",
            "description": "Move a layer to a new z-index. 0 = back; higher = front.",
            "inputSchema": {
                "type": "object", "required": ["id", "z_index"],
                "properties": { "id": layer_id, "z_index": { "type": "number" } }
            }
        }),
        json!({
            "name": "list_layers",
            "description": "List all layers in z-order with id, name, kind, and visibility.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_layer",
            "description": "Return the full JSON for a single layer (transform, effects, tracks, etc.).",
            "inputSchema": { "type": "object", "required": ["id"], "properties": { "id": layer_id } }
        }),
        json!({
            "name": "rename_layer",
            "description": "Rename a layer (the name is what shows up in the layer panel; the id is stable).",
            "inputSchema": {
                "type": "object", "required": ["id", "name"],
                "properties": { "id": layer_id, "name": { "type": "string" } }
            }
        }),

        // ── Per-property setters ──────────────────────────────────────────────
        json!({
            "name": "set_transform",
            "description": "Set transform components on a layer. Use rotate_x/rotate_y + perspective for 2.5D floating-card tilts (Apple-keynote look).",
            "inputSchema": {
                "type": "object", "required": ["id"],
                "properties": {
                    "id": layer_id,
                    "x": optional_number, "y": optional_number,
                    "rotation": { "type": "number", "description": "Z rotation in degrees." },
                    "scale_x": optional_number, "scale_y": optional_number,
                    "rotate_x": { "type": "number", "description": "X-axis tilt (degrees), 2.5D." },
                    "rotate_y": { "type": "number", "description": "Y-axis tilt (degrees), 2.5D." },
                    "perspective": { "type": "number", "description": "px; 0 = flat 2D, ~1200 for subtle tilt." },
                    "origin_x": { "type": "number", "description": "0-100%, defaults 50." },
                    "origin_y": { "type": "number", "description": "0-100%, defaults 50." }
                }
            }
        }),
        json!({
            "name": "set_opacity",
            "description": "Set opacity (0..1) on a layer.",
            "inputSchema": {
                "type": "object", "required": ["id", "opacity"],
                "properties": { "id": layer_id, "opacity": { "type": "number", "minimum": 0, "maximum": 1 } }
            }
        }),
        json!({
            "name": "set_size",
            "description": "Set width and/or height (px) on a layer.",
            "inputSchema": {
                "type": "object", "required": ["id"],
                "properties": { "id": layer_id, "width": optional_number, "height": optional_number }
            }
        }),
        json!({
            "name": "set_border_radius",
            "description": "Set border-radius (px). For non-shape layers this stores a frame-0 keyframe; for shape layers it updates the field.",
            "inputSchema": {
                "type": "object", "required": ["id", "radius"],
                "properties": { "id": layer_id, "radius": { "type": "number" } }
            }
        }),
        json!({
            "name": "set_shadow",
            "description": "Add or replace a CSS drop-shadow on a layer. For the Figma-look use offset_y≈8, blur≈32, spread≈-8, color='rgba(0,0,0,0.35)'. Pass `index` to set the Nth shadow (multi-shadow stacks); without index replaces shadow[0].",
            "inputSchema": {
                "type": "object", "required": ["id"],
                "properties": {
                    "id": layer_id,
                    "index": { "type": "number" },
                    "offset_x": optional_number, "offset_y": optional_number,
                    "blur": optional_number, "spread": optional_number,
                    "color": { "type": "string" },
                    "inset": { "type": "boolean" }
                }
            }
        }),
        json!({
            "name": "clear_shadows",
            "description": "Remove all shadows from a layer.",
            "inputSchema": { "type": "object", "required": ["id"], "properties": { "id": layer_id } }
        }),
        json!({
            "name": "set_blur",
            "description": "Set CSS filter:blur radius in px. 0 = no blur.",
            "inputSchema": {
                "type": "object", "required": ["id", "radius"],
                "properties": { "id": layer_id, "radius": { "type": "number" } }
            }
        }),
        json!({
            "name": "set_fill",
            "description": "Set the fill of a shape layer. Supports solid color or linear/radial gradient with multiple stops.",
            "inputSchema": {
                "type": "object", "required": ["id", "fill"],
                "properties": {
                    "id": layer_id,
                    "fill": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["solid", "linear-gradient", "radial-gradient"] },
                            "color": { "type": "string" },
                            "angle": { "type": "number" },
                            "stops": { "type": "array" }
                        }
                    }
                }
            }
        }),
        json!({
            "name": "set_stroke",
            "description": "Set or clear a shape's stroke. Pass {color, width} or omit to clear.",
            "inputSchema": {
                "type": "object", "required": ["id"],
                "properties": {
                    "id": layer_id,
                    "stroke": {
                        "type": "object",
                        "properties": { "color": { "type": "string" }, "width": { "type": "number" } }
                    }
                }
            }
        }),
        json!({
            "name": "set_text",
            "description": "Replace the text content of a text layer.",
            "inputSchema": {
                "type": "object", "required": ["id", "text"],
                "properties": { "id": layer_id, "text": { "type": "string" } }
            }
        }),
        json!({
            "name": "set_font",
            "description": "Set font properties on a text layer.",
            "inputSchema": {
                "type": "object", "required": ["id"],
                "properties": {
                    "id": layer_id,
                    "family": { "type": "string" },
                    "size": { "type": "number" },
                    "weight": { "type": "number" },
                    "italic": { "type": "boolean" },
                    "letter_spacing": { "type": "number" },
                    "line_height": { "type": "number" },
                    "color": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "set_html",
            "description": "Replace the HTML content of an html layer (re-renders the iframe srcdoc).",
            "inputSchema": {
                "type": "object", "required": ["id", "html"],
                "properties": { "id": layer_id, "html": { "type": "string" } }
            }
        }),
        json!({
            "name": "set_image_src",
            "description": "Replace the source file of an image layer. The new file is imported into the project's assets/.",
            "inputSchema": {
                "type": "object", "required": ["id", "src_path"],
                "properties": { "id": layer_id, "src_path": { "type": "string" } }
            }
        }),
        json!({
            "name": "set_blend_mode",
            "description": "Set CSS mix-blend-mode for compositing.",
            "inputSchema": {
                "type": "object", "required": ["id", "mode"],
                "properties": {
                    "id": layer_id,
                    "mode": { "type": "string", "enum": [
                        "normal", "multiply", "screen", "overlay", "darken", "lighten",
                        "color-dodge", "color-burn", "hard-light", "soft-light",
                        "difference", "exclusion"
                    ]}
                }
            }
        }),
        json!({
            "name": "set_visible",
            "description": "Show or hide a layer (does not delete it).",
            "inputSchema": {
                "type": "object", "required": ["id", "visible"],
                "properties": { "id": layer_id, "visible": { "type": "boolean" } }
            }
        }),

        // ── Keyframing ────────────────────────────────────────────────────────
        json!({
            "name": "set_keyframe",
            "description":
                "Insert a keyframe on any animatable property. \
                 Properties: x, y, rotation, scale_x, scale_y, rotate_x, rotate_y, perspective, \
                 opacity, width, height, border_radius, shadow_offset_x, shadow_offset_y, \
                 shadow_blur, shadow_spread, shadow_color, filter_blur, fill_color, \
                 text_content (step-only), font_size. \
                 Easing: either a named preset (`easing`: linear|step|ease-in|ease-out|ease-in-out|ease-back) \
                 OR cubic-bezier control points (`bezier`: [p1x, p1y, p2x, p2y]). Bezier matches \
                 Blender's F-curve handles / CSS cubic-bezier() exactly.",
            "inputSchema": {
                "type": "object", "required": ["id", "frame", "property", "value"],
                "properties": {
                    "id": layer_id,
                    "frame": { "type": "number" },
                    "property": { "type": "string" },
                    "value": {
                        "description": "Number for scalar properties, hex string for color, string for text_content."
                    },
                    "easing": { "type": "string", "enum": [
                        "linear", "step", "ease-in", "ease-out", "ease-in-out", "ease-back"
                    ]},
                    "bezier": {
                        "type": "array", "items": { "type": "number" }, "minItems": 4, "maxItems": 4,
                        "description": "[p1x, p1y, p2x, p2y]; e.g. [0.68, -0.55, 0.27, 1.55] for back-out overshoot."
                    }
                }
            }
        }),
        json!({
            "name": "remove_keyframe",
            "description": "Remove a keyframe at a specific frame from a track.",
            "inputSchema": {
                "type": "object", "required": ["id", "frame", "property"],
                "properties": { "id": layer_id, "frame": { "type": "number" }, "property": { "type": "string" } }
            }
        }),
        json!({
            "name": "clear_track",
            "description": "Remove the entire keyframe track for a property on a layer.",
            "inputSchema": {
                "type": "object", "required": ["id", "property"],
                "properties": { "id": layer_id, "property": { "type": "string" } }
            }
        }),
        json!({
            "name": "copy_track",
            "description": "Copy a property's keyframe track from one layer to another (e.g. stagger animations).",
            "inputSchema": {
                "type": "object", "required": ["from_id", "to_id", "property"],
                "properties": {
                    "from_id": { "type": "string" },
                    "to_id": { "type": "string" },
                    "property": { "type": "string" }
                }
            }
        }),

        // ── Canvas / timeline ─────────────────────────────────────────────────
        json!({
            "name": "set_canvas",
            "description": "Set canvas resolution, fps, and background. Background is any CSS color or gradient string.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "width": { "type": "number" }, "height": { "type": "number" },
                    "fps": { "type": "number" }, "background": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "set_duration",
            "description": "Set total animation duration in frames.",
            "inputSchema": {
                "type": "object", "required": ["frames"],
                "properties": { "frames": { "type": "number" } }
            }
        }),

        // ── Query / render ────────────────────────────────────────────────────
        json!({
            "name": "get_scene",
            "description": "Return the full Juicer scene JSON (canvas, duration, all layers with effects/tracks).",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "evaluate_at",
            "description": "Evaluate the scene at a specific frame and return the resolved per-layer CSS style snapshot — useful for debugging keyframes without rendering.",
            "inputSchema": {
                "type": "object", "required": ["frame"],
                "properties": { "frame": { "type": "number" } }
            }
        }),
        json!({
            "name": "render_frame",
            "description": "Render a single frame to PNG using the WebKit-backed renderer. Returns the file path AND an inline image so Claude can see what was produced.",
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
            "description": "Render every frame in 0..duration_frames and encode to MP4 via the native AVFoundation encoder.",
            "inputSchema": {
                "type": "object",
                "properties": { "output_path": { "type": "string" } }
            }
        }),

        // ── Projects ──────────────────────────────────────────────────────────
        json!({
            "name": "create_project",
            "description": "Create a new project folder (~/Movies/Juicer/<name>) and make it active. Scenes auto-save there.",
            "inputSchema": {
                "type": "object", "required": ["name"],
                "properties": { "name": { "type": "string" } }
            }
        }),
        json!({
            "name": "open_project",
            "description": "Open an existing project folder by absolute path and load its scene.json.",
            "inputSchema": {
                "type": "object", "required": ["path"],
                "properties": { "path": { "type": "string" } }
            }
        }),
        json!({
            "name": "save_project",
            "description": "Force-save the current scene to the active project's scene.json (scenes also autosave after every mutation).",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_project",
            "description": "Show the active project's name and folder paths.",
            "inputSchema": { "type": "object", "properties": {} }
        }),

        // ── Legacy convenience ────────────────────────────────────────────────
        json!({
            "name": "capture_html",
            "description": "One-shot: render HTML to a transparent PNG in the project's assets/. Optionally add_layer=true to also create an image layer with that PNG. Prefer add_html_layer for live-CSS layers; use this only when you want a baked PNG (e.g. for sprite-style movement without re-rendering CSS).",
            "inputSchema": {
                "type": "object", "required": ["html"],
                "properties": {
                    "html": { "type": "string" },
                    "name": { "type": "string" },
                    "width": { "type": "number" },
                    "height": { "type": "number" },
                    "font": { "type": "string" },
                    "libraries": { "type": "array", "items": { "type": "string" } },
                    "add_layer": { "type": "boolean" }
                }
            }
        }),
    ]
}
