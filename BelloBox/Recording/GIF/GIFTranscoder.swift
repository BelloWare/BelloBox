import AVFoundation
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// User-facing GIF settings. GIFs carry no sound; frame rate, width and clip
/// length are bounded so the file and the memory needed to build it stay sane.
struct GIFExportOptions: Equatable, Codable {
    var framesPerSecond: Int = 15
    /// Longest edge cap in pixels; the source is never upscaled.
    var maxWidth: Int = 640
    var loops: Bool = true
    var trimStart: TimeInterval = 0
    /// nil means the end of the clip (still bounded by `maxDuration`).
    var trimEnd: TimeInterval? = nil

    static let frameRateChoices = [10, 15, 20]
    static let frameRateRange = 5...20
    static let widthChoices = [320, 480, 640, 800, 1080]
    static let maxDuration: TimeInterval = 120
    static let maxFrames = 3_000
    static let `default` = GIFExportOptions()

    init(framesPerSecond: Int = 15, maxWidth: Int = 640, loops: Bool = true, trimStart: TimeInterval = 0, trimEnd: TimeInterval? = nil) {
        self.framesPerSecond = framesPerSecond
        self.maxWidth = maxWidth
        self.loops = loops
        self.trimStart = trimStart
        self.trimEnd = trimEnd
    }

    private enum CodingKeys: String, CodingKey { case framesPerSecond, maxWidth, loops, trimStart, trimEnd }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        framesPerSecond = try container.decodeIfPresent(Int.self, forKey: .framesPerSecond) ?? 15
        maxWidth = try container.decodeIfPresent(Int.self, forKey: .maxWidth) ?? 640
        loops = try container.decodeIfPresent(Bool.self, forKey: .loops) ?? true
        trimStart = try container.decodeIfPresent(TimeInterval.self, forKey: .trimStart) ?? 0
        trimEnd = try container.decodeIfPresent(TimeInterval.self, forKey: .trimEnd)
    }

    /// Values pulled back into range; the clip is clamped to the source once it is known.
    var normalized: GIFExportOptions {
        var copy = self
        copy.framesPerSecond = min(max(framesPerSecond, Self.frameRateRange.lowerBound), Self.frameRateRange.upperBound)
        copy.maxWidth = min(max(maxWidth, 64), 1_080)
        copy.trimStart = max(0, trimStart.isFinite ? trimStart : 0)
        if let end = trimEnd, !end.isFinite || end <= copy.trimStart { copy.trimEnd = nil }
        return copy
    }
}

/// What the converter learned about the source before planning.
struct GIFSourceInfo: Equatable {
    var duration: TimeInterval
    /// Displayed size after the track's preferred transform (a rotated phone clip is upright).
    var displaySize: CGSize
    var nominalFrameRate: Double

    static func load(url: URL) async throws -> GIFSourceInfo {
        let asset = AVURLAsset(url: url)
        guard let track = try await asset.loadTracks(withMediaType: .video).first else { throw GIFTranscoderError.noVideoTrack }
        let (naturalSize, transform, frameRate) = try await track.load(.naturalSize, .preferredTransform, .nominalFrameRate)
        let duration = try await asset.load(.duration)
        let display = CGRect(origin: .zero, size: naturalSize).applying(transform).standardized.size
        return GIFSourceInfo(duration: CMTimeGetSeconds(duration), displaySize: CGSize(width: abs(display.width), height: abs(display.height)),
                             nominalFrameRate: Double(frameRate))
    }
}

/// The concrete conversion: clip range, frame timing and output size, all bounded.
struct GIFExportPlan: Equatable {
    var start: TimeInterval
    var end: TimeInterval
    var framesPerSecond: Int
    var frameCount: Int
    var outputSize: CGSize
    var loops: Bool

    /// The clip taken from the source. `end` is pulled in when the frame cap applies,
    /// so this always equals what the GIF plays back (to the centisecond).
    var duration: TimeInterval { end - start }
    var frameDelay: TimeInterval { 1 / Double(framesPerSecond) }

