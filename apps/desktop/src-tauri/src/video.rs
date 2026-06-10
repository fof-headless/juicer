//! Video export: render every frame in the range, then encode to MP4.
//!
//! Encoding uses a small native Swift/AVFoundation helper (juicer-encoder) so
//! we ship zero external binaries (no ffmpeg). The helper takes a PNG sequence
//! directory + fps and writes an H.264 MP4 via AVAssetWriter.

use anyhow::{Context, Result};

use crate::renderer::Renderer;
use crate::scene::Scene;

/// Render every frame in `1..=scene.duration_frames`, then encode to MP4.
pub fn render_animation(renderer: &mut Renderer, scene: &Scene, output_path: &str) -> Result<String> {
    let fps = scene.canvas.fps.max(1);
    let total = scene.duration_frames.max(1);

    let dir = std::env::temp_dir().join(format!("juicer_render_{}", std::process::id()));
    std::fs::create_dir_all(&dir).context("creating render temp dir")?;

    for i in 0..total {
        let frame = i as f32;
        let path = dir.join(format!("frame_{:05}.png", i));
        renderer
            .render_frame_to_file(scene, frame, &path.to_string_lossy())
            .with_context(|| format!("rendering frame {i}"))?;
    }

    let out = if output_path.ends_with(".mp4") {
        output_path.to_string()
    } else {
        format!("{output_path}.mp4")
    };

    encode_mp4(&dir.to_string_lossy(), fps, &out)
        .context("encoding MP4 (is juicer-encoder built?)")?;

    let _ = std::fs::remove_dir_all(&dir);
    Ok(out)
}

#[cfg(target_os = "macos")]
fn encode_mp4(frames_dir: &str, fps: u32, output: &str) -> Result<()> {
    use std::process::Command;
    let helper = crate::html_capture::find_helper_binary("juicer-encoder");
    let status = Command::new(&helper)
        .args([frames_dir, output, &fps.to_string()])
        .status()
        .with_context(|| format!("running encoder '{helper}'"))?;
    if !status.success() {
        anyhow::bail!("encoder exited with error");
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn encode_mp4(_frames_dir: &str, _fps: u32, _output: &str) -> Result<()> {
    anyhow::bail!("Video encoding is currently macOS-only (uses AVFoundation).");
}
