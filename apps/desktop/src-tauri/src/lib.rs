//! Juicer Tauri host + shared op dispatcher.
//!
//! All scene mutations and queries flow through `dispatch_tool` in this file.
//! The MCP server (`mcp.rs`) calls it; the Tauri `call` command also calls it.
//! That keeps the UI and Claude-over-MCP completely in sync.

mod anim;
mod html_capture;
mod mcp;
mod project;
mod renderer;
mod scene;
mod video;

use base64::Engine as _;
use std::sync::Arc;
use serde_json::{json, Value};
use tauri::State;
use tokio::sync::Mutex;

use crate::anim::{parse_css_color, KeyValue};
use crate::project::Project;
use crate::renderer::Renderer;
use crate::scene::{
    easing_from_name, BlendMode, BoxShadow, Fill, FontSpec, GradientStop, Layer, LayerKind,
    Scene, ShapeKind, Stroke, TextAlign,
};

// ── App state ─────────────────────────────────────────────────────────────────

pub struct AppState {
    pub scene: Mutex<Scene>,
    pub renderer: Mutex<Option<Renderer>>,
    pub project: Mutex<Option<Project>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            scene: Mutex::new(Scene::default()),
            renderer: Mutex::new(None),
            project: Mutex::new(None),
        }
    }

    /// Lock order is always project → scene. Don't hold the scene lock when
    /// calling this.
    pub async fn autosave(&self) {
        let proj = self.project.lock().await;
        if let Some(p) = proj.as_ref() {
            let scene = self.scene.lock().await;
            if let Err(e) = p.save_scene(&scene) {
                eprintln!("[juicer] autosave failed: {e}");
            }
        }
    }

    pub async fn ensure_default_project(&self) {
        let mut proj = self.project.lock().await;
        if proj.is_some() {
            return;
        }
        match Project::create("Default") {
            Ok(p) => {
                if let Ok(Some(loaded)) = p.load_scene() {
                    *self.scene.lock().await = loaded;
                }
                *proj = Some(p);
            }
            Err(e) => eprintln!("[juicer] could not create default project: {e}"),
        }
    }
}

pub type SharedState = Arc<AppState>;

// ── Tool dispatcher: the single source of truth for every Juicer operation ────

/// Every scene mutation/query/render goes through this. MCP and the Tauri UI
/// both call it. If you add a new tool, register it here AND in `tool_definitions()`
/// in `mcp.rs`. If it mutates state, also add it to `is_mutation()` so autosave fires.
pub async fn dispatch_tool(tool: &str, args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    match tool {
        // ── Layer CRUD ────────────────────────────────────────────────────────
        "add_html_layer" => add_html_layer(args, state).await,
        "add_image_layer" => add_image_layer(args, state).await,
        "add_shape_layer" => add_shape_layer(args, state).await,
        "add_text_layer" => add_text_layer(args, state).await,
        "remove_layer" => remove_layer(args, state).await,
        "duplicate_layer" => duplicate_layer(args, state).await,
        "reorder_layer" => reorder_layer(args, state).await,
        "list_layers" => list_layers(state).await,

        // ── Per-property setters ──────────────────────────────────────────────
        "set_transform" => set_transform(args, state).await,
        "set_opacity" => set_opacity(args, state).await,
        "set_size" => set_size(args, state).await,
        "set_border_radius" => set_border_radius(args, state).await,
        "set_shadow" => set_shadow(args, state).await,
        "clear_shadows" => clear_shadows(args, state).await,
        "set_blur" => set_blur(args, state).await,
        "set_fill" => set_fill(args, state).await,
        "set_stroke" => set_stroke(args, state).await,
        "set_text" => set_text_content(args, state).await,
        "set_font" => set_font(args, state).await,
        "set_html" => set_html(args, state).await,
        "set_image_src" => set_image_src(args, state).await,
        "set_blend_mode" => set_blend_mode(args, state).await,
        "set_visible" => set_visible(args, state).await,
        "rename_layer" => rename_layer(args, state).await,

        // ── Keyframing ────────────────────────────────────────────────────────
        "set_keyframe" => set_keyframe(args, state).await,
        "remove_keyframe" => remove_keyframe(args, state).await,
        "clear_track" => clear_track(args, state).await,
        "copy_track" => copy_track(args, state).await,

        // ── Canvas / timeline ─────────────────────────────────────────────────
        "set_canvas" => set_canvas(args, state).await,
        "set_duration" => set_duration(args, state).await,

        // ── Query / render ────────────────────────────────────────────────────
        "get_scene" => {
            let scene = state.scene.lock().await;
            Ok(serde_json::to_value(&*scene).map_err(|e| e.to_string())?)
        }
        "get_layer" => get_layer(args, state).await,
        "evaluate_at" => evaluate_at(args, state).await,
        "render_frame" => render_frame(args, state).await,
        "render_animation" => render_animation(args, state).await,

        // ── Project ───────────────────────────────────────────────────────────
        "create_project" => create_project(args, state).await,
        "open_project" => open_project(args, state).await,
        "save_project" => save_project(state).await,
        "get_project" => get_project(state).await,

        // ── HTML one-shot capture (kept from the old API) ─────────────────────
        "capture_html" => capture_html(args, state).await,

        _ => Err(format!("unknown tool: {tool}")),
    }
}

