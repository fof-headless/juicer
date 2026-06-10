//! Juicer scene model — 2.5D layer compositor.
//!
//! A scene is an ordered list of layers (z-order = vec index). Each layer is one
//! of: HTML (Tailwind/CSS in an isolated iframe), Image (PNG/JPG), Shape (rect/
//! ellipse with fill+stroke+radius), or Text (font'd string).
//!
//! Every layer carries a `Transform2_5D` (x, y, rotation, scale, optional
//! rotateX/Y + perspective for floating-card 2.5D), opacity, blend mode, and
//! effects (drop shadows, blur). Any of these can be keyframed via named tracks
//! evaluated by `anim.rs`.
//!
//! `evaluate_at(frame)` produces a `RenderState` — a per-layer snapshot of CSS
//! styles that the renderer pipes into the offscreen WKWebView via JS.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::anim::{rgba_to_css, Easing, Track};

// ── Layer kind ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeKind {
    Rect,
    Ellipse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
}

impl Default for BlendMode {
    fn default() -> Self { BlendMode::Normal }
}

impl BlendMode {
    pub fn as_css(self) -> &'static str {
        match self {
            BlendMode::Normal => "normal",
            BlendMode::Multiply => "multiply",
            BlendMode::Screen => "screen",
            BlendMode::Overlay => "overlay",
            BlendMode::Darken => "darken",
            BlendMode::Lighten => "lighten",
            BlendMode::ColorDodge => "color-dodge",
            BlendMode::ColorBurn => "color-burn",
            BlendMode::HardLight => "hard-light",
            BlendMode::SoftLight => "soft-light",
            BlendMode::Difference => "difference",
            BlendMode::Exclusion => "exclusion",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign { Left, Center, Right }
impl Default for TextAlign { fn default() -> Self { TextAlign::Left } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientStop {
    /// 0..1 along the gradient.
    pub offset: f32,
    /// CSS color string ("#rrggbb" or "rgba(...)").
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Fill {
    Solid { color: String },
    LinearGradient { stops: Vec<GradientStop>, angle: f32 },
    RadialGradient { stops: Vec<GradientStop> },
}

impl Default for Fill {
    fn default() -> Self { Fill::Solid { color: "#6644ff".into() } }
}

impl Fill {
    /// Render to a CSS `background` value.
    pub fn as_css(&self) -> String {
        match self {
            Fill::Solid { color } => color.clone(),
            Fill::LinearGradient { stops, angle } => {
                let s = stops.iter()
                    .map(|s| format!("{} {}%", s.color, (s.offset * 100.0).clamp(0.0, 100.0)))
                    .collect::<Vec<_>>().join(", ");
                format!("linear-gradient({}deg, {})", angle, s)
            }
            Fill::RadialGradient { stops } => {
                let s = stops.iter()
                    .map(|s| format!("{} {}%", s.color, (s.offset * 100.0).clamp(0.0, 100.0)))
                    .collect::<Vec<_>>().join(", ");
                format!("radial-gradient(circle, {})", s)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stroke {
    pub color: String,
    pub width: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontSpec {
    pub family: String,
    pub size: f32,
    pub weight: u32,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub letter_spacing: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
}
fn default_line_height() -> f32 { 1.2 }

impl Default for FontSpec {
    fn default() -> Self {
        Self {
            family: "Inter".into(),
            size: 48.0,
            weight: 600,
            italic: false,
            letter_spacing: 0.0,
            line_height: 1.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LayerKind {
    /// User HTML/CSS rendered inside an isolated iframe srcdoc.
    Html { html: String },
    /// PNG/JPG from disk (or assets/).
    Image { src_path: String },
    /// Vector primitive.
    Shape {
        shape: ShapeKind,
        #[serde(default)]
        fill: Fill,
        #[serde(default)]
        stroke: Option<Stroke>,
        #[serde(default)]
        border_radius: f32,
    },
    /// Single styled string.
    Text {
        content: String,
        #[serde(default)]
        font: FontSpec,
        #[serde(default = "default_text_color")]
        color: String,
        #[serde(default)]
        align: TextAlign,
    },
}
fn default_text_color() -> String { "#ffffff".into() }

// ── Transform / effects ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform2_5D {
    pub x: f32,
    pub y: f32,
    /// Z-rotation in degrees.
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    /// 3D tilt (degrees) for floating-card / Apple-keynote look.
    pub rotate_x: f32,
    pub rotate_y: f32,
    /// CSS perspective in px. 0 → no perspective applied.
    pub perspective: f32,
    /// Transform origin as percentage of layer size (0..100).
    pub origin_x: f32,
    pub origin_y: f32,
}

impl Default for Transform2_5D {
    fn default() -> Self {
        Self {
            x: 0.0, y: 0.0, rotation: 0.0,
            scale_x: 1.0, scale_y: 1.0,
            rotate_x: 0.0, rotate_y: 0.0,
            perspective: 0.0,
            origin_x: 50.0, origin_y: 50.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: String,
    #[serde(default)]
    pub inset: bool,
}

impl Default for BoxShadow {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 8.0,
            blur: 32.0,
            spread: -8.0,
            color: "rgba(0,0,0,0.35)".into(),
            inset: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Effects {
    #[serde(default)]
    pub shadows: Vec<BoxShadow>,
    /// CSS filter: blur(...) in px.
    #[serde(default)]
    pub filter_blur: f32,
}

// ── Layer ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedTrack {
    pub property: String,
    pub track: Track,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub kind: LayerKind,

    /// On-canvas size in px (before transform). The layer's left/top in canvas
    /// space is given by `transform.x` / `transform.y`.
    pub width: f32,
    pub height: f32,

    #[serde(default)]
    pub transform: Transform2_5D,
    pub opacity: f32,
    pub visible: bool,
    #[serde(default)]
    pub blend_mode: BlendMode,
    #[serde(default)]
    pub effects: Effects,

    /// Per-property keyframe tracks. Property names listed in module docs.
    #[serde(default)]
    pub tracks: Vec<NamedTrack>,
}

impl Layer {
    pub fn new(id: String, name: String, kind: LayerKind) -> Self {
        let (w, h) = default_size_for(&kind);
        Self {
            id, name, kind,
            width: w, height: h,
            transform: Transform2_5D::default(),
            opacity: 1.0,
            visible: true,
            blend_mode: BlendMode::Normal,
            effects: Effects::default(),
            tracks: Vec::new(),
        }
    }

    pub fn track_mut(&mut self, property: &str) -> &mut Track {
        if let Some(idx) = self.tracks.iter().position(|t| t.property == property) {
            return &mut self.tracks[idx].track;
        }
        self.tracks.push(NamedTrack { property: property.to_string(), track: Track::default() });
        &mut self.tracks.last_mut().unwrap().track
    }

    pub fn track(&self, property: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.property == property).map(|t| &t.track)
    }

    pub fn remove_track(&mut self, property: &str) -> bool {
        let before = self.tracks.len();
        self.tracks.retain(|t| t.property != property);
        self.tracks.len() != before
    }
}

fn default_size_for(kind: &LayerKind) -> (f32, f32) {
    match kind {
        LayerKind::Html { .. } => (480.0, 320.0),
        LayerKind::Image { .. } => (480.0, 320.0),
        LayerKind::Shape { .. } => (240.0, 240.0),
        LayerKind::Text { .. } => (600.0, 80.0),
    }
}

// ── Canvas / scene ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// CSS color or gradient string for the canvas background.
    pub background: String,
}

impl Default for Canvas {
    fn default() -> Self {
        Self { width: 1920, height: 1080, fps: 30, background: "#0d0d12".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub layers: Vec<Layer>,
    pub canvas: Canvas,
    pub duration_frames: u32,
    next_id: u64,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            canvas: Canvas::default(),
            duration_frames: 150,
            next_id: 1,
        }
    }
}

impl Scene {
    pub fn alloc_id(&mut self) -> String {
        let id = format!("layer_{}", self.next_id);
        self.next_id += 1;
        id
    }

    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id || l.name == id)
    }

    pub fn layer_mut(&mut self, id: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id || l.name == id)
    }

    pub fn layer_index(&self, id: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id || l.name == id)
    }

    pub fn remove_layer(&mut self, id: &str) -> bool {
        let before = self.layers.len();
        self.layers.retain(|l| l.id != id && l.name != id);
        self.layers.len() != before
    }

    /// Move a layer to a new z-index (clamped to [0, len-1]). Returns the new
    /// index, or None if the layer doesn't exist.
    pub fn reorder(&mut self, id: &str, new_index: usize) -> Option<usize> {
        let idx = self.layer_index(id)?;
        let layer = self.layers.remove(idx);
        let new_idx = new_index.min(self.layers.len());
        self.layers.insert(new_idx, layer);
        Some(new_idx)
    }

    pub fn duration_secs(&self) -> f32 {
        self.duration_frames as f32 / self.canvas.fps.max(1) as f32
    }

    /// Evaluate the entire scene at `frame`, producing a per-layer CSS-style
    /// snapshot ready to ship to the renderer. See `RenderState` below for
    /// shape. Pure function — does not mutate the scene.
    pub fn evaluate_at(&self, frame: f32) -> RenderState {
        RenderState {
            frame,
            canvas_width: self.canvas.width,
            canvas_height: self.canvas.height,
            canvas_background: self.canvas.background.clone(),
            layers: self.layers.iter().map(|l| evaluate_layer(l, frame)).collect(),
        }
    }
}

// ── RenderState — what we ship to the JS runtime ──────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RenderState {
    pub frame: f32,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub canvas_background: String,
    pub layers: Vec<ResolvedLayer>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedLayer {
    pub id: String,
    pub visible: bool,
    /// CSS properties keyed by their CSS name (e.g. "transform", "box-shadow",
    /// "background", "border-radius"). The JS runtime just writes these into
    /// `element.style.setProperty(...)` directly.
    pub style: BTreeMap<String, String>,
    /// Override the element's text content this frame (text layers with a
    /// keyframed `text_content` track). None means don't touch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

// ── Evaluation ────────────────────────────────────────────────────────────────

fn evaluate_layer(l: &Layer, frame: f32) -> ResolvedLayer {
    // Clone base values, override with sampled track values.
    let mut t = l.transform.clone();
    let mut opacity = l.opacity;
    let mut width = l.width;
    let mut height = l.height;
    let mut filter_blur = l.effects.filter_blur;

    // Shadow override targets the FIRST shadow if present, otherwise creates one
    // when any shadow_* track is animated.
    let mut shadow_override = l.effects.shadows.first().cloned();
    let mut shadow_touched = false;

    let mut border_radius_override: Option<f32> = None;
    let mut fill_color_override: Option<String> = None;
    let mut text_override: Option<String> = None;
    let mut font_size_override: Option<f32> = None;

    for nt in &l.tracks {
        let Some(v) = nt.track.sample(frame) else { continue };
        match nt.property.as_str() {
            "x" => if let Some(s) = v.as_scalar() { t.x = s; }
            "y" => if let Some(s) = v.as_scalar() { t.y = s; }
            "rotation" => if let Some(s) = v.as_scalar() { t.rotation = s; }
            "scale_x" => if let Some(s) = v.as_scalar() { t.scale_x = s; }
            "scale_y" => if let Some(s) = v.as_scalar() { t.scale_y = s; }
            "rotate_x" => if let Some(s) = v.as_scalar() { t.rotate_x = s; }
            "rotate_y" => if let Some(s) = v.as_scalar() { t.rotate_y = s; }
            "perspective" => if let Some(s) = v.as_scalar() { t.perspective = s; }
            "opacity" => if let Some(s) = v.as_scalar() { opacity = s; }
            "width" => if let Some(s) = v.as_scalar() { width = s; }
            "height" => if let Some(s) = v.as_scalar() { height = s; }
            "border_radius" => if let Some(s) = v.as_scalar() { border_radius_override = Some(s); }
            "filter_blur" => if let Some(s) = v.as_scalar() { filter_blur = s; }
            "shadow_offset_x" => if let Some(s) = v.as_scalar() {
                shadow_override.get_or_insert_with(BoxShadow::default).offset_x = s; shadow_touched = true;
            }
            "shadow_offset_y" => if let Some(s) = v.as_scalar() {
                shadow_override.get_or_insert_with(BoxShadow::default).offset_y = s; shadow_touched = true;
            }
            "shadow_blur" => if let Some(s) = v.as_scalar() {
                shadow_override.get_or_insert_with(BoxShadow::default).blur = s; shadow_touched = true;
            }
            "shadow_spread" => if let Some(s) = v.as_scalar() {
                shadow_override.get_or_insert_with(BoxShadow::default).spread = s; shadow_touched = true;
            }
            "shadow_color" => if let Some(c) = v.as_color() {
                shadow_override.get_or_insert_with(BoxShadow::default).color = rgba_to_css(c); shadow_touched = true;
            }
            "fill_color" => if let Some(c) = v.as_color() {
                fill_color_override = Some(rgba_to_css(c));
            }
            "text_content" => if let Some(s) = v.as_str() {
                text_override = Some(s.to_string());
            }
            "font_size" => if let Some(s) = v.as_scalar() {
                font_size_override = Some(s);
            }
            _ => {}
        }
    }

    // Build CSS.
    let mut style: BTreeMap<String, String> = BTreeMap::new();

    // Position: absolute placement in canvas via left/top + transform.
    // We use translate3d (in addition to transform.x/y) to keep one canonical
    // transform string; the layer is left:0 / top:0 in DOM, all motion in
    // transform.
    style.insert("left".into(), "0px".into());
    style.insert("top".into(), "0px".into());
    style.insert("width".into(), format!("{}px", width));
    style.insert("height".into(), format!("{}px", height));
    style.insert("transform".into(), build_transform(&t));
    style.insert("transform-origin".into(), format!("{}% {}%", t.origin_x, t.origin_y));
    style.insert("opacity".into(), format!("{}", opacity.clamp(0.0, 1.0)));
    if l.blend_mode != BlendMode::Normal {
        style.insert("mix-blend-mode".into(), l.blend_mode.as_css().to_string());
    }
    if filter_blur > 0.0 {
        style.insert("filter".into(), format!("blur({}px)", filter_blur));
    }

    // Box-shadow: merge override into the shadows vec (replace index 0 if
    // touched). Concatenate all shadows into a single CSS string.
    let shadows = if shadow_touched {
        let mut shadows = l.effects.shadows.clone();
        if shadows.is_empty() {
            shadows.push(shadow_override.unwrap());
        } else {
            shadows[0] = shadow_override.unwrap();
        }
        shadows
    } else {
        l.effects.shadows.clone()
    };
    if !shadows.is_empty() {
        let css = shadows.iter().map(box_shadow_to_css).collect::<Vec<_>>().join(", ");
        style.insert("box-shadow".into(), css);
    }

    // Kind-specific styling.
    match &l.kind {
        LayerKind::Shape { shape, fill, stroke, border_radius } => {
            let radius = border_radius_override.unwrap_or(*border_radius);
            let radius_css = match shape {
                ShapeKind::Ellipse => "50%".to_string(),
                ShapeKind::Rect => format!("{}px", radius),
            };
            style.insert("border-radius".into(), radius_css);

            let bg = if let Some(c) = fill_color_override {
                c
            } else {
                fill.as_css()
            };
            style.insert("background".into(), bg);

            if let Some(s) = stroke {
                style.insert("border".into(), format!("{}px solid {}", s.width, s.color));
                style.insert("box-sizing".into(), "border-box".into());
            }
        }
        LayerKind::Text { content: _, font, color, align } => {
            if let Some(r) = border_radius_override {
                style.insert("border-radius".into(), format!("{}px", r));
            }
            let size = font_size_override.unwrap_or(font.size);
            let color = fill_color_override.unwrap_or_else(|| color.clone());
            style.insert("color".into(), color);
            style.insert("font-family".into(),
                format!("'{}', -apple-system, BlinkMacSystemFont, sans-serif", font.family));
            style.insert("font-size".into(), format!("{}px", size));
            style.insert("font-weight".into(), font.weight.to_string());
            style.insert("font-style".into(), if font.italic { "italic" } else { "normal" }.into());
            style.insert("letter-spacing".into(), format!("{}px", font.letter_spacing));
            style.insert("line-height".into(), font.line_height.to_string());
            style.insert("text-align".into(), match align {
                TextAlign::Left => "left", TextAlign::Center => "center", TextAlign::Right => "right",
            }.into());
            style.insert("display".into(), "flex".into());
            style.insert("align-items".into(), "center".into());
        }
        LayerKind::Image { .. } | LayerKind::Html { .. } => {
            if let Some(r) = border_radius_override {
                style.insert("border-radius".into(), format!("{}px", r));
                style.insert("overflow".into(), "hidden".into());
            }
        }
    }

    ResolvedLayer {
        id: l.id.clone(),
        visible: l.visible,
        style,
        text: text_override,
    }
}

fn build_transform(t: &Transform2_5D) -> String {
    let mut parts: Vec<String> = Vec::new();
    if t.perspective > 0.0 {
        parts.push(format!("perspective({}px)", t.perspective));
    }
    parts.push(format!("translate({}px, {}px)", t.x, t.y));
    if t.rotation != 0.0 { parts.push(format!("rotate({}deg)", t.rotation)); }
    if t.rotate_x != 0.0 { parts.push(format!("rotateX({}deg)", t.rotate_x)); }
    if t.rotate_y != 0.0 { parts.push(format!("rotateY({}deg)", t.rotate_y)); }
    if t.scale_x != 1.0 || t.scale_y != 1.0 {
        parts.push(format!("scale({}, {})", t.scale_x, t.scale_y));
    }
    parts.join(" ")
}

fn box_shadow_to_css(s: &BoxShadow) -> String {
    let prefix = if s.inset { "inset " } else { "" };
    format!("{}{}px {}px {}px {}px {}",
        prefix, s.offset_x, s.offset_y, s.blur, s.spread, s.color)
}

// ── Convenience for callers ───────────────────────────────────────────────────

/// Helper for setters that take string easing presets.
pub fn easing_from_name(name: &str) -> Easing {
    match name {
        "linear" => Easing::Linear,
        "step" => Easing::Step,
        "ease-in" | "ease_in" => Easing::ease_in(),
        "ease-out" | "ease_out" => Easing::ease_out(),
        "ease-in-out" | "ease_in_out" => Easing::ease_in_out(),
        "ease-back" | "back" => Easing::ease_back(),
        _ => Easing::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::{Easing, KeyValue};

    #[test]
    fn evaluate_animates_x() {
        let mut s = Scene::default();
        let id = s.alloc_id();
        let mut l = Layer::new(id.clone(), "card".into(), LayerKind::Shape {
            shape: ShapeKind::Rect,
            fill: Fill::default(),
            stroke: None,
            border_radius: 12.0,
        });
        l.track_mut("x").insert(0.0, KeyValue::Scalar(0.0), Easing::Linear);
        l.track_mut("x").insert(60.0, KeyValue::Scalar(600.0), Easing::Linear);
        s.layers.push(l);

        let mid = s.evaluate_at(30.0);
        let tr = mid.layers[0].style.get("transform").unwrap();
        assert!(tr.contains("translate(300"), "expected x≈300 at frame 30, got {}", tr);
    }

    #[test]
    fn evaluate_shape_background() {
        let mut s = Scene::default();
        let id = s.alloc_id();
        let l = Layer::new(id, "rect".into(), LayerKind::Shape {
            shape: ShapeKind::Rect,
            fill: Fill::Solid { color: "#ff8800".into() },
            stroke: None,
            border_radius: 16.0,
        });
        s.layers.push(l);
        let r = s.evaluate_at(0.0);
        assert_eq!(r.layers[0].style.get("background").map(String::as_str), Some("#ff8800"));
        assert_eq!(r.layers[0].style.get("border-radius").map(String::as_str), Some("16px"));
    }

    #[test]
    fn evaluate_shadow_animated() {
        let mut s = Scene::default();
        let id = s.alloc_id();
        let mut l = Layer::new(id, "card".into(), LayerKind::Shape {
            shape: ShapeKind::Rect, fill: Fill::default(), stroke: None, border_radius: 0.0,
        });
        l.track_mut("shadow_blur").insert(0.0, KeyValue::Scalar(0.0), Easing::Linear);
        l.track_mut("shadow_blur").insert(30.0, KeyValue::Scalar(40.0), Easing::Linear);
        s.layers.push(l);
        let r = s.evaluate_at(15.0);
        let bs = r.layers[0].style.get("box-shadow").unwrap();
        assert!(bs.contains("20px"), "expected blur≈20px at midpoint, got {}", bs);
    }
}
