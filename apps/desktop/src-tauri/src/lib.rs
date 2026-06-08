mod scene;
mod anim;
mod render;
mod mcp;
mod html_capture;
mod video;
mod project;
mod blender;

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::State;

use anim::{Easing, KeyValue};
use render::Renderer;
use scene::{Element, ElementKind, Scene};
use project::Project;

/// Shared application state: the scene, a lazily-created GPU renderer, and the
/// active on-disk project (where scenes/assets/renders are saved).
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

    /// Persist the current scene to the active project's scene.json (if any).
    /// Lock order is always project → scene to avoid deadlock; callers must not
    /// hold the scene lock when calling this.
    pub async fn autosave(&self) {
        let proj = self.project.lock().await;
        if let Some(p) = proj.as_ref() {
            let scene = self.scene.lock().await;
            if let Err(e) = p.save_scene(&scene) {
                eprintln!("[juicer] autosave failed: {e}");
            }
        }
    }

    /// Ensure there's an active project. If none, open-or-create "Default" and
    /// load its existing scene.json so restarts restore state.
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

// ── Scene query/mutation commands (called by the React UI) ─────────────────────

#[tauri::command]
async fn get_scene(state: State<'_, SharedState>) -> Result<serde_json::Value, String> {
    let scene = state.scene.lock().await;
    serde_json::to_value(&*scene).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
struct AddElementArgs {
    name: Option<String>,
    kind: Option<String>,
    position: Option<[f32; 3]>,
    rotation: Option<[f32; 3]>,
    scale: Option<[f32; 3]>,
    color: Option<String>,
    opacity: Option<f32>,
    image_path: Option<String>,
    width: Option<f32>,
    height: Option<f32>,
}

#[tauri::command]
async fn add_element(
    state: State<'_, SharedState>,
    args: AddElementArgs,
) -> Result<String, String> {
    let mut scene = state.scene.lock().await;
    let id = scene.alloc_id();
    let kind = parse_kind(args.kind.as_deref());
    let name = args.name.unwrap_or_else(|| format!("{:?}", kind));

    let mut el = Element::new(id.clone(), name, kind);
    if let Some(p) = args.position { el.position = p; }
    if let Some(r) = args.rotation { el.rotation = r; }
    if let Some(s) = args.scale { el.scale = s; }
    if let Some(c) = args.color { el.color = c; }
    if let Some(o) = args.opacity { el.opacity = o; }
    if let Some(w) = args.width { el.width = w; }
    if let Some(h) = args.height { el.height = h; }
    if let Some(ip) = args.image_path { el.image_path = Some(ip); }

    scene.elements.push(el);
    drop(scene);
    state.autosave().await;
    Ok(id)
}

#[tauri::command]
async fn update_element(
    state: State<'_, SharedState>,
    id: String,
    patch: serde_json::Value,
) -> Result<(), String> {
    {
        let mut scene = state.scene.lock().await;
        let el = scene.element_mut(&id).ok_or("element not found")?;
        apply_patch(el, &patch);
    }
    state.autosave().await;
    Ok(())
}

#[tauri::command]
async fn remove_element(state: State<'_, SharedState>, id: String) -> Result<(), String> {
    let removed = { state.scene.lock().await.remove(&id) };
    if removed {
        state.autosave().await;
        Ok(())
    } else {
        Err("element not found".into())
    }
}

#[tauri::command]
async fn set_keyframe(
    state: State<'_, SharedState>,
    id: String,
    frame: f32,
    property: String,
    value: serde_json::Value,
    easing: Option<String>,
) -> Result<(), String> {
    {
        let mut scene = state.scene.lock().await;
        let el = scene.element_mut(&id).ok_or("element not found")?;
        let kv = parse_keyvalue(&property, &value)?;
        let ease = parse_easing(easing.as_deref());
        el.track_mut(&property).insert(frame, kv, ease);
    }
    state.autosave().await;
    Ok(())
}

#[tauri::command]
async fn set_render_settings(
    state: State<'_, SharedState>,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<u32>,
    frame_start: Option<u32>,
    frame_end: Option<u32>,
) -> Result<(), String> {
    {
        let mut scene = state.scene.lock().await;
        if let Some(w) = width { scene.render.width = w; }
        if let Some(h) = height { scene.render.height = h; }
        if let Some(f) = fps { scene.render.fps = f; }
        if let Some(s) = frame_start { scene.render.frame_start = s; }
        if let Some(e) = frame_end { scene.render.frame_end = e; }
    }
    state.autosave().await;
    Ok(())
}

#[tauri::command]
async fn set_camera(
    state: State<'_, SharedState>,
    position: Option<[f32; 3]>,
    target: Option<[f32; 3]>,
    fov_deg: Option<f32>,
) -> Result<(), String> {
    {
        let mut scene = state.scene.lock().await;
        if let Some(p) = position { scene.camera.position = p; }
        if let Some(t) = target { scene.camera.target = t; }
        if let Some(f) = fov_deg { scene.camera.fov_deg = f; }
    }
    state.autosave().await;
    Ok(())
}

/// Render a single frame to PNG at reduced resolution, return the path (viewport preview).
/// Uses a capped 960×540 resolution so the preview is fast (~1–2s vs 30s at full res).
#[tauri::command]
async fn render_preview(
    state: State<'_, SharedState>,
    frame: f32,
) -> Result<String, String> {
    let mut scene = state.scene.lock().await.clone();
    // Cap preview resolution to 960×540 for speed (the canvas viewport handles real-time editing).
    let scale = (960.0 / scene.render.width as f32).min(1.0);
    scene.render.width = (scene.render.width as f32 * scale) as u32;
    scene.render.height = (scene.render.height as f32 * scale) as u32;

    let mut renderer_guard = state.renderer.lock().await;
    if renderer_guard.is_none() {
        *renderer_guard = Some(Renderer::new().map_err(|e| e.to_string())?);
    }
    let renderer = renderer_guard.as_mut().unwrap();

    let out = std::env::temp_dir().join("juicer_preview.png");
    let out_str = out.to_string_lossy().to_string();
    renderer
        .render_to_png(&scene, frame, &out_str)
        .map_err(|e| e.to_string())?;
    Ok(out_str)
}

/// Render the full animation to an MP4. Returns the output path.
#[tauri::command]
async fn render_video(
    state: State<'_, SharedState>,
    output_path: Option<String>,
) -> Result<String, String> {
    // Default into the active project's renders/ folder.
    let out = match output_path.filter(|s| !s.is_empty()) {
        Some(p) => p,
        None => {
            let proj = state.project.lock().await;
            match proj.as_ref() {
                Some(p) => p.render_path("output", "mp4"),
                None => std::env::temp_dir().join("juicer_output.mp4").to_string_lossy().to_string(),
            }
        }
    };
    let scene = state.scene.lock().await.clone();
    let mut renderer_guard = state.renderer.lock().await;
    if renderer_guard.is_none() {
        *renderer_guard = Some(Renderer::new().map_err(|e| e.to_string())?);
    }
    let renderer = renderer_guard.as_mut().unwrap();

    video::render_animation(renderer, &scene, &out).map_err(|e| e.to_string())
}

/// Render via the forked Blender binary (see /blender-fork). Writes the current
/// scene to the project's scene.json, then drives Blender natively (--juicer).
/// `single_frame` renders just one frame (preview); otherwise the full range.
#[tauri::command]
async fn render_blender(
    state: State<'_, SharedState>,
    output_path: Option<String>,
    single_frame: Option<u32>,
) -> Result<String, String> {
    // Persist the scene to disk so the Blender fork can ingest it.
    let scene_path = {
        let proj = state.project.lock().await;
        match proj.as_ref() {
            Some(p) => {
                p.save_scene(&*state.scene.lock().await).map_err(|e| e.to_string())?;
                p.scene_path().to_string_lossy().to_string()
            }
            None => {
                let tmp = std::env::temp_dir().join("juicer_scene.json");
                let json = serde_json::to_string_pretty(&*state.scene.lock().await)
                    .map_err(|e| e.to_string())?;
                std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
                tmp.to_string_lossy().to_string()
            }
        }
    };

    let out = match output_path.filter(|s| !s.is_empty()) {
        Some(p) => p,
        None => {
            let proj = state.project.lock().await;
            match proj.as_ref() {
                Some(p) => p.render_path("blender", "mp4"),
                None => std::env::temp_dir().join("juicer_blender.mp4").to_string_lossy().to_string(),
            }
        }
    };

    tauri::async_runtime::spawn_blocking(move || blender::render(&scene_path, &out, single_frame))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_scene_json(state: State<'_, SharedState>, path: String) -> Result<(), String> {
    let scene = state.scene.lock().await;
    let json = serde_json::to_string_pretty(&*scene).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_scene_json(state: State<'_, SharedState>, path: String) -> Result<(), String> {
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let loaded: Scene = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    *state.scene.lock().await = loaded;
    Ok(())
}

#[tauri::command]
async fn capture_html(
    state: State<'_, SharedState>,
    html: String,
    width: u32,
    height: u32,
    output_path: Option<String>,
) -> Result<String, String> {
    // Default the output into the active project's assets/ folder.
    let out = match output_path {
        Some(p) => p,
        None => {
            let proj = state.project.lock().await;
            match proj.as_ref() {
                Some(p) => p.asset_path("capture", "png"),
                None => std::env::temp_dir().join("juicer_capture.png").to_string_lossy().to_string(),
            }
        }
    };
    let wrapped = html_capture::wrap_html(&html, &[], None);
    html_capture::capture_html_to_png(&wrapped, width, height, &out)
        .await
        .map_err(|e| e.to_string())?;
    Ok(out)
}

// ── Project commands ───────────────────────────────────────────────────────────

fn project_info(p: &Project) -> serde_json::Value {
    serde_json::json!({
        "name": p.name,
        "root": p.root.to_string_lossy(),
        "assets": p.assets_dir().to_string_lossy(),
        "renders": p.renders_dir().to_string_lossy(),
    })
}

#[tauri::command]
async fn get_project(state: State<'_, SharedState>) -> Result<Option<serde_json::Value>, String> {
    let proj = state.project.lock().await;
    Ok(proj.as_ref().map(project_info))
}

#[tauri::command]
async fn create_project(
    state: State<'_, SharedState>,
    name: String,
) -> Result<serde_json::Value, String> {
    let p = Project::create(&name).map_err(|e| e.to_string())?;
    // Fresh scene for a new project.
    *state.scene.lock().await = Scene::default();
    let info = project_info(&p);
    p.save_scene(&*state.scene.lock().await).map_err(|e| e.to_string())?;
    *state.project.lock().await = Some(p);
    Ok(info)
}

#[tauri::command]
async fn open_project(
    state: State<'_, SharedState>,
    path: String,
) -> Result<serde_json::Value, String> {
    let (p, scene) = Project::open(&path).map_err(|e| e.to_string())?;
    if let Some(loaded) = scene {
        *state.scene.lock().await = loaded;
    }
    let info = project_info(&p);
    *state.project.lock().await = Some(p);
    Ok(info)
}

#[tauri::command]
async fn save_project(state: State<'_, SharedState>) -> Result<String, String> {
    let proj = state.project.lock().await;
    match proj.as_ref() {
        Some(p) => {
            p.save_scene(&*state.scene.lock().await).map_err(|e| e.to_string())?;
            Ok(p.scene_path().to_string_lossy().to_string())
        }
        None => Err("no active project".into()),
    }
}

// ── Helpers shared with MCP ────────────────────────────────────────────────────

pub fn parse_kind(s: Option<&str>) -> ElementKind {
    match s.unwrap_or("plane") {
        "box" | "cube" => ElementKind::Box,
        "sphere" => ElementKind::Sphere,
        _ => ElementKind::Plane,
    }
}

pub fn parse_easing(s: Option<&str>) -> Easing {
    match s.unwrap_or("ease-in-out") {
        "linear" => Easing::Linear,
        "ease-in" => Easing::EaseIn,
        "ease-out" => Easing::EaseOut,
        "step" => Easing::Step,
        _ => Easing::EaseInOut,
    }
}

pub fn parse_keyvalue(property: &str, value: &serde_json::Value) -> Result<KeyValue, String> {
    if property == "opacity" {
        // Accept number or string-encoded number (MCP clients may stringify untyped values).
        let s = value.as_f64()
            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
            .ok_or("opacity must be a number")? as f32;
        Ok(KeyValue::Scalar(s))
    } else {
        // Accept JSON array or a JSON string like "[0,1,0]".
        let owned;
        let arr: &[serde_json::Value] = if let Some(a) = value.as_array() {
            a
        } else if let Some(s) = value.as_str() {
            owned = serde_json::from_str::<serde_json::Value>(s)
                .ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            &owned
        } else {
            return Err("expected [x,y,z]".into());
        };
        if arr.len() != 3 {
            return Err(format!("expected 3 components, got {}", arr.len()));
        }
        let v = [
            arr[0].as_f64().unwrap_or(0.0) as f32,
            arr[1].as_f64().unwrap_or(0.0) as f32,
            arr[2].as_f64().unwrap_or(0.0) as f32,
        ];
        Ok(KeyValue::Vec3(v))
    }
}

pub fn apply_patch(el: &mut Element, patch: &serde_json::Value) {
    if let Some(v) = patch.get("name").and_then(|v| v.as_str()) { el.name = v.into(); }
    if let Some(v) = patch.get("color").and_then(|v| v.as_str()) { el.color = v.into(); }
    if let Some(v) = patch.get("opacity").and_then(|v| v.as_f64()) { el.opacity = v as f32; }
    if let Some(v) = patch.get("visible").and_then(|v| v.as_bool()) { el.visible = v; }
    if let Some(v) = patch.get("width").and_then(|v| v.as_f64()) { el.width = v as f32; }
    if let Some(v) = patch.get("height").and_then(|v| v.as_f64()) { el.height = v as f32; }
    if let Some(v) = patch.get("unlit").and_then(|v| v.as_bool()) { el.unlit = v; }
    if let Some(v) = patch.get("image_path").and_then(|v| v.as_str()) { el.image_path = Some(v.into()); }
    if let Some(arr) = patch.get("position").and_then(|v| v.as_array()) { el.position = json_vec3(arr, el.position); }
    if let Some(arr) = patch.get("rotation").and_then(|v| v.as_array()) { el.rotation = json_vec3(arr, el.rotation); }
    if let Some(arr) = patch.get("scale").and_then(|v| v.as_array()) { el.scale = json_vec3(arr, el.scale); }
}

fn json_vec3(arr: &[serde_json::Value], fallback: [f32; 3]) -> [f32; 3] {
    if arr.len() != 3 {
        return fallback;
    }
    [
        arr[0].as_f64().unwrap_or(fallback[0] as f64) as f32,
        arr[1].as_f64().unwrap_or(fallback[1] as f64) as f32,
        arr[2].as_f64().unwrap_or(fallback[2] as f64) as f32,
    ]
}

// ── Entry point ────────────────────────────────────────────────────────────────

pub fn run() {
    // If launched with --mcp, run as a pure MCP stdio server (for Claude Desktop).
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

    // Open-or-create the Default project so the UI always has a home on disk.
    tauri::async_runtime::block_on(state.ensure_default_project());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_scene,
            add_element,
            update_element,
            remove_element,
            set_keyframe,
            set_render_settings,
            set_camera,
            render_preview,
            render_video,
            render_blender,
            save_scene_json,
            load_scene_json,
            capture_html,
            get_project,
            create_project,
            open_project,
            save_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Juicer");
}
