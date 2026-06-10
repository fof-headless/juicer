/// juicer-frame-renderer
///
/// Long-lived helper that holds one offscreen WKWebView with renderer.html
/// loaded. Accepts one JSON request per line on stdin, returns one JSON
/// response per line on stdout.
///
/// Usage: juicer-frame-renderer <resources_dir>
///   resources_dir must contain renderer.html (and runtime.js + tailwind.js).
///
/// Protocol:
///   Ready signal (emitted once on startup):
///       {"ready": true}
///
///   Request (one per line):
///       {
///         "scene_doc":   { layers: [{id, kind, html_srcdoc?, image_src?, ...}], background },
///         "state":       { layers: [{id, visible, style: {...}, text?}], canvas_width, canvas_height },
///         "canvas_width": 1920,
///         "canvas_height": 1080
///       }
///
///   Response (one per line):
///       {"png_base64": "..."}             on success
///       {"error":      "message"}         on failure
///
/// Built with: swiftc main.swift -O -o juicer-frame-renderer \
///             -framework WebKit -framework AppKit

import AppKit
import WebKit
import ImageIO
import UniformTypeIdentifiers

// ── Args ──────────────────────────────────────────────────────────────────────
let args = CommandLine.arguments
guard args.count >= 2 else {
    FileHandle.standardError.write("Usage: juicer-frame-renderer <resources_dir>\n".data(using: .utf8)!)
    exit(1)
}
let resourcesDir = args[1]
let rendererHTMLPath = (resourcesDir as NSString).appendingPathComponent("renderer.html")
guard FileManager.default.fileExists(atPath: rendererHTMLPath) else {
    emitError("renderer.html not found at \(rendererHTMLPath)")
    exit(1)
}

// ── Output helpers (stderr for diagnostics, stdout strictly JSON lines) ───────
let stdout = FileHandle.standardOutput
let stderr = FileHandle.standardError

func writeLine(_ json: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: json, options: []) else {
        stderr.write("failed to serialize response\n".data(using: .utf8)!)
        return
    }
    stdout.write(data)
    stdout.write("\n".data(using: .utf8)!)
}

func emitError(_ msg: String) {
    writeLine(["error": msg])
}

// ── WKWebView host ────────────────────────────────────────────────────────────
class Host: NSObject, WKNavigationDelegate {
    let webView: WKWebView
    var loaded: Bool = false
    var pendingLoad: ((Result<Void, Error>) -> Void)? = nil
    var lastSize: CGSize = .zero

    override init() {
        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = true
        self.webView = WKWebView(frame: CGRect(origin: .zero, size: CGSize(width: 1920, height: 1080)),
                                  configuration: config)
        super.init()
        self.webView.navigationDelegate = self
        self.webView.setValue(false, forKey: "drawsBackground")
    }