    /// Clamps the trim to the clip, to `maxDuration` and to `maxFrames` (shortening
    /// the clip rather than dropping frames), and scales the display size down
    /// (never up) to the longest-edge cap.
    static func make(source: GIFSourceInfo, options: GIFExportOptions) throws -> GIFExportPlan {
        let options = options.normalized
        guard source.duration.isFinite, source.duration > 0 else { throw GIFTranscoderError.noFrames }
        guard source.displaySize.width >= 1, source.displaySize.height >= 1 else { throw GIFTranscoderError.noVideoTrack }
        let fps = Double(options.framesPerSecond)
        let start = min(options.trimStart, max(0, source.duration - 0.05))
        var end = min(options.trimEnd ?? source.duration, source.duration)
        end = min(end, start + GIFExportOptions.maxDuration)
        guard end - start >= 0.05 else { throw GIFTranscoderError.clipTooShort }
        var frameCount = max(1, Int(((end - start) * fps - 1e-6).rounded(.up)))
        if frameCount > GIFExportOptions.maxFrames {
            frameCount = GIFExportOptions.maxFrames
            end = start + Double(frameCount) / fps
        }
        let longEdge = max(source.displaySize.width, source.displaySize.height)
        let scale = min(1, CGFloat(options.maxWidth) / longEdge)
        let size = CGSize(width: max(1, (source.displaySize.width * scale).rounded()),
                          height: max(1, (source.displaySize.height * scale).rounded()))
        return GIFExportPlan(start: start, end: end, framesPerSecond: options.framesPerSecond, frameCount: frameCount,
                             outputSize: size, loops: options.loops)
    }

    /// Browsers treat delays under this as "unset" and substitute 100 ms, so no
    /// frame is ever written shorter than 2 cs.
    static let minimumDelayCentiseconds = 2

    /// GIF delays are whole centiseconds. Rounding every frame to the same value
    /// drifts (15 fps → 7 cs each → 5% slow), so the clip length is turned into a
    /// centisecond budget and each frame gets the difference of consecutive rounded
    /// cumulative times (7, 6, 7, 7, 6, 7… at 15 fps). The last frame takes whatever
    /// partial frame the clip ends on, and frames below the minimum borrow from the
    /// frames before them, so the total never moves.
    var frameDelays: [TimeInterval] {
        Self.delays(frameCount: frameCount, framesPerSecond: framesPerSecond, duration: duration).map { Double($0) / 100 }
    }

    static func delays(frameCount: Int, framesPerSecond: Int, duration: TimeInterval) -> [Int] {
        guard frameCount > 0 else { return [] }
        let fps = Double(framesPerSecond)
        let minimum = minimumDelayCentiseconds
        let total = max(frameCount * minimum, Int((duration * 100).rounded()))
        var cumulative = (0...frameCount).map { frame in min(total, Int((Double(frame) * 100 / fps).rounded())) }
        cumulative[frameCount] = total
        var delays = (0..<frameCount).map { cumulative[$0 + 1] - cumulative[$0] }
        for index in delays.indices where delays[index] < minimum {
            var need = minimum - delays[index]
            delays[index] = minimum
            var donor = index - 1
            while need > 0, donor >= 0 {
                let slack = delays[donor] - minimum
                if slack > 0 {
                    let take = min(slack, need)
                    delays[donor] -= take
                    need -= take
                }
                donor -= 1
            }
        }
        return delays
    }

    /// The length the GIF actually plays: the planned frames at their centisecond
    /// delays, which is the clip length rounded to the centisecond.
    var encodedDuration: TimeInterval { frameDelays.reduce(0, +) }

    var summary: String {
        let seconds = String(format: duration < 10 ? "%.1f s" : "%.0f s", duration)
        return "\(frameCount) frames · \(Int(outputSize.width)) × \(Int(outputSize.height)) px · \(seconds) at \(framesPerSecond) fps"
    }
}

struct GIFExportResult: Equatable {
    var url: URL
    var frameCount: Int
    var size: CGSize
    var duration: TimeInterval
    var fileSize: Int64
}

enum GIFTranscoderError: LocalizedError, Equatable {
    case noVideoTrack
    case noFrames
    case clipTooShort
    case sameFile
    case cannotRead(String)
    case cannotWrite
    case invalidOutput

