//! Drives the Juicer fork of Blender (see /blender-fork) as the render engine.
//!
//! The forked `blender` binary accepts `--juicer <scene.json>` and ingests the
//! scene natively (no Python). Juicer writes scene.json, then runs:
//!
//!   blender --background --juicer scene.json -o <out> -a
//!
//! This is the "Pro" path chosen over the native wgpu renderer: Blender's real
//! keyframe/F-curve engine and Cycles/Eevee do the work.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Locate the forked Blender binary. Order: env override, bundled in the app,
/// the build-fork output dir, then PATH.
pub fn find_blender() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("JUICER_BLENDER") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    // Bundled next to the app (Contents/Resources/blender on macOS).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(macos) = exe.parent() {
            for cand in [
                macos.join("blender"),
                macos.join("../Resources/blender/blender"),
                macos.join("../Resources/blender"),
            ] {
                if cand.exists() {
                    return Some(cand);
                }
            }
        }
    }
    // Dev: the build-fork.sh output.
    for rel in [
        "blender-fork/build-blender/bin/blender",
        "../blender-fork/build-blender/bin/blender",
        "../../blender-fork/build-blender/bin/blender",
    ] {
        let p = Path::new(rel);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

/// Render the scene.json at `scene_path` to `out_path` using the forked Blender.
/// If `single_frame` is Some(f), only that frame is rendered (fast preview).
pub fn render(scene_path: &str, out_path: &str, single_frame: Option<u32>) -> Result<String> {
    let blender = find_blender().context(
        "Juicer's Blender fork not found. Build it: cd blender-fork && ./build-fork.sh \
         (or set JUICER_BLENDER to the binary).",
    )?;

    let mut cmd = Command::new(&blender);
    cmd.args(["--background", "--factory-startup", "--juicer", scene_path, "-o", out_path]);

    if let Some(f) = single_frame {
        cmd.env("JUICER_FRAME", f.to_string());
        cmd.args(["-f", &f.to_string()]);
    } else {
        cmd.arg("-a"); // render the full animation range
    }

    let output = cmd.output().with_context(|| {
        format!("failed to launch forked Blender at {}", blender.display())
    })?;

    if !output.status.success() {
        anyhow::bail!(
            "Blender fork render failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(out_path.to_string())
}
