/// Captures an HTML string to a PNG file using macOS WebKit via a temporary WKWebView.
///
/// On macOS this uses the system WebKit — zero external deps, full CSS/font support,
/// exactly what Safari renders, native GPU compositing.
///
/// Falls back to a fast headless-chromium approach (via Tauri shell) on other platforms.

use anyhow::{Context, Result};
use std::path::Path;

/// Wrap a user's HTML fragment in a full document with the common web
/// libraries pre-loaded, so Claude can send raw Tailwind/font/icon markup and
/// it renders correctly. `libraries` is an extra list of CSS/JS URLs to inject;
/// `font` overrides the default Google Font family.
pub fn wrap_html(fragment: &str, libraries: &[String], font: Option<&str>) -> String {
    // If the caller already sent a full document, don't double-wrap it.
    let looks_complete = {
        let lower = fragment.trim_start().to_lowercase();
        lower.starts_with("<!doctype") || lower.starts_with("<html")
    };
    if looks_complete {
        return fragment.to_string();
    }

    let font_family = font.unwrap_or("Inter");
    let font_param = font_family.replace(' ', "+");

    // Built-in library set: Tailwind (JIT via Play CDN), Google Fonts, Lucide,
    // Animate.css, Font Awesome. These cover the vast majority of component HTML.
    let mut head = String::new();
    head.push_str("<script src=\"https://cdn.tailwindcss.com\"></script>\n");
    head.push_str(&format!(
        "<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\
         <link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin>\
         <link href=\"https://fonts.googleapis.com/css2?family={font_param}:wght@300;400;500;600;700;800;900&display=swap\" rel=\"stylesheet\">\n"
    ));
    head.push_str("<link rel=\"stylesheet\" href=\"https://cdnjs.cloudflare.com/ajax/libs/animate.css/4.1.1/animate.min.css\">\n");
    head.push_str("<link rel=\"stylesheet\" href=\"https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1/css/all.min.css\">\n");
    head.push_str("<script src=\"https://unpkg.com/lucide@latest\"></script>\n");

    // Any caller-supplied libraries (URLs): .css → <link>, otherwise <script>.
    for url in libraries {
        if url.trim_end().to_lowercase().ends_with(".css") {
            head.push_str(&format!("<link rel=\"stylesheet\" href=\"{url}\">\n"));
        } else {
            head.push_str(&format!("<script src=\"{url}\"></script>\n"));
        }
    }

    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n{head}\
         <style>\n\
           html,body {{ margin:0; padding:0; background:transparent; \
           font-family:'{font_family}',system-ui,-apple-system,sans-serif; }}\n\
           *,*::before,*::after {{ box-sizing:border-box; }}\n\
         </style>\n</head>\n<body>\n{fragment}\n\
         <script>window.addEventListener('load',()=>{{try{{lucide.createIcons()}}catch(e){{}}}});</script>\n\
         </body>\n</html>\n"
    )
}

#[cfg(target_os = "macos")]
pub async fn capture_html_to_png(html: &str, width: u32, height: u32, output_path: &str) -> Result<()> {
    use std::process::Command;

    // Write HTML to a temp file
    let tmp_html = std::env::temp_dir().join(format!("juicer_capture_{}.html", std::process::id()));
    std::fs::write(&tmp_html, html)?;

    // Use the bundled Swift capture helper (see tools/html-capture/main.swift)
    let helper = find_capture_helper();

    // Library CDNs (Tailwind JIT especially) need a moment to fetch + compile.
    // Pass a generous settle delay as the 5th arg.
    let status = Command::new(&helper)
        .args([
            tmp_html.to_str().unwrap(),
            output_path,
            &width.to_string(),
            &height.to_string(),
            "1.2",
        ])
        .status()
        .with_context(|| format!("Failed to run HTML capture helper at '{helper}'"))?;

    let _ = std::fs::remove_file(&tmp_html);

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