    var errorDescription: String? {
        switch self {
        case .sameFile: return "The GIF cannot replace the movie it is made from. Choose a different destination."
        case .noVideoTrack: return "The file has no video track to convert."
        case .noFrames: return "No video frames could be read from the file."
        case .clipTooShort: return "The selected clip is too short to make a GIF."
        case let .cannotRead(message): return "The video could not be read. \(message)"
        case .cannotWrite: return "The GIF file could not be created."
        case .invalidOutput: return "The GIF was not written correctly, so it was discarded."
        }
    }
}

/// Movie → GIF with AVFoundation and ImageIO only. Frames are decoded sequentially,
/// scaled and oriented one at a time, and appended to an ImageIO destination at a
/// staged path beside the destination. Only a finalized file that reads back as a
/// GIF with the planned frame count is renamed into place; cancellation or any
/// failure removes the staged file and never touches the source.
enum GIFTranscoder {
    typealias Progress = @Sendable (Double) -> Void

    static func transcode(sourceURL: URL, to destinationURL: URL, options: GIFExportOptions,
                          progress: @escaping Progress = { _ in }) async throws -> GIFExportResult {
        try Task.checkCancellation()
        let destination = RecordingFileStore.resolved(destinationURL)
        guard !Self.isSameFile(sourceURL, destination) else { throw GIFTranscoderError.sameFile }
        let source = try await GIFSourceInfo.load(url: sourceURL)
        let plan = try GIFExportPlan.make(source: source, options: options)
        let asset = AVURLAsset(url: sourceURL)
        guard let track = try await asset.loadTracks(withMediaType: .video).first else { throw GIFTranscoderError.noVideoTrack }
        let (naturalSize, transform) = try await track.load(.naturalSize, .preferredTransform)
        let staged = RecordingFileStore.stagedURL(for: destination, pathExtension: "gif")
        let flag = CancellationFlag()
        let readerBox = ReaderBox()
        let input = EncodeInput(asset: asset, track: track, naturalSize: naturalSize, transform: transform)
        let work = Task.detached(priority: .userInitiated) { () throws -> Int in
            defer { readerBox.clear() }
            return try encode(input, staged: staged, plan: plan, flag: flag, readerBox: readerBox, progress: progress)
        }
        do {
            let frames = try await withTaskCancellationHandler {
                try await work.value
            } onCancel: {
                flag.cancel()
                readerBox.cancel()
            }
            try Task.checkCancellation()
            if !plan.loops {
                // ImageIO writes a NETSCAPE repeat block even when no loop count is set,
                // which browsers read as one extra repeat. A one-shot GIF must not carry it.
                _ = try GIFLoopBlock.strip(at: staged)
            }
            try validate(staged, expectedFrames: frames, loops: plan.loops)
            try RecordingFileStore.publish(staged, to: destination)
        } catch {
            try? FileManager.default.removeItem(at: staged)
            if flag.isCancelled || Task.isCancelled { throw CancellationError() }
            throw error
        }
        let size = (try? FileManager.default.attributesOfItem(atPath: destination.path)[.size] as? NSNumber)?.int64Value ?? 0
        return GIFExportResult(url: destination, frameCount: plan.frameCount, size: plan.outputSize, duration: plan.encodedDuration, fileSize: size)
    }

    /// The destination may never be the source: same path once symlinks are resolved,
    /// or the same file under another name (a hard link or alias), which the final
    /// rename would otherwise replace.
    static func isSameFile(_ source: URL, _ destination: URL) -> Bool {
        let source = RecordingFileStore.resolved(source)
        let destination = RecordingFileStore.resolved(destination)
        if source.path == destination.path { return true }
        guard FileManager.default.fileExists(atPath: destination.path) else { return false }
        let key: Set<URLResourceKey> = [.fileResourceIdentifierKey]
        guard let a = try? source.resourceValues(forKeys: key).fileResourceIdentifier,
              let b = try? destination.resourceValues(forKeys: key).fileResourceIdentifier
        else { return false }
        return a.isEqual(b)
    }