/// Tools whose execution mutates scene state. After running such a tool we
/// trigger an autosave.
pub fn is_mutation(tool: &str) -> bool {
    matches!(
        tool,
        "add_html_layer" | "add_image_layer" | "add_shape_layer" | "add_text_layer"
        | "remove_layer" | "duplicate_layer" | "reorder_layer"
        | "set_transform" | "set_opacity" | "set_size" | "set_border_radius"
        | "set_shadow" | "clear_shadows" | "set_blur" | "set_fill" | "set_stroke"
        | "set_text" | "set_font" | "set_html" | "set_image_src"
        | "set_blend_mode" | "set_visible" | "rename_layer"
        | "set_keyframe" | "remove_keyframe" | "clear_track" | "copy_track"
        | "set_canvas" | "set_duration"
        | "capture_html"
    )
}

// ── Layer CRUD impls ──────────────────────────────────────────────────────────

async fn add_html_layer(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let html = args["html"].as_str().ok_or("html is required")?.to_string();
    let name = args["name"].as_str().unwrap_or("HTML").to_string();
    let mut scene = state.scene.lock().await;
    let id = scene.alloc_id();
    let mut layer = Layer::new(id.clone(), name, LayerKind::Html { html });
    apply_common_layer_props(&mut layer, args);
    scene.layers.push(layer);
    Ok(json!({ "id": id }))
}

async fn add_image_layer(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let src = args["src_path"].as_str().ok_or("src_path is required")?;
    // Copy into project's assets/ if outside it.
    let src = {
        let proj = state.project.lock().await;
        match proj.as_ref() {
            Some(p) => p.import_asset(src).unwrap_or_else(|_| src.to_string()),
            None => src.to_string(),
        }
    };
    let name = args["name"].as_str().unwrap_or("Image").to_string();
    let mut scene = state.scene.lock().await;
    let id = scene.alloc_id();
    let mut layer = Layer::new(id.clone(), name, LayerKind::Image { src_path: src });
    apply_common_layer_props(&mut layer, args);
    scene.layers.push(layer);
    Ok(json!({ "id": id }))
}

async fn add_shape_layer(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let shape = match args["shape"].as_str().unwrap_or("rect") {
        "ellipse" | "circle" => ShapeKind::Ellipse,
        _ => ShapeKind::Rect,
    };
    let fill = parse_fill(args.get("fill")).unwrap_or_default();
    let stroke = parse_stroke(args.get("stroke"));
    let border_radius = args["border_radius"].as_f64().unwrap_or(12.0) as f32;
    let name = args["name"].as_str().unwrap_or("Shape").to_string();
    let mut scene = state.scene.lock().await;
    let id = scene.alloc_id();
    let mut layer = Layer::new(
        id.clone(),
        name,
        LayerKind::Shape { shape, fill, stroke, border_radius },
    );
    apply_common_layer_props(&mut layer, args);
    scene.layers.push(layer);
    Ok(json!({ "id": id }))
}

