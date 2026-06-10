//! Frame-renderer driver: spawns and talks to the long-lived
//! `juicer-frame-renderer` Swift helper.
//!
//! Protocol — one JSON request per line over stdin, one JSON response per line
//! over stdout:
//!
//! Request:
//!   { "scene_doc": {layers:[{id,kind,...}], background}, "state": RenderState,
//!     "canvas_width": u32, "canvas_height": u32 }
//!
//! Response (success): { "png_base64": "..." }
//! Response (error):   { "error": "message" }
//!
//! The helper holds one offscreen WKWebView with renderer.html loaded. On every
//! request it rebuilds the layer DOM from `scene_doc`, applies per-layer styles
//! from `state`, waits for layout/paint, then snapshots a PNG.

use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::Serialize;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::html_capture;
use crate::scene::{LayerKind, Scene, ShapeKind};

/// A handle on the spawned helper. Dropping it kills the child.
pub struct Renderer {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        let helper = html_capture::find_helper_binary("juicer-frame-renderer");
        let resources_dir = locate_renderer_html_dir()?;

        let mut child = Command::new(&helper)
            .arg(&resources_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| {
                format!(
                    "spawning frame-renderer at '{}'. Build helpers with: \
                     pnpm --filter juicer-desktop helpers",
                    helper
                )
            })?;

        let stdin = child.stdin.take().context("helper has no stdin pipe")?;
        let stdout = BufReader::new(child.stdout.take().context("helper has no stdout pipe")?);
        let mut r = Self { child, stdin, stdout };
        r.wait_ready().context("frame-renderer failed to initialize")?;
        Ok(r)
    }

    fn wait_ready(&mut self) -> Result<()> {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .context("reading ready-signal from frame-renderer")?;
        let line = line.trim();
        if line.is_empty() {
            bail!("frame-renderer closed stdout without signalling ready");
        }
        let v: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("frame-renderer sent non-JSON: {line}"))?;
        if v.get("ready").and_then(|x| x.as_bool()) == Some(true) {
            Ok(())
        } else if let Some(err) = v.get("error").and_then(|x| x.as_str()) {
            bail!("frame-renderer init error: {err}")
        } else {
            bail!("frame-renderer unexpected init response: {line}")
        }
    }

    /// Render one frame of `scene` at `frame` and return raw PNG bytes.
    pub fn render_frame(&mut self, scene: &Scene, frame: f32) -> Result<Vec<u8>> {
        let req = RenderRequest {
            scene_doc: build_scene_doc(scene),
            state: scene.evaluate_at(frame),
            canvas_width: scene.canvas.width,
            canvas_height: scene.canvas.height,
        };
        let json = serde_json::to_string(&req).context("serializing render request")?;
        self.stdin.write_all(json.as_bytes())?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;

        let mut line = String::new();
        let n = self.stdout.read_line(&mut line)?;
        if n == 0 {
            bail!("frame-renderer closed stdout");
        }
        let resp: RenderResponse = serde_json::from_str(line.trim())
            .with_context(|| format!("parsing renderer response: {line}"))?;
        if let Some(e) = resp.error {
            bail!("frame-renderer error: {e}");
        }
        let b64 = resp.png_base64.context("response has no png_base64")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .context("decoding png base64")?;
        Ok(bytes)
    }

    /// Render to file. Returns the canonical written path.
    pub fn render_frame_to_file(&mut self, scene: &Scene, frame: f32, path: &str) -> Result<String> {
        let bytes = self.render_frame(scene, frame)?;
        std::fs::write(path, &bytes).with_context(|| format!("writing {path}"))?;
        Ok(path.to_string())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── Request/response payloads ─────────────────────────────────────────────────

#[derive(Serialize)]
struct RenderRequest {
    scene_doc: SceneDoc,
    state: crate::scene::RenderState,
    canvas_width: u32,
    canvas_height: u32,
}

#[derive(Serialize)]
struct SceneDoc {
    layers: Vec<LayerDoc>,
    background: String,
}

#[derive(Serialize)]
struct LayerDoc {
    id: String,
    /// "html" | "image" | "shape" | "text"
    kind: &'static str,
    /// HTML kind only — full document (already wrapped with Tailwind etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    html_srcdoc: Option<String>,
    /// Image kind only — data: URL of the image bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    image_src: Option<String>,
    /// Text kind only — initial text content.
    #[serde(skip_serializing_if = "Option::is_none")]
    text_content: Option<String>,
    /// Shape kind only — "rect" | "ellipse".
    #[serde(skip_serializing_if = "Option::is_none")]
    shape: Option<&'static str>,
}

#[derive(serde::Deserialize)]
struct RenderResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    png_base64: Option<String>,
}

/// Build the scene doc to ship to the renderer. Image layers are embedded as
/// data: URLs so the WebView doesn't need filesystem read access.
pub fn build_scene_doc_value(scene: &Scene) -> serde_json::Value {
    serde_json::to_value(build_scene_doc(scene)).unwrap_or(serde_json::Value::Null)
}

fn build_scene_doc(scene: &Scene) -> SceneDoc {
    let layers = scene
        .layers
        .iter()
        .map(|l| match &l.kind {
            LayerKind::Html { html } => LayerDoc {
                id: l.id.clone(),
                kind: "html",
                html_srcdoc: Some(html_capture::wrap_html(html, &[], None)),
                image_src: None,
                text_content: None,
                shape: None,
            },
            LayerKind::Image { src_path } => LayerDoc {
                id: l.id.clone(),
                kind: "image",
                html_srcdoc: None,
                image_src: Some(file_to_data_url(src_path).unwrap_or_default()),
                text_content: None,
                shape: None,
            },
            LayerKind::Text { content, .. } => LayerDoc {
                id: l.id.clone(),
                kind: "text",
                html_srcdoc: None,
                image_src: None,
                text_content: Some(content.clone()),
                shape: None,
            },
            LayerKind::Shape { shape, .. } => LayerDoc {
                id: l.id.clone(),
                kind: "shape",
                html_srcdoc: None,
                image_src: None,
                text_content: None,
                shape: Some(match shape {
                    ShapeKind::Rect => "rect",
                    ShapeKind::Ellipse => "ellipse",
                }),
            },
        })
        .collect();
    SceneDoc { layers, background: scene.canvas.background.clone() }
}

fn file_to_data_url(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mime = guess_image_mime(path);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(format!("data:{mime};base64,{b64}"))
}

fn guess_image_mime(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") { "image/png" }
    else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") { "image/jpeg" }
    else if lower.ends_with(".gif") { "image/gif" }
    else if lower.ends_with(".webp") { "image/webp" }
    else if lower.ends_with(".svg") { "image/svg+xml" }
    else { "application/octet-stream" }
}

fn locate_renderer_html_dir() -> Result<String> {
    let p: PathBuf = html_capture::find_resource("renderer.html").with_context(|| {
        "renderer.html not found. Expected in src-tauri/resources/ (dev) or \
         bundled into the .app's Resources dir."
    })?;
    let dir = p.parent().context("renderer.html has no parent dir")?;
    Ok(Path::new(dir).to_string_lossy().to_string())
}