    /// Reads back the staged file: it must be a GIF with exactly the frames that were
    /// planned and the loop metadata the user chose (a repeat block for forever, none
    /// for once).
    static func validate(_ url: URL, expectedFrames: Int, loops: Bool) throws {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary),
              let type = CGImageSourceGetType(source), UTType(type as String)?.conforms(to: .gif) == true,
              CGImageSourceGetCount(source) == expectedFrames, expectedFrames > 0
        else { throw GIFTranscoderError.invalidOutput }
        // ImageIO's reader reports 1 for a file with no repeat block, so the file's
        // own structure is what gets checked.
        guard let data = try? Data(contentsOf: url, options: .mappedIfSafe) else { throw GIFTranscoderError.invalidOutput }
        let block = GIFLoopBlock.locate(in: data)
        guard loops ? block?.loopCount == 0 : block == nil else { throw GIFTranscoderError.invalidOutput }
    }

    private struct EncodeInput: @unchecked Sendable {
        let asset: AVURLAsset
        let track: AVAssetTrack
        let naturalSize: CGSize
        let transform: CGAffineTransform
    }

    /// Blocking: runs on a worker task. Returns the number of frames written.
    private static func encode(_ input: EncodeInput, staged: URL, plan: GIFExportPlan, flag: CancellationFlag,
                               readerBox: ReaderBox, progress: @escaping Progress) throws -> Int {
        let asset = input.asset, track = input.track, naturalSize = input.naturalSize, transform = input.transform
        let reader: AVAssetReader
        do { reader = try AVAssetReader(asset: asset) } catch { throw GIFTranscoderError.cannotRead(error.localizedDescription) }
        // Read to the end of the clip plus one frame of tolerance; samples past the
        // trim end are never picked below, so nothing after the cut can appear.
        reader.timeRange = CMTimeRange(start: CMTime(seconds: plan.start, preferredTimescale: 600),
                                       duration: CMTime(seconds: plan.duration + plan.frameDelay, preferredTimescale: 600))
        let output = AVAssetReaderTrackOutput(track: track, outputSettings: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
        ])
        output.alwaysCopiesSampleData = false
        guard reader.canAdd(output) else { throw GIFTranscoderError.cannotRead("The video format is not supported.") }
        reader.add(output)
        readerBox.set(reader)
        guard reader.startReading() else { throw GIFTranscoderError.cannotRead(reader.error?.localizedDescription ?? "Unknown error.") }

        try? FileManager.default.removeItem(at: staged)
        try FileManager.default.createDirectory(at: staged.deletingLastPathComponent(), withIntermediateDirectories: true)
        guard let destination = CGImageDestinationCreateWithURL(staged as CFURL, UTType.gif.identifier as CFString, plan.frameCount, nil) else {
            throw GIFTranscoderError.cannotWrite
        }
        // A NETSCAPE block with 0 repeats forever. A one-shot GIF must carry no block
        // at all (any count N means N extra plays in browsers), so nothing is set.
        if plan.loops {
            CGImageDestinationSetProperties(destination, [
                kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFLoopCount: 0],
            ] as CFDictionary)
        }
        let delays = plan.frameDelays
        func frameProperties(_ index: Int) -> CFDictionary {
            let delay = delays[min(index, delays.count - 1)]
            return [
                kCGImagePropertyGIFDictionary: [
                    kCGImagePropertyGIFDelayTime: delay,
                    kCGImagePropertyGIFUnclampedDelayTime: delay,
                ],
            ] as CFDictionary
        }

        let renderer = GIFFrameRenderer(naturalSize: naturalSize, transform: transform, outputSize: plan.outputSize)
        let tolerance = plan.frameDelay / 2
        // A sample is usable only while it is presented no later than the trim end
        // (a hair of slack for timescale rounding), so no frame after the cut appears.
        let latest = plan.end + 0.001
        func presentation(_ sample: CMSampleBuffer) -> Double { CMTimeGetSeconds(CMSampleBufferGetPresentationTimeStamp(sample)) }
        var pending = output.copyNextSampleBuffer()
        var current: CVPixelBuffer?
        var written = 0
        for index in 0..<plan.frameCount {
            if flag.isCancelled { throw CancellationError() }
            let target = plan.start + Double(index) * plan.frameDelay
            // The frame shown at `target` is the last sample presented at or before it.
            while let sample = pending, presentation(sample) <= min(target + tolerance, latest) {
                if let buffer = CMSampleBufferGetImageBuffer(sample) { current = buffer }
                pending = output.copyNextSampleBuffer()
            }
            if current == nil, let sample = pending, presentation(sample) <= latest {
                // The clip starts before its first decoded frame: use that frame.
                current = CMSampleBufferGetImageBuffer(sample)
                pending = output.copyNextSampleBuffer()
            }
            guard let buffer = current else {
                if written == 0 { throw reader.status == .failed ? GIFTranscoderError.cannotRead(reader.error?.localizedDescription ?? "Unknown error.") : GIFTranscoderError.noFrames }
                // The source ended early (a variable-rate recording): hold the last frame.
                break
            }
            guard let frame = renderer.render(buffer) else { throw GIFTranscoderError.cannotWrite }
            CGImageDestinationAddImage(destination, frame, frameProperties(written))
            written += 1
            progress(Double(written) / Double(plan.frameCount))
        }
        if flag.isCancelled { throw CancellationError() }
        if reader.status == .failed { throw GIFTranscoderError.cannotRead(reader.error?.localizedDescription ?? "Unknown error.") }
        // Repeat the last frame so the GIF keeps the planned length and count.
        if written < plan.frameCount, let buffer = current, let frame = renderer.render(buffer) {
            while written < plan.frameCount {
                if flag.isCancelled { throw CancellationError() }
                CGImageDestinationAddImage(destination, frame, frameProperties(written))
                written += 1
                progress(Double(written) / Double(plan.frameCount))
            }
        }
        reader.cancelReading()
        guard CGImageDestinationFinalize(destination) else { throw GIFTranscoderError.cannotWrite }
        return written
    }
}

