//! Video export: render every frame in the range, then encode to MP4.
//!
//! Encoding uses a small native Swift/AVFoundation helper (juicer-encoder) so
//! we ship zero external binaries (no ffmpeg). The helper takes a PNG sequence
//! directory + fps and writes an H.264 MP4 via AVAssetWriter.

use anyhow::{Context, Result};
use crate::render::Renderer;
use crate::scene::Scene;

pub fn render_animation(renderer: &mut Renderer, scene: &Scene, output_path: &str) -> Result<String> {
    let start = scene.render.frame_start;
    let end = scene.render.frame_end.max(start);
    let fps = scene.render.fps.max(1);

    // Temp dir for the frame sequence.
    let dir = std::env::temp_dir().join(format!("juicer_render_{}", std::process::id()));
    std::fs::create_dir_all(&dir).context("creating render temp dir")?;

    let mut frame_paths = Vec::new();
    for (i, frame) in (start..=end).enumerate() {
        let path = dir.join(format!("frame_{:05}.png", i));
        let path_str = path.to_string_lossy().to_string();
        renderer
            .render_to_png(scene, frame as f32, &path_str)
            .with_context(|| format!("rendering frame {frame}"))?;
        frame_paths.push(path_str);
    }

    let out = if output_path.ends_with(".mp4") {
        output_path.to_string()
    } else {
        format!("{output_path}.mp4")
    };

    encode_mp4(&dir.to_string_lossy(), fps, &out)
        .context("encoding MP4 (is juicer-encoder built?)")?;

    // Clean up frames.
    let _ = std::fs::remove_dir_all(&dir);

    Ok(out)
}

#[cfg(target_os = "macos")]
fn encode_mp4(frames_dir: &str, fps: u32, output: &str) -> Result<()> {
    use std::process::Command;
    let helper = find_encoder();
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

#[cfg(target_os = "macos")]
fn find_encoder() -> String {
    crate::html_capture::find_helper_binary("juicer-encoder")
}