async fn add_text_layer(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let text = args["text"].as_str().unwrap_or("Text").to_string();
    let mut font = FontSpec::default();
    if let Some(s) = args["font"].as_str() { font.family = s.to_string(); }
    if let Some(s) = args["size"].as_f64() { font.size = s as f32; }
    if let Some(s) = args["weight"].as_u64() { font.weight = s as u32; }
    if let Some(s) = args["italic"].as_bool() { font.italic = s; }
    let color = args["color"].as_str().unwrap_or("#ffffff").to_string();
    let align = match args["align"].as_str().unwrap_or("left") {
        "center" => TextAlign::Center,
        "right" => TextAlign::Right,
        _ => TextAlign::Left,
    };
    let name = args["name"].as_str().unwrap_or("Text").to_string();
    let mut scene = state.scene.lock().await;
    let id = scene.alloc_id();
    let mut layer = Layer::new(
        id.clone(),
        name,
        LayerKind::Text { content: text, font, color, align },
    );
    apply_common_layer_props(&mut layer, args);
    scene.layers.push(layer);
    Ok(json!({ "id": id }))
}

fn apply_common_layer_props(layer: &mut Layer, args: &Value) {
    if let Some(v) = args.get("x").and_then(|v| v.as_f64()) { layer.transform.x = v as f32; }
    if let Some(v) = args.get("y").and_then(|v| v.as_f64()) { layer.transform.y = v as f32; }
    if let Some(v) = args.get("width").and_then(|v| v.as_f64()) { layer.width = v as f32; }
    if let Some(v) = args.get("height").and_then(|v| v.as_f64()) { layer.height = v as f32; }
    if let Some(v) = args.get("rotation").and_then(|v| v.as_f64()) { layer.transform.rotation = v as f32; }
    if let Some(v) = args.get("opacity").and_then(|v| v.as_f64()) { layer.opacity = v as f32; }
    if let Some(v) = args.get("z").and_then(|v| v.as_u64()) {
        // The actual reorder happens after push; this is a hint for "reorder_layer" callers.
        let _ = v;
    }
}

async fn remove_layer(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let mut scene = state.scene.lock().await;
    if scene.remove_layer(id) {
        Ok(json!({ "removed": id }))
    } else {
        Err(format!("layer '{id}' not found"))
    }
}

async fn duplicate_layer(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let mut scene = state.scene.lock().await;
    let idx = scene.layer_index(id).ok_or_else(|| format!("layer '{id}' not found"))?;
    let mut clone = scene.layers[idx].clone();
    let new_id = scene.alloc_id();
    clone.id = new_id.clone();
    clone.name = format!("{} copy", clone.name);
    // Slight offset so the duplicate is visible.
    clone.transform.x += 20.0;
    clone.transform.y += 20.0;
    scene.layers.insert(idx + 1, clone);
    Ok(json!({ "id": new_id }))
}

async fn reorder_layer(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let new_index = args["z_index"]
        .as_u64()
        .or_else(|| args["index"].as_u64())
        .ok_or("z_index is required")? as usize;
    let mut scene = state.scene.lock().await;
    match scene.reorder(id, new_index) {
        Some(n) => Ok(json!({ "id": id, "z_index": n })),
        None => Err(format!("layer '{id}' not found")),
    }
}

async fn list_layers(state: &Arc<AppState>) -> Result<Value, String> {
    let scene = state.scene.lock().await;
    let arr: Vec<Value> = scene
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            json!({
                "id": l.id,
                "name": l.name,
                "kind": match &l.kind {
                    LayerKind::Html { .. } => "html",
                    LayerKind::Image { .. } => "image",
                    LayerKind::Shape { .. } => "shape",
                    LayerKind::Text { .. } => "text",
                },
                "z": i,
                "visible": l.visible,
            })
        })
        .collect();
    Ok(json!({ "layers": arr }))
}

async fn get_layer(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let scene = state.scene.lock().await;
    let layer = scene.layer(id).ok_or_else(|| format!("layer '{id}' not found"))?;
    Ok(serde_json::to_value(layer).map_err(|e| e.to_string())?)
}

// ── Per-property setters ──────────────────────────────────────────────────────