/// Draws one decoded BGRA frame into the output size, applying the track's
/// preferred transform so rotated or flipped sources come out upright.
struct GIFFrameRenderer {
    let naturalSize: CGSize
    let transform: CGAffineTransform
    let outputSize: CGSize

    func render(_ buffer: CVPixelBuffer) -> CGImage? {
        guard let source = Self.cgImage(from: buffer) else { return nil }
        return render(source)
    }

    func render(_ source: CGImage) -> CGImage? {
        let width = Int(outputSize.width), height = Int(outputSize.height)
        guard width > 0, height > 0,
              let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                                      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        context.interpolationQuality = .medium
        context.setFillColor(CGColor(gray: 0, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let displayRect = CGRect(origin: .zero, size: naturalSize).applying(transform).standardized
        guard displayRect.width > 0, displayRect.height > 0 else { return nil }
        // Work in the video's top-left, y-down space, scaled to the output.
        context.translateBy(x: 0, y: CGFloat(height))
        context.scaleBy(x: 1, y: -1)
        context.scaleBy(x: CGFloat(width) / displayRect.width, y: CGFloat(height) / displayRect.height)
        context.translateBy(x: -displayRect.minX, y: -displayRect.minY)
        context.concatenate(transform)
        // Core Graphics draws images bottom-up; flip once more inside the source rect.
        context.translateBy(x: 0, y: naturalSize.height)
        context.scaleBy(x: 1, y: -1)
        context.draw(source, in: CGRect(origin: .zero, size: naturalSize))
        return context.makeImage()
    }

    /// Wraps a BGRA pixel buffer's bytes as a CGImage (copied, so the buffer can be recycled).
    static func cgImage(from buffer: CVPixelBuffer) -> CGImage? {
        CVPixelBufferLockBaseAddress(buffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(buffer, .readOnly) }
        guard let base = CVPixelBufferGetBaseAddress(buffer) else { return nil }
        let width = CVPixelBufferGetWidth(buffer), height = CVPixelBufferGetHeight(buffer)
        let info = CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.premultipliedFirst.rawValue
        guard let context = CGContext(data: base, width: width, height: height, bitsPerComponent: 8,
                                      bytesPerRow: CVPixelBufferGetBytesPerRow(buffer),
                                      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: info)
        else { return nil }
        return context.makeImage()
    }
}

private final class CancellationFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false
    var isCancelled: Bool { lock.lock(); defer { lock.unlock() }; return cancelled }
    func cancel() { lock.lock(); cancelled = true; lock.unlock() }
}

private final class ReaderBox: @unchecked Sendable {
    private let lock = NSLock()
    private var reader: AVAssetReader?
    private var cancelled = false
    func set(_ reader: AVAssetReader) {
        lock.lock(); defer { lock.unlock() }
        self.reader = reader
        if cancelled { reader.cancelReading() }
    }
    func cancel() {
        lock.lock(); defer { lock.unlock() }
        cancelled = true
        reader?.cancelReading()
    }
    func clear() { lock.lock(); reader = nil; lock.unlock() }
}

/// The NETSCAPE2.0 / ANIMEXTS1.0 application extension that tells a viewer how
/// often to repeat. It is found by walking the GIF block structure (header,
/// screen descriptor, colour tables, extensions, image descriptors and their
/// sub-blocks), never by searching for bytes, so compressed image data is never
/// mistaken for it or touched when it is removed.
enum GIFLoopBlock {
    struct Location: Equatable {
        /// The whole extension block, from its 0x21 introducer through the terminator.
        let range: Range<Int>
        let loopCount: Int
    }