    func loadRenderer(htmlURL: URL) async throws {
        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            self.pendingLoad = { result in
                switch result {
                case .success: cont.resume()
                case .failure(let e): cont.resume(throwing: e)
                }
            }
            // Grant access to the parent dir so the page can fetch runtime.js + tailwind.js.
            self.webView.loadFileURL(htmlURL, allowingReadAccessTo: htmlURL.deletingLastPathComponent())
        }
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        loaded = true
        pendingLoad?(.success(()))
        pendingLoad = nil
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        pendingLoad?(.failure(error))
        pendingLoad = nil
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        pendingLoad?(.failure(error))
        pendingLoad = nil
    }

    /// Render one frame and return the base64-encoded PNG.
    func renderFrame(payload: [String: Any]) async throws -> String {
        let w = (payload["canvas_width"] as? NSNumber)?.doubleValue ?? 1920
        let h = (payload["canvas_height"] as? NSNumber)?.doubleValue ?? 1080
        let size = CGSize(width: w, height: h)
        if size != lastSize {
            await MainActor.run { self.webView.frame = CGRect(origin: .zero, size: size) }
            lastSize = size
        }

        let _ = try await self.webView.callAsyncJavaScript(
            "return await window.__juicer.renderFrame(payload);",
            arguments: ["payload": payload],
            in: nil,
            contentWorld: .page
        )

        // Snapshot. snapshotWidth is in *points* — on Retina displays the
        // returned image is points × backingScaleFactor pixels. To get a
        // pixel-exact PNG we downscale into a target-size bitmap below.
        let config = WKSnapshotConfiguration()
        config.rect = CGRect(origin: .zero, size: size)
        config.afterScreenUpdates = true

        let image: NSImage = try await withCheckedThrowingContinuation { cont in
            self.webView.takeSnapshot(with: config) { image, error in
                if let error = error { cont.resume(throwing: error); return }
                guard let image = image else {
                    cont.resume(throwing: NSError(domain: "juicer", code: -1, userInfo: [NSLocalizedDescriptionKey: "snapshot returned nil"]))
                    return
                }
                cont.resume(returning: image)
            }
        }

        // Resize to exact canvas pixels (so MP4 frames match scene.canvas dims
        // regardless of the display's backing scale factor).
        let pixelW = Int(w.rounded())
        let pixelH = Int(h.rounded())
        guard let cgRef = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
            throw NSError(domain: "juicer", code: -3, userInfo: [NSLocalizedDescriptionKey: "no CGImage backing"])
        }
        guard let colorSpace = CGColorSpace(name: CGColorSpace.sRGB) else {
            throw NSError(domain: "juicer", code: -4, userInfo: [NSLocalizedDescriptionKey: "no sRGB color space"])
        }
        guard let cgCtx = CGContext(
            data: nil,
            width: pixelW,
            height: pixelH,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            throw NSError(domain: "juicer", code: -5, userInfo: [NSLocalizedDescriptionKey: "could not create CGContext"])
        }
        cgCtx.interpolationQuality = .high
        cgCtx.draw(cgRef, in: CGRect(x: 0, y: 0, width: pixelW, height: pixelH))
        guard let resized = cgCtx.makeImage() else {
            throw NSError(domain: "juicer", code: -6, userInfo: [NSLocalizedDescriptionKey: "no resized CGImage"])
        }
        // Encode the resized CGImage to PNG via ImageIO — bypasses NSBitmapImageRep,
        // which mysteriously double-scales on Retina even with explicit pixel dims.
        let pngOut = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(
            pngOut, UTType.png.identifier as CFString, 1, nil
        ) else {
            throw NSError(domain: "juicer", code: -7, userInfo: [NSLocalizedDescriptionKey: "no PNG destination"])
        }
        CGImageDestinationAddImage(dest, resized, nil)
        guard CGImageDestinationFinalize(dest) else {
            throw NSError(domain: "juicer", code: -8, userInfo: [NSLocalizedDescriptionKey: "PNG finalize failed"])
        }
        return (pngOut as Data).base64EncodedString()
    }
}

// ── Main loop ─────────────────────────────────────────────────────────────────
let app = NSApplication.shared
app.setActivationPolicy(.prohibited)  // Don't appear in Dock.

let host = Host()

// Bootstrap: load renderer.html, then signal ready.
Task.detached {
    do {
        try await host.loadRenderer(htmlURL: URL(fileURLWithPath: rendererHTMLPath))
        writeLine(["ready": true])
    } catch {
        emitError("failed to load renderer.html: \(error.localizedDescription)")
        exit(1)
    }

    // Then read requests from stdin, one per line.
    // Swift.readLine() is blocking but inside a detached Task it runs on a
    // background thread so the main run loop (driving the WKWebView) is
    // unaffected. Returns nil on EOF.
    while true {
        guard let line = Swift.readLine(strippingNewline: true) else {
            // EOF — parent closed stdin; exit gracefully.
            exit(0)
        }
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { continue }

        guard let data = trimmed.data(using: .utf8),
              let payload = (try? JSONSerialization.jsonObject(with: data, options: [])) as? [String: Any]
        else {
            emitError("invalid request JSON: \(trimmed.prefix(120))")
            continue
        }

        do {
            let b64 = try await host.renderFrame(payload: payload)
            writeLine(["png_base64": b64])
        } catch {
            emitError("render error: \(error.localizedDescription)")
        }
    }
}

// Drive the run loop so async tasks and the WKWebView can progress.
RunLoop.main.run()

/// Read one '\n'-terminated line from stdin synchronously. Returns nil on EOF.
func readLineFromStdin(_ fh: FileHandle) -> String? {
    var buffer = Data()
    while true {
        let chunk = fh.availableData
        if chunk.isEmpty {
            // EOF.
            return buffer.isEmpty ? nil : String(data: buffer, encoding: .utf8)
        }
        buffer.append(chunk)
        // Check for newline.
        if let nlRange = buffer.range(of: Data([0x0A])) {
            let lineData = buffer.subdata(in: 0..<nlRange.lowerBound)
            // Any leftover after the newline is the start of the next line;
            // we don't have a peek-back mechanism with FileHandle.availableData,
            // so it's simpler to assume one line per readChunk arrival. In
            // practice Rust's BufWriter sends one line at a time and flushes,
            // so this assumption holds. If we ever get multiple lines in one
            // chunk we drop the extras — caller should retry.
            return String(data: lineData, encoding: .utf8)
        }
    }
}
