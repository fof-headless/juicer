/// juicer-encoder
///
/// Encodes a directory of PNG frames into an H.264 MP4 using AVFoundation.
/// No ffmpeg, no external deps — pure macOS native.
///
/// Usage: juicer-encoder <frames_dir> <output.mp4> <fps>
/// Frames must be named frame_00000.png, frame_00001.png, ...
///
/// Build: swiftc main.swift -o juicer-encoder \
///        -framework AVFoundation -framework AppKit -framework CoreMedia

import AVFoundation
import AppKit

let args = CommandLine.arguments
guard args.count >= 4 else {
    fputs("Usage: juicer-encoder <frames_dir> <output.mp4> <fps>\n", stderr)
    exit(1)
}

let framesDir = args[1]
let outputPath = args[2]
let fps = Int32(args[3]) ?? 30

// Gather frames in order.
let fm = FileManager.default
guard let files = try? fm.contentsOfDirectory(atPath: framesDir) else {
    fputs("Cannot read frames dir: \(framesDir)\n", stderr)
    exit(1)
}
let frames = files
    .filter { $0.hasSuffix(".png") }
    .sorted()
    .map { "\(framesDir)/\($0)" }

guard !frames.isEmpty else {
    fputs("No PNG frames found in \(framesDir)\n", stderr)
    exit(1)
}

// Determine dimensions from the first frame.
guard let firstImage = NSImage(contentsOfFile: frames[0]),
      let firstRep = firstImage.representations.first else {
    fputs("Cannot load first frame\n", stderr)
    exit(1)
}
let width = firstRep.pixelsWide
let height = firstRep.pixelsHigh

// Remove existing output.
try? fm.removeItem(atPath: outputPath)

let outputURL = URL(fileURLWithPath: outputPath)
guard let writer = try? AVAssetWriter(outputURL: outputURL, fileType: .mp4) else {
    fputs("Cannot create AVAssetWriter\n", stderr)
    exit(1)
}

let settings: [String: Any] = [
    AVVideoCodecKey: AVVideoCodecType.h264,
    AVVideoWidthKey: width,
    AVVideoHeightKey: height,
    AVVideoCompressionPropertiesKey: [
        AVVideoAverageBitRateKey: width * height * 8,
        AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
    ],
]

let input = AVAssetWriterInput(mediaType: .video, outputSettings: settings)
input.expectsMediaDataInRealTime = false

let attrs: [String: Any] = [
    kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32ARGB,
    kCVPixelBufferWidthKey as String: width,
    kCVPixelBufferHeightKey as String: height,
]
let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: attrs)

writer.add(input)
writer.startWriting()
writer.startSession(atSourceTime: .zero)

func pixelBuffer(from path: String) -> CVPixelBuffer? {
    guard let image = NSImage(contentsOfFile: path) else { return nil }
    var rect = NSRect(x: 0, y: 0, width: width, height: height)
    guard let cgImage = image.cgImage(forProposedRect: &rect, context: nil, hints: nil) else { return nil }

    var pb: CVPixelBuffer?
    CVPixelBufferCreate(kCFAllocatorDefault, width, height, kCVPixelFormatType_32ARGB, attrs as CFDictionary, &pb)
    guard let buffer = pb else { return nil }

    CVPixelBufferLockBaseAddress(buffer, [])
    let ctx = CGContext(
        data: CVPixelBufferGetBaseAddress(buffer),
        width: width, height: height,
        bitsPerComponent: 8,
        bytesPerRow: CVPixelBufferGetBytesPerRow(buffer),
        space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue
    )
    ctx?.draw(cgImage, in: CGRect(x: 0, y: 0, width: width, height: height))
    CVPixelBufferUnlockBaseAddress(buffer, [])
    return buffer
}

let frameDuration = CMTime(value: 1, timescale: fps)
var frameIndex: Int64 = 0
let queue = DispatchQueue(label: "juicer.encode")
let sema = DispatchSemaphore(value: 0)

input.requestMediaDataWhenReady(on: queue) {
    while input.isReadyForMoreMediaData {
        if frameIndex >= frames.count {
            input.markAsFinished()
            writer.finishWriting {
                sema.signal()
            }
            return
        }
        let path = frames[Int(frameIndex)]
        if let buffer = pixelBuffer(from: path) {
            let pts = CMTime(value: frameIndex, timescale: fps)
            _ = frameDuration  // duration implied by pts spacing
            if !adaptor.append(buffer, withPresentationTime: pts) {
                fputs("Failed to append frame \(frameIndex)\n", stderr)
            }
        }
        frameIndex += 1
    }
}

sema.wait()

if writer.status == .completed {
    print("Encoded \(frames.count) frames → \(outputPath)")
    exit(0)
} else {
    fputs("Encode failed: \(writer.error?.localizedDescription ?? "unknown")\n", stderr)
    exit(1)
}