    private static let identifiers: [[UInt8]] = [Array("NETSCAPE2.0".utf8), Array("ANIMEXTS1.0".utf8)]

    static func locate(in data: Data) -> Location? {
        let bytes = [UInt8](data.prefix(13))
        guard bytes.count == 13, bytes[0] == 0x47, bytes[1] == 0x49, bytes[2] == 0x46 else { return nil }
        var offset = 13
        let packed = bytes[10]
        if packed & 0x80 != 0 { offset += 3 << (Int(packed & 0x07) + 1) }
        let count = data.count
        func byte(_ index: Int) -> UInt8? { index < count ? data[data.startIndex + index] : nil }
        /// Advances past a chain of data sub-blocks; nil when the file ends mid-block.
        func skipSubBlocks(from start: Int) -> Int? {
            var cursor = start
            while let size = byte(cursor) {
                cursor += 1
                if size == 0 { return cursor }
                cursor += Int(size)
            }
            return nil
        }
        while let introducer = byte(offset) {
            switch introducer {
            case 0x3B:
                return nil
            case 0x2C:
                guard let packed = byte(offset + 9) else { return nil }
                var cursor = offset + 10
                if packed & 0x80 != 0 { cursor += 3 << (Int(packed & 0x07) + 1) }
                cursor += 1 // LZW minimum code size
                guard let next = skipSubBlocks(from: cursor) else { return nil }
                offset = next
            case 0x21:
                guard let label = byte(offset + 1) else { return nil }
                let blockStart = offset
                var cursor = offset + 2
                if label == 0xFF, let size = byte(cursor), size == 11, cursor + 12 <= count {
                    let identifier = [UInt8](data[(data.startIndex + cursor + 1)..<(data.startIndex + cursor + 12)])
                    if identifiers.contains(identifier) {
                        var loopCount = 0
                        var sub = cursor + 12
                        if let subSize = byte(sub), subSize >= 3, let low = byte(sub + 2), let high = byte(sub + 3) {
                            loopCount = Int(low) | (Int(high) << 8)
                        }
                        guard let end = skipSubBlocks(from: sub) else { return nil }
                        sub = end
                        return Location(range: blockStart..<sub, loopCount: loopCount)
                    }
                }
                guard let next = skipSubBlocks(from: cursor) else { return nil }
                offset = next
            default:
                return nil
            }
        }
        return nil
    }

    /// Rewrites the file without its repeat block, byte for byte otherwise. Returns
    /// false when the file had none.
    @discardableResult
    static func strip(at url: URL) throws -> Bool {
        let data = try Data(contentsOf: url, options: .mappedIfSafe)
        guard let location = locate(in: data) else { return false }
        let replacement = url.deletingLastPathComponent().appendingPathComponent(".BelloBox-loop-\(UUID().uuidString).gif")
        try? FileManager.default.removeItem(at: replacement)
        guard FileManager.default.createFile(atPath: replacement.path, contents: nil) else { throw GIFTranscoderError.cannotWrite }
        do {
            let handle = try FileHandle(forWritingTo: replacement)
            defer { try? handle.close() }
            try handle.write(contentsOf: data[data.startIndex..<(data.startIndex + location.range.lowerBound)])
            try handle.write(contentsOf: data[(data.startIndex + location.range.upperBound)...])
        } catch {
            try? FileManager.default.removeItem(at: replacement)
            throw error
        }
        _ = try FileManager.default.replaceItemAt(url, withItemAt: replacement)
        return true
    }
}