async fn with_layer_mut<F>(state: &Arc<AppState>, id: &str, f: F) -> Result<(), String>
where
    F: FnOnce(&mut Layer) -> Result<(), String>,
{
    let mut scene = state.scene.lock().await;
    let layer = scene.layer_mut(id).ok_or_else(|| format!("layer '{id}' not found"))?;
    f(layer)
}

async fn set_transform(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    with_layer_mut(state, id, |l| {
        if let Some(v) = args.get("x").and_then(|v| v.as_f64()) { l.transform.x = v as f32; }
        if let Some(v) = args.get("y").and_then(|v| v.as_f64()) { l.transform.y = v as f32; }
        if let Some(v) = args.get("rotation").and_then(|v| v.as_f64()) { l.transform.rotation = v as f32; }
        if let Some(v) = args.get("scale_x").and_then(|v| v.as_f64()) { l.transform.scale_x = v as f32; }
        if let Some(v) = args.get("scale_y").and_then(|v| v.as_f64()) { l.transform.scale_y = v as f32; }
        if let Some(v) = args.get("rotate_x").and_then(|v| v.as_f64()) { l.transform.rotate_x = v as f32; }
        if let Some(v) = args.get("rotate_y").and_then(|v| v.as_f64()) { l.transform.rotate_y = v as f32; }
        if let Some(v) = args.get("perspective").and_then(|v| v.as_f64()) { l.transform.perspective = v as f32; }
        if let Some(v) = args.get("origin_x").and_then(|v| v.as_f64()) { l.transform.origin_x = v as f32; }
        if let Some(v) = args.get("origin_y").and_then(|v| v.as_f64()) { l.transform.origin_y = v as f32; }
        Ok(())
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_opacity(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let v = args["opacity"].as_f64().ok_or("opacity is required")? as f32;
    with_layer_mut(state, id, |l| { l.opacity = v.clamp(0.0, 1.0); Ok(()) }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_size(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    with_layer_mut(state, id, |l| {
        if let Some(v) = args.get("width").and_then(|v| v.as_f64()) { l.width = v as f32; }
        if let Some(v) = args.get("height").and_then(|v| v.as_f64()) { l.height = v as f32; }
        Ok(())
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_border_radius(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let r = args["radius"].as_f64().ok_or("radius is required")? as f32;
    with_layer_mut(state, id, |l| {
        if let LayerKind::Shape { border_radius, .. } = &mut l.kind {
            *border_radius = r;
        } else {
            // Add a generic radius track-free override via effects? For HTML/image/text
            // we keyframe via tracks too; for direct set, we just stash on the shape variant.
            // For non-shape layers, set via a keyframe at frame 0:
            l.track_mut("border_radius").insert(0.0, KeyValue::Scalar(r), Default::default());
        }
        Ok(())
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_shadow(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let shadow = BoxShadow {
        offset_x: args["offset_x"].as_f64().unwrap_or(0.0) as f32,
        offset_y: args["offset_y"].as_f64().unwrap_or(8.0) as f32,
        blur: args["blur"].as_f64().unwrap_or(32.0) as f32,
        spread: args["spread"].as_f64().unwrap_or(0.0) as f32,
        color: args["color"].as_str().unwrap_or("rgba(0,0,0,0.35)").to_string(),
        inset: args["inset"].as_bool().unwrap_or(false),
    };
    let index = args["index"].as_u64().map(|n| n as usize);
    with_layer_mut(state, id, |l| {
        match index {
            Some(i) => {
                while l.effects.shadows.len() <= i {
                    l.effects.shadows.push(BoxShadow::default());
                }
                l.effects.shadows[i] = shadow;
            }
            None => {
                if l.effects.shadows.is_empty() {
                    l.effects.shadows.push(shadow);
                } else {
                    l.effects.shadows[0] = shadow;
                }
            }
        }
        Ok(())
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn clear_shadows(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    with_layer_mut(state, id, |l| { l.effects.shadows.clear(); Ok(()) }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_blur(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let r = args["radius"].as_f64().ok_or("radius is required")? as f32;
    with_layer_mut(state, id, |l| { l.effects.filter_blur = r; Ok(()) }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_fill(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let fill = parse_fill(args.get("fill")).ok_or("invalid fill spec")?;
    with_layer_mut(state, id, |l| {
        match &mut l.kind {
            LayerKind::Shape { fill: f, .. } => *f = fill,
            _ => return Err("set_fill only applies to shape layers (use set_text/set_html for others)".into()),
        }
        Ok(())
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_stroke(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let stroke = parse_stroke(args.get("stroke"));
    with_layer_mut(state, id, |l| {
        if let LayerKind::Shape { stroke: s, .. } = &mut l.kind {
            *s = stroke;
            Ok(())
        } else {
            Err("set_stroke only applies to shape layers".into())
        }
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_text_content(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let text = args["text"].as_str().ok_or("text is required")?.to_string();
    with_layer_mut(state, id, |l| {
        if let LayerKind::Text { content, .. } = &mut l.kind {
            *content = text;
            Ok(())
        } else {
            Err("set_text only applies to text layers".into())
        }
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_font(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    with_layer_mut(state, id, |l| {
        if let LayerKind::Text { font, color, .. } = &mut l.kind {
            if let Some(s) = args.get("family").and_then(|v| v.as_str()) { font.family = s.to_string(); }
            if let Some(s) = args.get("size").and_then(|v| v.as_f64()) { font.size = s as f32; }
            if let Some(s) = args.get("weight").and_then(|v| v.as_u64()) { font.weight = s as u32; }
            if let Some(s) = args.get("italic").and_then(|v| v.as_bool()) { font.italic = s; }
            if let Some(s) = args.get("letter_spacing").and_then(|v| v.as_f64()) { font.letter_spacing = s as f32; }
            if let Some(s) = args.get("line_height").and_then(|v| v.as_f64()) { font.line_height = s as f32; }
            if let Some(s) = args.get("color").and_then(|v| v.as_str()) { *color = s.to_string(); }
            Ok(())
        } else {
            Err("set_font only applies to text layers".into())
        }
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_html(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let new_html = args["html"].as_str().ok_or("html is required")?.to_string();
    with_layer_mut(state, id, |l| {
        if let LayerKind::Html { html } = &mut l.kind {
            *html = new_html;
            Ok(())
        } else {
            Err("set_html only applies to html layers".into())
        }
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_image_src(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let src = args["src_path"].as_str().ok_or("src_path is required")?;
    // Import into assets/ if external.
    let imported = {
        let proj = state.project.lock().await;
        match proj.as_ref() {
            Some(p) => p.import_asset(src).unwrap_or_else(|_| src.to_string()),
            None => src.to_string(),
        }
    };
    with_layer_mut(state, id, |l| {
        if let LayerKind::Image { src_path } = &mut l.kind {
            *src_path = imported;
            Ok(())
        } else {
            Err("set_image_src only applies to image layers".into())
        }
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_blend_mode(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let mode_str = args["mode"].as_str().unwrap_or("normal");
    let mode = match mode_str {
        "normal" => BlendMode::Normal,
        "multiply" => BlendMode::Multiply,
        "screen" => BlendMode::Screen,
        "overlay" => BlendMode::Overlay,
        "darken" => BlendMode::Darken,
        "lighten" => BlendMode::Lighten,
        "color-dodge" => BlendMode::ColorDodge,
        "color-burn" => BlendMode::ColorBurn,
        "hard-light" => BlendMode::HardLight,
        "soft-light" => BlendMode::SoftLight,
        "difference" => BlendMode::Difference,
        "exclusion" => BlendMode::Exclusion,
        _ => return Err(format!("unknown blend mode: {mode_str}")),
    };
    with_layer_mut(state, id, |l| { l.blend_mode = mode; Ok(()) }).await?;
    Ok(json!({ "ok": true }))
}

async fn set_visible(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let v = args["visible"].as_bool().ok_or("visible is required")?;
    with_layer_mut(state, id, |l| { l.visible = v; Ok(()) }).await?;
    Ok(json!({ "ok": true }))
}

async fn rename_layer(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let name = args["name"].as_str().ok_or("name is required")?.to_string();
    with_layer_mut(state, id, |l| { l.name = name; Ok(()) }).await?;
    Ok(json!({ "ok": true }))
}

// ── Keyframing ────────────────────────────────────────────────────────────────

async fn set_keyframe(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let frame = args["frame"].as_f64().ok_or("frame is required")? as f32;
    let property = args["property"].as_str().ok_or("property is required")?.to_string();
    let kv = parse_keyvalue_for_property(&property, &args["value"])?;
    let easing = if let Some(s) = args["easing"].as_str() {
        easing_from_name(s)
    } else if let Some(arr) = args.get("bezier").and_then(|v| v.as_array()) {
        // bezier: [p1x, p1y, p2x, p2y]
        if arr.len() == 4 {
            crate::anim::Easing::Bezier {
                p1: [arr[0].as_f64().unwrap_or(0.0) as f32, arr[1].as_f64().unwrap_or(0.0) as f32],
                p2: [arr[2].as_f64().unwrap_or(1.0) as f32, arr[3].as_f64().unwrap_or(1.0) as f32],
            }
        } else {
            Default::default()
        }
    } else {
        Default::default()
    };
    with_layer_mut(state, id, |l| {
        l.track_mut(&property).insert(frame, kv, easing);
        Ok(())
    }).await?;
    Ok(json!({ "ok": true, "frame": frame, "property": property }))
}

async fn remove_keyframe(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let frame = args["frame"].as_f64().ok_or("frame is required")? as f32;
    let property = args["property"].as_str().ok_or("property is required")?.to_string();
    with_layer_mut(state, id, |l| {
        if let Some(idx) = l.tracks.iter().position(|t| t.property == property) {
            l.tracks[idx].track.remove_at(frame);
        }
        Ok(())
    }).await?;
    Ok(json!({ "ok": true }))
}

async fn clear_track(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("id is required")?;
    let property = args["property"].as_str().ok_or("property is required")?.to_string();
    with_layer_mut(state, id, |l| { l.remove_track(&property); Ok(()) }).await?;
    Ok(json!({ "ok": true }))
}

async fn copy_track(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let from_id = args["from_id"].as_str().ok_or("from_id is required")?;
    let to_id = args["to_id"].as_str().ok_or("to_id is required")?;
    let property = args["property"].as_str().ok_or("property is required")?;
    let mut scene = state.scene.lock().await;
    let source = scene
        .layer(from_id)
        .and_then(|l| l.track(property))
        .cloned()
        .ok_or_else(|| format!("source track {from_id}.{property} not found"))?;
    let dest = scene.layer_mut(to_id).ok_or_else(|| format!("layer '{to_id}' not found"))?;
    *dest.track_mut(property) = source;
    Ok(json!({ "ok": true }))
}

// ── Canvas / timeline ─────────────────────────────────────────────────────────

async fn set_canvas(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let mut scene = state.scene.lock().await;
    if let Some(v) = args.get("width").and_then(|v| v.as_u64()) { scene.canvas.width = v as u32; }
    if let Some(v) = args.get("height").and_then(|v| v.as_u64()) { scene.canvas.height = v as u32; }
    if let Some(v) = args.get("fps").and_then(|v| v.as_u64()) { scene.canvas.fps = v as u32; }
    if let Some(v) = args.get("background").and_then(|v| v.as_str()) { scene.canvas.background = v.to_string(); }
    Ok(json!({ "ok": true }))
}

async fn set_duration(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let frames = args["frames"].as_u64().ok_or("frames is required")? as u32;
    let mut scene = state.scene.lock().await;
    scene.duration_frames = frames;
    Ok(json!({ "ok": true }))
}

// ── Query / render ────────────────────────────────────────────────────────────

async fn evaluate_at(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let frame = args["frame"].as_f64().unwrap_or(0.0) as f32;
    let scene = state.scene.lock().await;
    Ok(serde_json::to_value(scene.evaluate_at(frame)).map_err(|e| e.to_string())?)
}

async fn render_frame(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let frame = args["frame"].as_f64().unwrap_or(0.0) as f32;
    let out_path = match args["output_path"].as_str() {
        Some(p) => p.to_string(),
        None => {
            let proj = state.project.lock().await;
            match proj.as_ref() {
                Some(p) => p.render_path(&format!("frame_{:05}", frame as u32), "png"),
                None => std::env::temp_dir().join(format!("juicer_frame_{:05}.png", frame as u32))
                    .to_string_lossy().to_string(),
            }
        }
    };
    let scene = state.scene.lock().await.clone();
    let mut rg = state.renderer.lock().await;
    if rg.is_none() {
        *rg = Some(Renderer::new().map_err(|e| e.to_string())?);
    }
    let r = rg.as_mut().unwrap();
    let bytes = r.render_frame(&scene, frame).map_err(|e| e.to_string())?;
    std::fs::write(&out_path, &bytes).map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(json!({ "path": out_path, "image_base64": b64, "frame": frame }))
}

async fn render_animation(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let out_path = match args["output_path"].as_str() {
        Some(p) => p.to_string(),
        None => {
            let proj = state.project.lock().await;
            match proj.as_ref() {
                Some(p) => p.render_path("output", "mp4"),
                None => std::env::temp_dir().join("juicer_output.mp4").to_string_lossy().to_string(),
            }
        }
    };
    let scene = state.scene.lock().await.clone();
    let mut rg = state.renderer.lock().await;
    if rg.is_none() {
        *rg = Some(Renderer::new().map_err(|e| e.to_string())?);
    }
    let r = rg.as_mut().unwrap();
    let path = video::render_animation(r, &scene, &out_path).map_err(|e| e.to_string())?;
    Ok(json!({ "path": path }))
}

// ── Projects ──────────────────────────────────────────────────────────────────

fn project_info(p: &Project) -> Value {
    json!({
        "name": p.name,
        "root": p.root.to_string_lossy(),
        "assets": p.assets_dir().to_string_lossy(),
        "renders": p.renders_dir().to_string_lossy(),
    })
}

async fn create_project(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let name = args["name"].as_str().unwrap_or("Untitled");
    let p = Project::create(name).map_err(|e| e.to_string())?;
    *state.scene.lock().await = Scene::default();
    let _ = p.save_scene(&*state.scene.lock().await);
    let info = project_info(&p);
    *state.project.lock().await = Some(p);
    Ok(info)
}

async fn open_project(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let path = args["path"].as_str().ok_or("path is required")?;
    let (p, scene) = Project::open(path).map_err(|e| e.to_string())?;
    if let Some(loaded) = scene {
        *state.scene.lock().await = loaded;
    }
    let info = project_info(&p);
    *state.project.lock().await = Some(p);
    Ok(info)
}

async fn save_project(state: &Arc<AppState>) -> Result<Value, String> {
    let proj = state.project.lock().await;
    match proj.as_ref() {
        Some(p) => {
            p.save_scene(&*state.scene.lock().await).map_err(|e| e.to_string())?;
            Ok(json!({ "path": p.scene_path().to_string_lossy() }))
        }
        None => Err("no active project".into()),
    }
}

async fn get_project(state: &Arc<AppState>) -> Result<Value, String> {
    let proj = state.project.lock().await;
    Ok(proj.as_ref().map(project_info).unwrap_or(Value::Null))
}

// ── HTML one-shot capture (legacy convenience, kept) ──────────────────────────

async fn capture_html(args: &Value, state: &Arc<AppState>) -> Result<Value, String> {
    let html = args["html"].as_str().ok_or("html is required")?;
    let name = args["name"].as_str().unwrap_or("capture").to_string();
    let width = args["width"].as_u64().unwrap_or(1200) as u32;
    let height = args["height"].as_u64().unwrap_or(800) as u32;
    let font = args["font"].as_str();
    let libraries: Vec<String> = args["libraries"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let add_layer = args["add_layer"].as_bool().unwrap_or(false);

    let out = {
        let proj = state.project.lock().await;
        match proj.as_ref() {
            Some(p) => p.asset_path(&name, "png"),
            None => std::env::temp_dir().join(format!("{name}.png")).to_string_lossy().to_string(),
        }
    };
    let wrapped = html_capture::wrap_html(html, &libraries, font);
    html_capture::capture_html_to_png(&wrapped, width, height, &out)
        .await
        .map_err(|e| e.to_string())?;

    let mut result = json!({ "path": out.clone() });
    if add_layer {
        let mut scene = state.scene.lock().await;
        let id = scene.alloc_id();
        let mut layer = Layer::new(id.clone(), name.clone(), LayerKind::Image { src_path: out.clone() });
        layer.width = width as f32;
        layer.height = height as f32;
        scene.layers.push(layer);
        result["layer_id"] = Value::String(id);
    }
    Ok(result)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_fill(v: Option<&Value>) -> Option<Fill> {
    let v = v?;
    let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("solid");
    match t {
        "solid" => Some(Fill::Solid {
            color: v.get("color").and_then(|x| x.as_str()).unwrap_or("#6644ff").to_string(),
        }),
        "linear-gradient" => {
            let stops = parse_stops(v.get("stops"))?;
            let angle = v.get("angle").and_then(|x| x.as_f64()).unwrap_or(180.0) as f32;
            Some(Fill::LinearGradient { stops, angle })
        }
        "radial-gradient" => {
            let stops = parse_stops(v.get("stops"))?;
            Some(Fill::RadialGradient { stops })
        }
        _ => None,
    }
}

fn parse_stops(v: Option<&Value>) -> Option<Vec<GradientStop>> {
    let arr = v?.as_array()?;
    Some(arr.iter().filter_map(|s| {
        Some(GradientStop {
            offset: s.get("offset").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
            color: s.get("color").and_then(|x| x.as_str()).unwrap_or("#ffffff").to_string(),
        })
    }).collect())
}

fn parse_stroke(v: Option<&Value>) -> Option<Stroke> {
    let v = v?;
    if v.is_null() { return None; }
    Some(Stroke {
        color: v.get("color").and_then(|x| x.as_str()).unwrap_or("#ffffff").to_string(),
        width: v.get("width").and_then(|x| x.as_f64()).unwrap_or(1.0) as f32,
    })
}

/// Parse a JSON `value` into a `KeyValue` appropriate for the given property name.
pub fn parse_keyvalue_for_property(property: &str, value: &Value) -> Result<KeyValue, String> {
    match property {
        // Color tracks
        "shadow_color" | "fill_color" | "text_color" => {
            let s = value.as_str().ok_or("color value must be a string")?;
            parse_css_color(s)
                .map(KeyValue::Color)
                .ok_or_else(|| format!("invalid color: {s}"))
        }
        // Text content track (step-only)
        "text_content" => {
            let s = value.as_str().ok_or("text_content value must be a string")?;
            Ok(KeyValue::Str(s.to_string()))
        }
        // Everything else is scalar.
        _ => {
            let n = value.as_f64()
                .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                .ok_or_else(|| format!("value for '{property}' must be a number"))?;
            Ok(KeyValue::Scalar(n as f32))
        }
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
async fn call(
    state: State<'_, SharedState>,
    tool: String,
    args: Option<Value>,
) -> Result<Value, String> {
    let args = args.unwrap_or(Value::Null);
    let result = dispatch_tool(&tool, &args, state.inner()).await?;
    if is_mutation(&tool) {
        state.autosave().await;
    }
    Ok(result)
}

/// Convenience: scene snapshot (same as `call("get_scene", null)` but a direct
/// command so the UI can call it without extra plumbing).
#[tauri::command]
async fn get_scene_cmd(state: State<'_, SharedState>) -> Result<Value, String> {
    let scene = state.scene.lock().await;
    Ok(serde_json::to_value(&*scene).map_err(|e| e.to_string())?)
}

/// Returns the canonical scene_doc + render-state at frame T — what the UI's
/// preview iframe consumes via postMessage.
#[tauri::command]
async fn preview_state(state: State<'_, SharedState>, frame: f32) -> Result<Value, String> {
    let scene = state.scene.lock().await;
    Ok(json!({
        "scene_doc": crate::renderer::build_scene_doc_value(&*scene),
        "state": scene.evaluate_at(frame),
        "canvas_width": scene.canvas.width,
        "canvas_height": scene.canvas.height,
    }))
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run() {
    if std::env::args().any(|a| a == "--mcp") {
        let state = Arc::new(AppState::new());
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            if let Err(e) = mcp::run_stdio_server(state).await {
                eprintln!("[MCP] {e}");
            }
        });
        return;
    }

    let state: SharedState = Arc::new(AppState::new());
    tauri::async_runtime::block_on(state.ensure_default_project());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            call,
            get_scene_cmd,
            preview_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Juicer");
}
