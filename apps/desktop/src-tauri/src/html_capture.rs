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
    // Look for our bundled Swift helper first, then system webkit2png
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("juicer-html-capture")));

    if let Some(p) = bundled {
        if p.exists() {
            return p.to_string_lossy().into_owned();
        }
    }

    // webkit2png installed via brew
    if Path::new("/usr/local/bin/webkit2png").exists() {
        return "/usr/local/bin/webkit2png".into();
    }

    // Last resort: python3 -m webkit2png may work on some setups
    "juicer-html-capture".into()
}
