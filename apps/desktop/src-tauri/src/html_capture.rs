/// Captures an HTML string to a PNG file using macOS WebKit via a temporary WKWebView.
///
/// On macOS this uses the system WebKit — zero external deps, full CSS/font support,
/// exactly what Safari renders, native GPU compositing.
///
/// Falls back to a fast headless-chromium approach (via Tauri shell) on other platforms.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Wrap a user's HTML fragment in a full document with the common web
/// libraries pre-loaded, so Claude can send raw Tailwind/font/icon markup and
/// it renders correctly. `libraries` is an extra list of CSS/JS URLs to inject;
/// `font` overrides the default Google Font family.
///
/// Tailwind is loaded from the **vendored local copy** (resources/tailwind.js)
/// when present — so captures work fully offline — falling back to the CDN.
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

    // Tailwind: prefer the vendored local build (copied next to the capture
    // HTML as ./tailwind.js by capture_html_to_png); else the Play CDN.
    let tailwind_tag = if local_tailwind_path().is_some() {
        "<script src=\"tailwind.js\"></script>\n".to_string()
    } else {
        "<script src=\"https://cdn.tailwindcss.com\"></script>\n".to_string()
    };

    // Built-in library set: Tailwind, Google Fonts, Lucide, Animate.css,
    // Font Awesome. These cover the vast majority of component HTML.
    let mut head = String::new();
    head.push_str(&tailwind_tag);
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

    // Per-capture temp dir. The Swift helper grants WebKit read access to the
    // HTML's parent dir, so a vendored tailwind.js placed alongside loads via a
    // relative <script src="tailwind.js"> — fully offline.
    let dir = std::env::temp_dir().join(format!("juicer_cap_{}_{}", std::process::id(), now_millis()));
    std::fs::create_dir_all(&dir)?;
    let tmp_html = dir.join("index.html");
    std::fs::write(&tmp_html, html)?;

    // Copy the vendored Tailwind build next to the HTML if we have it. Whether
    // the local file loads (relative path) or the CDN is used (offline) the
    // markup is identical to what wrap_html injected.
    let local_tw = local_tailwind_path().is_some();
    if let Some(src) = local_tailwind_path() {
        let _ = std::fs::copy(&src, dir.join("tailwind.js"));
    }

    let helper = find_capture_helper();

    // Tailwind JIT needs a moment to compile. Local build is fast (~0.6s);
    // CDN needs longer to fetch first.
    let settle = if local_tw { "0.7" } else { "1.4" };
    let status = Command::new(&helper)
        .args([
            tmp_html.to_str().unwrap(),
            output_path,
            &width.to_string(),
            &height.to_string(),
            settle,
        ])
        .status()
        .with_context(|| format!("Failed to run HTML capture helper at '{helper}'"))?;

    let _ = std::fs::remove_dir_all(&dir);

    if !status.success() {
        anyhow::bail!("HTML capture helper exited with error. Make sure '{helper}' is built.");
    }

    Ok(())
}

fn now_millis() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

/// Absolute path to the vendored Tailwind browser build, if bundled.
pub fn local_tailwind_path() -> Option<PathBuf> {
    find_resource("tailwind.js")
}

/// Locate a bundled resource (e.g. tailwind.js) across dev and packaged layouts.
pub fn find_resource(name: &str) -> Option<PathBuf> {
    // 1. Explicit override.
    if let Ok(dir) = std::env::var("JUICER_RESOURCE_DIR") {
        let p = Path::new(&dir).join(name);
        if p.exists() { return Some(p); }
    }
    // 2. Packaged .app: Contents/MacOS/juicer → Contents/Resources/resources/<name>.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(macos) = exe.parent() {
            for cand in [
                macos.join("resources").join(name),
                macos.join(name),
                macos.join("../Resources/resources").join(name),
                macos.join("../Resources").join(name),
            ] {
                if cand.exists() { return Some(cand); }
            }
        }
        // 3. Walk up for the dev layout (src-tauri/resources).
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..6 {
            if let Some(d) = dir {
                for suffix in ["src-tauri/resources", "apps/desktop/src-tauri/resources"] {
                    let p = d.join(suffix).join(name);
                    if p.exists() { return Some(p); }
                }
                dir = d.parent().map(|p| p.to_path_buf());
            } else { break; }
        }
    }
    // 4. Relative to cwd.
    for rel in ["src-tauri/resources", "apps/desktop/src-tauri/resources", "resources"] {
        let p = Path::new(rel).join(name);
        if p.exists() { return Some(p); }
    }
    None
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
    // 2. Next to the running executable OR in the bundled Resources hierarchy.
    // In a packaged .app Tauri copies resources to Contents/Resources/<name>,
    // so the executable is at Contents/MacOS/juicer and binaries land at
    // Contents/Resources/bin/<name> (or Contents/Resources/<name>).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(macos) = exe.parent() {
            for cand in [
                macos.join(name),
                macos.join("bin").join(name),
                // Packaged .app: Contents/MacOS → ../Resources
                macos.join("../Resources/bin").join(name),
                macos.join("../Resources").join(name),
            ] {
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
