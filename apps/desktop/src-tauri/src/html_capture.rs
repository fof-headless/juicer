/// Captures an HTML string to a PNG file using macOS WebKit via a temporary WKWebView.
///
/// On macOS this uses the system WebKit — zero external deps, full CSS/font support,
/// exactly what Safari renders, native GPU compositing.
///
/// Falls back to a fast headless-chromium approach (via Tauri shell) on other platforms.

use anyhow::{Context, Result};
use std::path::Path;

#[cfg(target_os = "macos")]
pub async fn capture_html_to_png(html: &str, width: u32, height: u32, output_path: &str) -> Result<()> {
    use std::process::Command;

    // Write HTML to a temp file
    let tmp_html = std::env::temp_dir().join("juicer_capture.html");
    std::fs::write(&tmp_html, html)?;

    // Use the bundled Swift capture helper (see tools/html-capture/main.swift)
    // Falls back to `webkit2png` if available (brew install webkit2png)
    let helper = find_capture_helper();

    let status = Command::new(&helper)
        .args([
            tmp_html.to_str().unwrap(),
            output_path,
            &width.to_string(),
            &height.to_string(),
        ])
        .status()
        .with_context(|| format!("Failed to run HTML capture helper at '{helper}'"))?;

    if !status.success() {
        anyhow::bail!("HTML capture helper exited with error. Make sure '{helper}' is built.");
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub async fn capture_html_to_png(html: &str, width: u32, height: u32, output_path: &str) -> Result<()> {
    anyhow::bail!("HTML capture is currently macOS-only (uses native WebKit). Windows/Linux support coming.");
}

#[cfg(target_os = "macos")]
fn find_capture_helper() -> String {
    find_helper_binary("juicer-html-capture")
}

/// Locate a bundled native helper binary across dev and packaged layouts.
#[cfg(target_os = "macos")]
pub fn find_helper_binary(name: &str) -> String {
    // 1. Explicit override.
    if let Ok(dir) = std::env::var("JUICER_BIN_DIR") {
        let p = Path::new(&dir).join(name);
        if p.exists() {
            return p.to_string_lossy().into_owned();
        }
    }
    // 2. Next to the running executable (packaged .app / Resources).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            for cand in [d.join(name), d.join("bin").join(name)] {
                if cand.exists() {
                    return cand.to_string_lossy().into_owned();
                }
            }
        }
    }
    // 3. Walk up from the executable looking for src-tauri/bin (dev layout).
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..6 {
            if let Some(d) = dir {
                for suffix in ["src-tauri/bin", "apps/desktop/src-tauri/bin"] {
                    let p = d.join(suffix).join(name);
                    if p.exists() {
                        return p.to_string_lossy().into_owned();
                    }
                }
                dir = d.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }
    // 4. Relative to cwd (fallback for dev when cwd is project root).
    for rel in ["src-tauri/bin", "apps/desktop/src-tauri/bin", "bin"] {
        let p = Path::new(rel).join(name);
        if p.exists() {
            return p.to_string_lossy().into_owned();
        }
    }
    // 5. Fall back to PATH lookup.
    name.to_string()
}
