/// juicer-html-capture
///
/// Renders an HTML file to a PNG using macOS native WebKit.
/// Usage: juicer-html-capture <input.html> <output.png> <width> <height>
///
/// Build: swiftc main.swift -o juicer-html-capture
///        (requires Xcode CLI tools only — no full Xcode needed)
///
/// This uses WKWebView offscreen rendering — full CSS3, WebFonts,
/// SVG, gradients, all supported. Zero external dependencies.

import AppKit
import WebKit

// ── Args ──────────────────────────────────────────────────────────────────────
let args = CommandLine.arguments
guard args.count >= 5 else {
    fputs("Usage: juicer-html-capture <input.html> <output.png> <width> <height>\n", stderr)
    exit(1)
}

let inputPath  = args[1]
let outputPath = args[2]
let width      = CGFloat(Double(args[3]) ?? 1200)
let height     = CGFloat(Double(args[4]) ?? 800)
// Optional 5th arg: settle delay in seconds (lets Tailwind/fonts load).
let settle     = args.count >= 6 ? (Double(args[5]) ?? 0.4) : 0.4

// ── Off-screen rendering ───────────────────────────────────────────────────────
class Renderer: NSObject, WKNavigationDelegate {
    let webView: WKWebView
    let outputPath: String
    let size: CGSize
    var done = false

    let settle: Double

    init(size: CGSize, outputPath: String, settle: Double) {
        self.size = size
        self.outputPath = outputPath
        self.settle = settle

        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = true
        self.webView = WKWebView(frame: CGRect(origin: .zero, size: size), configuration: config)
        super.init()
        self.webView.navigationDelegate = self
        // Transparent background so captured cards composite cleanly onto 3D
        // planes (the wrapper sets body background:transparent).
        self.webView.setValue(false, forKey: "drawsBackground")
    }

    func load(fileURL: URL) {
        webView.loadFileURL(fileURL, allowingReadAccessTo: fileURL.deletingLastPathComponent())
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        // Give CSS/fonts/Tailwind a moment to load and lay out.
        DispatchQueue.main.asyncAfter(deadline: .now() + self.settle) {
            self.snapshot()
        }
    }

    func snapshot() {
        let config = WKSnapshotConfiguration()
        config.rect = CGRect(origin: .zero, size: size)
        // Preserve alpha in the snapshot for transparent compositing.
        if #available(macOS 10.15, *) {
            config.afterScreenUpdates = true
        }
        webView.takeSnapshot(with: config) { image, error in
            if let error = error {
                fputs("Snapshot error: \(error)\n", stderr)
                exit(1)
            }
            guard let image = image,
                  let tiff = image.tiffRepresentation,
                  let bitmap = NSBitmapImageRep(data: tiff),
                  let pngData = bitmap.representation(using: .png, properties: [:])
            else {
                fputs("Failed to encode PNG\n", stderr)
                exit(1)
            }
            do {
                try pngData.write(to: URL(fileURLWithPath: self.outputPath))
                self.done = true
            } catch {
                fputs("Write error: \(error)\n", stderr)
                exit(1)
            }
        }
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────
let app = NSApplication.shared
app.setActivationPolicy(.prohibited)  // Don't appear in Dock

let renderer = Renderer(size: CGSize(width: width, height: height), outputPath: outputPath, settle: settle)
renderer.load(fileURL: URL(fileURLWithPath: inputPath))

// Run the run loop until snapshot is done (timeout scales with settle delay).
let deadline = Date().addingTimeInterval(10 + settle)
while !renderer.done && Date() < deadline {
    RunLoop.main.run(until: Date().addingTimeInterval(0.05))
}

if renderer.done {
    print("Captured: \(outputPath)")
    exit(0)
} else {
    fputs("Timed out\n", stderr)
    exit(1)
}
