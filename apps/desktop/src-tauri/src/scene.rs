//! Juicer scene model — the native equivalent of Blender's object/data layer.
//!
//! A scene is a flat list of elements, a camera, lights, and render settings.
//! Each element owns animation tracks (keyframes) evaluated at a given time.

use serde::{Deserialize, Serialize};
use crate::anim::Track;

/// What kind of geometry an element draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementKind {
    /// Flat quad — the workhorse for HTML/image content. Unlit by default.
    Plane,
    /// Solid lit box — accents, bars, backdrops.
    Box,
    /// Solid lit sphere.
    Sphere,
}

impl Default for ElementKind {
    fn default() -> Self {
        ElementKind::Plane
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub id: String,
    pub name: String,
    pub kind: ElementKind,

    // Base (un-animated) transform. Animation tracks override per-frame.
    pub position: [f32; 3],
    pub rotation: [f32; 3], // euler XYZ, radians
    pub scale: [f32; 3],

    pub visible: bool,

    /// Tint / solid color, hex "#rrggbb".
    pub color: String,
    pub opacity: f32,

    /// Absolute path to a PNG/JPG to texture a plane (e.g. captured HTML).
    pub image_path: Option<String>,

    /// Plane dimensions in world units (ignored for box/sphere).
    pub width: f32,
    pub height: f32,

    /// If true, skip lighting (show texture/color flat). Planes default true.
    pub unlit: bool,

    /// Animation tracks keyed by property name:
    /// "position" | "rotation" | "scale" | "opacity"
    #[serde(default)]
    pub tracks: Vec<NamedTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedTrack {
    pub property: String,
    pub track: Track,
}

impl Element {
    pub fn new(id: String, name: String, kind: ElementKind) -> Self {
        let unlit = matches!(kind, ElementKind::Plane);
        Self {
            id,
            name,
            kind,
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            visible: true,
            color: "#4488ff".into(),
            opacity: 1.0,
            image_path: None,
            width: 2.0,
            height: 2.0,
            unlit,
            tracks: Vec::new(),
        }
    }

    pub fn track_mut(&mut self, property: &str) -> &mut Track {
        if let Some(idx) = self.tracks.iter().position(|t| t.property == property) {
            return &mut self.tracks[idx].track;
        }
        self.tracks.push(NamedTrack {
            property: property.to_string(),
            track: Track::default(),
        });
        &mut self.tracks.last_mut().unwrap().track
    }

    pub fn track(&self, property: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.property == property).map(|t| &t.track)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Camera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub fov_deg: f32,
    pub near: f32,
    pub far: f32,
    #[serde(default)]
    pub tracks: Vec<NamedTrack>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: [0.0, 1.0, 7.0],
            target: [0.0, 0.5, 0.0],
            fov_deg: 45.0,
            near: 0.1,
            far: 100.0,
            tracks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frame_start: u32,
    pub frame_end: u32,
    pub background: [f32; 4], // rgba 0..1
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30,
            frame_start: 1,
            frame_end: 300,
            background: [0.03, 0.03, 0.06, 1.0],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionalLight {
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: [-0.4, -0.8, -0.5],
            color: [1.0, 1.0, 1.0],
            intensity: 1.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub elements: Vec<Element>,
    pub camera: Camera,
    pub light: DirectionalLight,
    pub ambient: f32,
    pub render: RenderSettings,
    /// Quality tier: "lite" | "standard" | "pro". Only "lite" is implemented.
    pub mode: String,
    next_id: u64,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            elements: Vec::new(),
            camera: Camera::default(),
            light: DirectionalLight::default(),
            ambient: 0.35,
            render: RenderSettings::default(),
            mode: "lite".into(),
            next_id: 1,
        }
    }
}

impl Scene {
    pub fn alloc_id(&mut self) -> String {
        let id = format!("el_{}", self.next_id);
        self.next_id += 1;
        id
    }

    pub fn element(&self, id: &str) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id || e.name == id)
    }

    pub fn element_mut(&mut self, id: &str) -> Option<&mut Element> {
        self.elements.iter_mut().find(|e| e.id == id || e.name == id)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.elements.len();
        self.elements.retain(|e| e.id != id && e.name != id);
        self.elements.len() != before
    }

    /// Total seconds for the animation given fps.
    pub fn duration_secs(&self) -> f32 {
        (self.render.frame_end.saturating_sub(self.render.frame_start)) as f32
            / self.render.fps.max(1) as f32
    }
}

/// Parse "#rrggbb" → linear RGB (approx sRGB gamma 2.2).
pub fn hex_to_linear(hex: &str) -> [f32; 3] {
    let h = hex.trim_start_matches('#');
    if h.len() < 6 {
        return [0.5, 0.5, 0.5];
    }
    let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(128) as f32 / 255.0;
    let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(128) as f32 / 255.0;
    let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(128) as f32 / 255.0;
    [r.powf(2.2), g.powf(2.2), b.powf(2.2)]
}
