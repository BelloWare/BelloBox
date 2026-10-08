// Test-only driver. The two marked sections are replaced with hash-checked,
// verbatim source from the shipped Swift converter, never Rust pixel code.
import AVFoundation
import CoreGraphics
import Foundation

/* ORIGINAL_RENDERER */

enum OracleError: Error { case invalid(String) }

@main
struct SwiftMovieOracle {
    static func append<T: FixedWidthInteger>(_ value: T, to data: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { data.append(contentsOf: $0) }
    }

    static func main() async throws {
        guard CommandLine.arguments.count == 3 else { throw OracleError.invalid("arguments") }
        let asset = AVURLAsset(url: URL(fileURLWithPath: CommandLine.arguments[1]))
        guard let track = try await asset.loadTracks(withMediaType: .video).first else {
            throw OracleError.invalid("no video")
        }
        let (naturalSize, transform) = try await track.load(.naturalSize, .preferredTransform)
        guard naturalSize == CGSize(width: 96, height: 64) else {
            throw OracleError.invalid("fixture dimensions")
        }
        let reader = try AVAssetReader(asset: asset)
        reader.timeRange = CMTimeRange(start: .zero, duration: CMTime(seconds: 1.3, preferredTimescale: 600))
        /* ORIGINAL_READER_OUTPUT */
        guard reader.canAdd(output) else { throw OracleError.invalid("reader output") }
        reader.add(output)
        guard reader.startReading() else { throw OracleError.invalid("reader start") }
        defer { reader.cancelReading() }
        let renderer = GIFFrameRenderer(naturalSize: naturalSize, transform: transform,
                                       outputSize: CGSize(width: 96, height: 64))
        var frames = Data()
        var count: UInt32 = 0
        while let sample = output.copyNextSampleBuffer() {
            guard count < 12 else { throw OracleError.invalid("extra frame") }
            try autoreleasepool {
                guard let buffer = CMSampleBufferGetImageBuffer(sample),
                      let image = renderer.render(buffer),
                      image.width == 96, image.height == 64,
                      image.bitsPerComponent == 8, image.bitsPerPixel == 32,
                      image.alphaInfo == .premultipliedLast,
                      let data = image.dataProvider?.data,
                      let bytes = CFDataGetBytePtr(data) else {
                    throw OracleError.invalid("rendered image storage")
                }
                // Inspect the original renderer's image; do not redraw, color
                // convert, clamp or otherwise normalize any reference pixel.
                let order = image.bitmapInfo.rawValue & CGBitmapInfo.byteOrderMask.rawValue
                guard order == CGBitmapInfo.byteOrderDefault.rawValue || order == CGBitmapInfo.byteOrder32Big.rawValue,
                      image.bytesPerRow >= 96 * 4,
                      CFDataGetLength(data) >= image.bytesPerRow * 64 else {
                    throw OracleError.invalid("unexpected reference byte order or stride")
                }
                let pts = CMTimeGetSeconds(CMSampleBufferGetPresentationTimeStamp(sample))
                guard pts.isFinite, pts >= 0 else { throw OracleError.invalid("PTS") }
                append(pts.bitPattern, to: &frames)
                append(UInt32(image.width), to: &frames)
                append(UInt32(image.height), to: &frames)
                append(UInt32(96 * 64 * 4), to: &frames)
                for row in 0..<64 {
                    frames.append(bytes.advanced(by: row * image.bytesPerRow), count: 96 * 4)
                }
            }
            count += 1
        }
        guard reader.status == .completed, count == 12 else {
            throw OracleError.invalid("incomplete reader")
        }
        var result = Data("BBSWIFT1".utf8)
        append(count, to: &result)
        result.append(frames)
        try result.write(to: URL(fileURLWithPath: CommandLine.arguments[2]), options: .atomic)
    }
}
