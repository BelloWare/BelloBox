import AppKit
import AVFoundation
import AVKit
import ImageIO
import SwiftUI
import XCTest
@testable import BelloBox

/// Movie → GIF conversion against synthetic movies: valid output with the planned
/// frames, timing and loop metadata, orientation, cancellation that leaves nothing
/// behind, and overwrite that never loses an existing file.
final class GIFTranscoderTests: XCTestCase {
    private var directory: URL!

    override func setUp() async throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent("BelloBoxGIF-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        try? FileManager.default.removeItem(at: directory)
    }

    private func movie(_ name: String = "source.mov", size: CGSize = CGSize(width: 320, height: 200), duration: TimeInterval = 2,
                       framesPerSecond: Int = 20, transform: CGAffineTransform = .identity) async throws -> URL {
        try await SyntheticMediaFixture.writeMovie(to: directory.appendingPathComponent(name), size: size, duration: duration,
                                                   framesPerSecond: framesPerSecond, transform: transform)
    }

    private func gifProperties(_ url: URL) throws -> (count: Int, loop: Int?, delays: [Double], size: CGSize) {
        let source = try XCTUnwrap(CGImageSourceCreateWithURL(url as CFURL, nil))
        XCTAssertTrue(UTType(CGImageSourceGetType(source)! as String)?.conforms(to: .gif) == true)
        let count = CGImageSourceGetCount(source)
        let file = CGImageSourceCopyProperties(source, nil) as? [CFString: Any]
        let loop = (file?[kCGImagePropertyGIFDictionary] as? [CFString: Any])?[kCGImagePropertyGIFLoopCount] as? Int
        var delays: [Double] = []
        for index in 0..<count {
            let properties = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any]
            let gif = properties?[kCGImagePropertyGIFDictionary] as? [CFString: Any]
            delays.append(gif?[kCGImagePropertyGIFUnclampedDelayTime] as? Double ?? gif?[kCGImagePropertyGIFDelayTime] as? Double ?? -1)
        }
        let first = try XCTUnwrap(CGImageSourceCreateImageAtIndex(source, 0, nil))
        return (count, loop, delays, CGSize(width: first.width, height: first.height))
    }

    private func stagedFiles() throws -> [String] {
        try FileManager.default.contentsOfDirectory(atPath: directory.path).filter { $0.hasPrefix(".BelloBox-export") }
    }

    func testConvertsASyntheticMovieIntoAValidLoopingGIFWithPlannedFramesAndTiming() async throws {
        let source = try await movie()
        let sourceSize = try XCTUnwrap(FileManager.default.attributesOfItem(atPath: source.path)[.size] as? Int)
        let destination = directory.appendingPathComponent("out.gif")
        var progress: [Double] = []
        let lock = NSLock()
        let result = try await GIFTranscoder.transcode(sourceURL: source, to: destination,
            options: GIFExportOptions(framesPerSecond: 10, maxWidth: 160, loops: true)) { value in
            lock.lock(); progress.append(value); lock.unlock()
        }
        XCTAssertEqual(result.url, destination)
        XCTAssertEqual(result.frameCount, 20)
        XCTAssertEqual(result.size, CGSize(width: 160, height: 100))
        XCTAssertEqual(result.duration, 2, accuracy: 0.001)
        XCTAssertGreaterThan(result.fileSize, 1_000)
        let properties = try gifProperties(destination)
        XCTAssertEqual(properties.count, 20)
        XCTAssertEqual(properties.loop, 0, "0 loops forever")
        XCTAssertEqual(properties.size, CGSize(width: 160, height: 100))
        XCTAssertEqual(properties.delays.count, 20)
        XCTAssertEqual(properties.delays[0], 0.1, accuracy: 0.011)
        XCTAssertEqual(progress.last, 1)
        XCTAssertEqual(progress, progress.sorted(), "Progress only moves forward")
        XCTAssertEqual(progress.count, 20)
        let gif = try XCTUnwrap(CGImageSourceCreateWithURL(destination as CFURL, nil))
        let first = try XCTUnwrap(CGImageSourceCreateImageAtIndex(gif, 0, nil))
        let last = try XCTUnwrap(CGImageSourceCreateImageAtIndex(gif, 19, nil))
        // The orange square moves left to right across the movie.
        XCTAssertGreaterThan(ScreenshotTestHelpers.pixel(first, x: 20, y: 40)[0], 180, "orange square starts on the left")
        XCTAssertLessThan(ScreenshotTestHelpers.pixel(last, x: 20, y: 40)[0], 120, "the square has moved away from the left")
        XCTAssertGreaterThan(ScreenshotTestHelpers.pixel(last, x: 140, y: 40)[0], 180, "the square ends on the right")
        XCTAssertEqual(try FileManager.default.attributesOfItem(atPath: source.path)[.size] as? Int, sourceSize, "The source is untouched")
        XCTAssertTrue(try stagedFiles().isEmpty, "No staged file is left behind")
    }

    func testTrimAndSingleLoopProduceTheExactClipAndOmitTheLoopExtension() async throws {
        let source = try await movie(duration: 3)
        let destination = directory.appendingPathComponent("trim.gif")
        let result = try await GIFTranscoder.transcode(sourceURL: source, to: destination,
            options: GIFExportOptions(framesPerSecond: 10, maxWidth: 320, loops: false, trimStart: 0.5, trimEnd: 1.0))
        XCTAssertEqual(result.frameCount, 5)
        XCTAssertEqual(result.size, CGSize(width: 320, height: 200), "Never upscaled beyond the source")
        let properties = try gifProperties(destination)
        XCTAssertEqual(properties.count, 5)
        XCTAssertNil(GIFLoopBlock.locate(in: try Data(contentsOf: destination)), "Loop off writes no repeat block, which every viewer plays once")
        XCTAssertNil(AnimatedGIFNSView.loopCount(at: destination))
        XCTAssertEqual(result.duration, 0.5, accuracy: 0.0001)
        let gif = try XCTUnwrap(CGImageSourceCreateWithURL(destination as CFURL, nil))
        let first = try XCTUnwrap(CGImageSourceCreateImageAtIndex(gif, 0, nil))
        // At 0.5 s of a 3 s movie the 56 px square (rows 72–128) sits around x 40–100, not at the left edge.
        XCTAssertLessThan(ScreenshotTestHelpers.pixel(first, x: 6, y: 90)[0], 120)
        XCTAssertGreaterThan(ScreenshotTestHelpers.pixel(first, x: 70, y: 90)[0], 180)
    }

    func testOneShotGIFsHaveNoRepeatBlockAndLoopingOnesRepeatForever() async throws {
        let source = try await movie(duration: 1, framesPerSecond: 10)
        let looping = directory.appendingPathComponent("loop.gif")
        _ = try await GIFTranscoder.transcode(sourceURL: source, to: looping, options: GIFExportOptions(framesPerSecond: 10, maxWidth: 64, loops: true))
        let loopingData = try Data(contentsOf: looping)
        let block = try XCTUnwrap(GIFLoopBlock.locate(in: loopingData))
        XCTAssertEqual(block.loopCount, 0, "0 repeats forever in every viewer")
        XCTAssertEqual(block.range.count, 19, "A NETSCAPE2.0 block is exactly 19 bytes")
        XCTAssertEqual(try gifProperties(looping).loop, 0)

        let once = directory.appendingPathComponent("once.gif")
        _ = try await GIFTranscoder.transcode(sourceURL: source, to: once, options: GIFExportOptions(framesPerSecond: 10, maxWidth: 64, loops: false))
        let onceData = try Data(contentsOf: once)
        XCTAssertNil(GIFLoopBlock.locate(in: onceData))
        XCTAssertEqual(try gifProperties(once).loop, 1, "ImageIO's reader reports 1 for an absent block, which is why the file structure is what counts")
        XCTAssertEqual(onceData.count, loopingData.count - 19, "Only the repeat block differs, byte for byte")
        XCTAssertEqual(try gifProperties(once).count, 10)
        let loopingFirst = try XCTUnwrap(CGImageSourceCreateImageAtIndex(CGImageSourceCreateWithURL(looping as CFURL, nil)!, 9, nil))
        let onceFirst = try XCTUnwrap(CGImageSourceCreateImageAtIndex(CGImageSourceCreateWithURL(once as CFURL, nil)!, 9, nil))
        XCTAssertEqual(ScreenshotTestHelpers.rgbaPixels(loopingFirst), ScreenshotTestHelpers.rgbaPixels(onceFirst), "Image data is untouched")

        // Structural parsing: bytes inside image data that spell NETSCAPE2.0 are not a block.
        var decoy = onceData
        let magic = Array("NETSCAPE2.0".utf8)
        let imageStart = try XCTUnwrap(onceData.firstIndex(of: 0x2C))
        decoy.replaceSubrange((imageStart + 20)..<(imageStart + 20 + magic.count), with: magic)
        XCTAssertNil(GIFLoopBlock.locate(in: decoy), "A byte pattern inside an image block is ignored")
        XCTAssertNil(GIFLoopBlock.locate(in: Data([0x47, 0x49, 0x46])), "Truncated files are rejected, not scanned")

        XCTAssertEqual(AnimatedGIFNSView.plays(forLoopCount: nil), 1, "No block: play once")
        XCTAssertNil(AnimatedGIFNSView.plays(forLoopCount: 0), "0: forever")
        XCTAssertEqual(AnimatedGIFNSView.plays(forLoopCount: 1), 2, "1 extra repeat: two plays (Chromium, libwebp)")
        XCTAssertEqual(AnimatedGIFNSView.plays(forLoopCount: 3), 4)
    }

    func testFrameDelaysKeepTheClipLengthToTheCentisecond() async throws {
        let fifteen = GIFExportPlan.delays(frameCount: 15, framesPerSecond: 15, duration: 1)
        XCTAssertEqual(fifteen.reduce(0, +), 100, "15 frames at 15 fps play for exactly 1.00 s, not 1.05 s")
        XCTAssertEqual(Array(fifteen.prefix(6)), [7, 6, 7, 7, 6, 7])
        XCTAssertTrue(fifteen.allSatisfy { $0 >= GIFExportPlan.minimumDelayCentiseconds })
        let partial = GIFExportPlan.delays(frameCount: 8, framesPerSecond: 15, duration: 0.507)
        XCTAssertEqual(partial.reduce(0, +), 51, "A 0.507 s clip encodes as 0.51 s: the partial last frame is kept")
        XCTAssertEqual(partial.last, 4)
        let tiny = GIFExportPlan.delays(frameCount: 2, framesPerSecond: 15, duration: 0.101)
        XCTAssertEqual(tiny.reduce(0, +), 10)
        XCTAssertEqual(tiny, [7, 3], "The final partial frame still gets its centiseconds")
        let sliver = GIFExportPlan.delays(frameCount: 3, framesPerSecond: 15, duration: 0.1334)
        XCTAssertEqual(sliver.reduce(0, +), 13)
        XCTAssertTrue(sliver.allSatisfy { $0 >= 2 }, "A sliver of a last frame borrows from the frame before it: \(sliver)")
        XCTAssertEqual(sliver, [7, 4, 2])

        for (duration, expected): (TimeInterval, Double) in [(1.0, 1.00), (2.3, 2.30)] {
            let source = try await movie("timing-\(Int(duration * 10)).mov", duration: 3, framesPerSecond: 20)
            let destination = directory.appendingPathComponent("timing-\(Int(duration * 10)).gif")
            let result = try await GIFTranscoder.transcode(sourceURL: source, to: destination,
                options: GIFExportOptions(framesPerSecond: 15, maxWidth: 64, trimStart: 0, trimEnd: duration))
            let decoded = try gifProperties(destination)
            XCTAssertEqual(decoded.delays.reduce(0, +), expected, accuracy: 0.0051, "decoded \(decoded.delays)")
            XCTAssertEqual(result.duration, expected, accuracy: 0.0051)
            XCTAssertEqual(decoded.count, Int((duration * 15 - 1e-6).rounded(.up)))
        }
        let fractional = try await movie("fraction.mov", duration: 2, framesPerSecond: 20)
        let result = try await GIFTranscoder.transcode(sourceURL: fractional, to: directory.appendingPathComponent("fraction.gif"),
            options: GIFExportOptions(framesPerSecond: 15, maxWidth: 64, trimStart: 0.5, trimEnd: 1.007))
        let decoded = try gifProperties(result.url)
        XCTAssertEqual(decoded.count, 8)
        XCTAssertEqual(decoded.delays.reduce(0, +), 0.51, accuracy: 0.0051, "0.507 s intended, 0.51 s encoded: centisecond rounding only")
        XCTAssertEqual(result.duration, 0.51, accuracy: 0.0051)
    }

    func testNoFrameAfterTheTrimEndOrBeforeTheTrimStartIsUsed() async throws {
        // Frames at or after 1.0 s are solid magenta; the clip ends exactly there.
        let source = try await SyntheticMediaFixture.writeMovie(to: directory.appendingPathComponent("sentinel.mov"),
            size: CGSize(width: 160, height: 100), duration: 2, framesPerSecond: 20, sentinelAfter: 1.0)
        let destination = directory.appendingPathComponent("sentinel.gif")
        let result = try await GIFTranscoder.transcode(sourceURL: source, to: destination,
            options: GIFExportOptions(framesPerSecond: 10, maxWidth: 160, loops: true, trimStart: 0.5, trimEnd: 1.0))
        XCTAssertEqual(result.frameCount, 5)
        let gif = try XCTUnwrap(CGImageSourceCreateWithURL(destination as CFURL, nil))
        for index in 0..<CGImageSourceGetCount(gif) {
            let frame = try XCTUnwrap(CGImageSourceCreateImageAtIndex(gif, index, nil))
            let pixel = ScreenshotTestHelpers.pixel(frame, x: 80, y: 20)
            XCTAssertFalse(pixel[0] > 200 && pixel[1] < 60 && pixel[2] > 200, "frame \(index) must not come from after the cut: \(pixel)")
        }
        let first = try XCTUnwrap(CGImageSourceCreateImageAtIndex(gif, 0, nil))
        // At 0.5 s of a 2 s clip the 28 px square (rows 36–64) has left the first 30 px.
        XCTAssertLessThan(ScreenshotTestHelpers.pixel(first, x: 4, y: 40)[0], 120, "the first frame is not the movie's first frame")
        // A clip ending inside the sentinel run must show it (it is the content there).
        let late = try await GIFTranscoder.transcode(sourceURL: source, to: directory.appendingPathComponent("late.gif"),
            options: GIFExportOptions(framesPerSecond: 10, maxWidth: 160, trimStart: 1.2, trimEnd: 1.5))
        let lateGIF = try XCTUnwrap(CGImageSourceCreateWithURL(late.url as CFURL, nil))
        let lateFrame = try XCTUnwrap(CGImageSourceCreateImageAtIndex(lateGIF, 0, nil))
        let latePixel = ScreenshotTestHelpers.pixel(lateFrame, x: 80, y: 20)
        XCTAssertTrue(latePixel[0] > 200 && latePixel[2] > 200, "content after 1.0 s is magenta: \(latePixel)")
    }

    func testTheDestinationMayNeverBeTheSourceOrAnAliasOfIt() async throws {
        let source = try await movie(duration: 0.5, framesPerSecond: 10)
        let original = try Data(contentsOf: source)
        for destination in [source, URL(fileURLWithPath: directory.path + "/./source.mov")] {
            do {
                _ = try await GIFTranscoder.transcode(sourceURL: source, to: destination, options: .default)
                XCTFail("Converting onto the source must be refused")
            } catch let error as GIFTranscoderError {
                XCTAssertEqual(error, .sameFile)
            }
        }
        let symlink = directory.appendingPathComponent("alias.gif")
        try FileManager.default.createSymbolicLink(at: symlink, withDestinationURL: source)
        do {
            _ = try await GIFTranscoder.transcode(sourceURL: source, to: symlink, options: .default)
            XCTFail("A symlink to the source must be refused")
        } catch let error as GIFTranscoderError {
            XCTAssertEqual(error, .sameFile)
        }
        let hardLink = directory.appendingPathComponent("hardlink.gif")
        try FileManager.default.linkItem(at: source, to: hardLink)
        XCTAssertTrue(GIFTranscoder.isSameFile(source, hardLink), "A hard link is the same file")
        do {
            _ = try await GIFTranscoder.transcode(sourceURL: source, to: hardLink, options: .default)
            XCTFail("A hard link to the source must be refused")
        } catch let error as GIFTranscoderError {
            XCTAssertEqual(error, .sameFile)
        }
        XCTAssertEqual(try Data(contentsOf: source), original, "The source is byte-identical")
        XCTAssertTrue(try stagedFiles().isEmpty)
        XCTAssertFalse(GIFTranscoder.isSameFile(source, directory.appendingPathComponent("other.gif")))
    }

    func testRotatedSourcesComeOutUpright() throws {
        // A 4×2 source: red on the left, blue on the right. A 90° clockwise transform
        // (the way a portrait phone clip is stored) puts red on top.
        let source = ScreenshotTestHelpers.image(width: 4, height: 2) { context in
            context.setFillColor(CGColor(red: 1, green: 0, blue: 0, alpha: 1))
            context.fill(CGRect(x: 0, y: 0, width: 2, height: 2))
            context.setFillColor(CGColor(red: 0, green: 0, blue: 1, alpha: 1))
            context.fill(CGRect(x: 2, y: 0, width: 2, height: 2))
        }
        let rotation = CGAffineTransform(a: 0, b: 1, c: -1, d: 0, tx: 2, ty: 0)
        let renderer = GIFFrameRenderer(naturalSize: CGSize(width: 4, height: 2), transform: rotation, outputSize: CGSize(width: 2, height: 4))
        let output = try XCTUnwrap(renderer.render(source))
        XCTAssertEqual(output.width, 2)
        XCTAssertEqual(output.height, 4)
        XCTAssertGreaterThan(ScreenshotTestHelpers.pixel(output, x: 0, y: 0)[0], 200, "red is on top")
        XCTAssertGreaterThan(ScreenshotTestHelpers.pixel(output, x: 1, y: 3)[2], 200, "blue is at the bottom")
        let identity = GIFFrameRenderer(naturalSize: CGSize(width: 4, height: 2), transform: .identity, outputSize: CGSize(width: 4, height: 2))
        let plain = try XCTUnwrap(identity.render(source))
        XCTAssertGreaterThan(ScreenshotTestHelpers.pixel(plain, x: 0, y: 0)[0], 200)
        XCTAssertGreaterThan(ScreenshotTestHelpers.pixel(plain, x: 3, y: 1)[2], 200)
    }

    func testRotatedMovieProducesAPortraitGIF() async throws {
        let rotation = CGAffineTransform(a: 0, b: 1, c: -1, d: 0, tx: 200, ty: 0)
        let source = try await movie("portrait.mov", size: CGSize(width: 320, height: 200), duration: 1, framesPerSecond: 10, transform: rotation)
        let info = try await GIFSourceInfo.load(url: source)
        XCTAssertEqual(info.displaySize, CGSize(width: 200, height: 320))
        let result = try await GIFTranscoder.transcode(sourceURL: source, to: directory.appendingPathComponent("portrait.gif"),
            options: GIFExportOptions(framesPerSecond: 10, maxWidth: 160))
        XCTAssertEqual(result.size, CGSize(width: 100, height: 160))
        XCTAssertEqual(try gifProperties(result.url).size, CGSize(width: 100, height: 160))
    }

    func testCancellationLeavesNoOutputAndNoStagedFileAndKeepsTheSource() async throws {
        let source = try await movie("long.mov", duration: 6, framesPerSecond: 20)
        let sourceSize = try XCTUnwrap(FileManager.default.attributesOfItem(atPath: source.path)[.size] as? Int)
        let destination = directory.appendingPathComponent("cancelled.gif")
        let started = expectation(description: "First frame written")
        started.assertForOverFulfill = false
        let task = Task {
            try await GIFTranscoder.transcode(sourceURL: source, to: destination,
                options: GIFExportOptions(framesPerSecond: 20, maxWidth: 320)) { _ in started.fulfill() }
        }
        await fulfillment(of: [started], timeout: 10)
        task.cancel()
        do {
            _ = try await task.value
            XCTFail("A cancelled conversion must not report success")
        } catch is CancellationError {
        } catch {
            XCTFail("Expected CancellationError, got \(error)")
        }
        XCTAssertFalse(FileManager.default.fileExists(atPath: destination.path))
        XCTAssertTrue(try stagedFiles().isEmpty)
        XCTAssertEqual(try FileManager.default.attributesOfItem(atPath: source.path)[.size] as? Int, sourceSize)
    }

    func testOverwriteIsAtomicAndAFailedConversionKeepsTheExistingFile() async throws {
        let source = try await movie(duration: 1, framesPerSecond: 10)
        let destination = directory.appendingPathComponent("existing.gif")
        try Data("not a gif".utf8).write(to: destination)
        _ = try await GIFTranscoder.transcode(sourceURL: source, to: destination, options: GIFExportOptions(framesPerSecond: 10, maxWidth: 64))
        XCTAssertEqual(try gifProperties(destination).count, 10, "The old file was replaced by the new GIF")
        let good = try Data(contentsOf: destination)

        let bogus = directory.appendingPathComponent("bogus.mov")
        try Data("still not a movie".utf8).write(to: bogus)
        do {
            _ = try await GIFTranscoder.transcode(sourceURL: bogus, to: destination, options: .default)
            XCTFail("A non-movie must fail")
        } catch {
            XCTAssertFalse(error is CancellationError)
        }
        XCTAssertEqual(try Data(contentsOf: destination), good, "A failed conversion never disturbs the file already there")
        XCTAssertTrue(try stagedFiles().isEmpty)
    }

    func testPlanBoundsClipLengthFrameCountAndDimensionsAndNeverUpscales() throws {
        let long = GIFSourceInfo(duration: 500, displaySize: CGSize(width: 3840, height: 2160), nominalFrameRate: 60)
        let plan = try GIFExportPlan.make(source: long, options: GIFExportOptions(framesPerSecond: 20, maxWidth: 1080))
        XCTAssertEqual(plan.duration, GIFExportOptions.maxDuration)
        XCTAssertEqual(plan.frameCount, 2_400)
        XCTAssertEqual(plan.encodedDuration, 120, accuracy: 0.0001, "Encoded and intended lengths agree")
        XCTAssertEqual(plan.outputSize, CGSize(width: 1080, height: 608))
        XCTAssertEqual(plan.frameDelay, 0.05)
        XCTAssertTrue(plan.summary.hasPrefix("2400 frames · 1080 × 608 px · 120 s at 20 fps"))

        let capped = try GIFExportPlan.make(source: long, options: GIFExportOptions(framesPerSecond: 30, maxWidth: 4000))
        XCTAssertEqual(capped.framesPerSecond, 20, "Frame rates outside the offered range are clamped to it")
        XCTAssertLessThanOrEqual(capped.frameCount, GIFExportOptions.maxFrames)
        XCTAssertEqual(capped.encodedDuration, capped.duration, accuracy: 0.0051)
        XCTAssertEqual(capped.outputSize.width, 1080, "The width cap is clamped")

        let small = GIFSourceInfo(duration: 1.5, displaySize: CGSize(width: 100, height: 50), nominalFrameRate: 30)
        let tiny = try GIFExportPlan.make(source: small, options: GIFExportOptions(framesPerSecond: 15, maxWidth: 640))
        XCTAssertEqual(tiny.outputSize, CGSize(width: 100, height: 50), "Sources are never upscaled")
        XCTAssertEqual(tiny.frameCount, 23)

        let portrait = GIFSourceInfo(duration: 2, displaySize: CGSize(width: 400, height: 1600), nominalFrameRate: 30)
        XCTAssertEqual(try GIFExportPlan.make(source: portrait, options: GIFExportOptions(maxWidth: 800)).outputSize, CGSize(width: 200, height: 800),
                       "The cap applies to the longer edge")

        let backwards = try GIFExportPlan.make(source: small, options: GIFExportOptions(trimStart: 1.2, trimEnd: 0.4))
        XCTAssertEqual(backwards.start, 1.2)
        XCTAssertEqual(backwards.end, 1.5, "An end before the start falls back to the clip end")
        let nearEnd = try GIFExportPlan.make(source: small, options: GIFExportOptions(trimStart: 1.49))
        XCTAssertEqual(nearEnd.start, 1.45, accuracy: 0.001, "A start at the very end is pulled back to leave a minimal clip")
        XCTAssertThrowsError(try GIFExportPlan.make(source: GIFSourceInfo(duration: 0.04, displaySize: CGSize(width: 10, height: 10), nominalFrameRate: 30), options: .default))
        XCTAssertThrowsError(try GIFExportPlan.make(source: GIFSourceInfo(duration: 0, displaySize: CGSize(width: 10, height: 10), nominalFrameRate: 0), options: .default))
    }

    func testOptionsAndRecordingOptionsDecodeWithoutTheNewKeys() throws {
        let decoded = try JSONDecoder().decode(GIFExportOptions.self, from: Data("{}".utf8))
        XCTAssertEqual(decoded, .default)
        let legacy = """
        {"audioSource":"none","includeCursor":true,"clickOverlayMode":"off","keystrokeMode":"off","secureFieldRedactionMode":"strict",
         "quality":"balanced","countdownSeconds":3,"excludeBelloBoxWindows":true,"excludesCurrentProcessAudio":true}
        """
        let options = try JSONDecoder().decode(RecordingOptions.self, from: Data(legacy.utf8))
        XCTAssertEqual(options.outputFormat, .movie)
        XCTAssertEqual(options.gif, .default)
        var gifOptions = RecordingOptions.default
        gifOptions.outputFormat = .gif
        gifOptions.gif = GIFExportOptions(framesPerSecond: 20, maxWidth: 800, loops: false)
        let roundTrip = try JSONDecoder().decode(RecordingOptions.self, from: JSONEncoder().encode(gifOptions))
        XCTAssertEqual(roundTrip, gifOptions)
        XCTAssertEqual(GIFExportOptions(framesPerSecond: 99, maxWidth: 5, trimStart: -3).normalized, GIFExportOptions(framesPerSecond: 20, maxWidth: 64, trimStart: 0))
        XCTAssertEqual(GIFExportOptions.frameRateRange.upperBound, GIFExportOptions.frameRateChoices.max())
    }

    func testTrimHandlesHoldEachOtherWithoutLeapfrogging() {
        let duration: TimeInterval = 600
        let start = GIFTrimControls.movingStart(to: 590, options: GIFExportOptions(trimStart: 0, trimEnd: 120), duration: duration)
        XCTAssertEqual(start.trimStart, 590)
        XCTAssertEqual(start.trimEnd, 600, "The end follows the start in one move, keeping the clip as long as it can")
        let end = GIFTrimControls.movingEnd(to: 50, options: GIFExportOptions(trimStart: 100, trimEnd: 220), duration: duration)
        XCTAssertEqual(end.trimEnd, 50)
        XCTAssertEqual(end.trimStart, 49.9, accuracy: 0.0001, "The start follows the end")
        let far = GIFTrimControls.movingEnd(to: 600, options: GIFExportOptions(trimStart: 0, trimEnd: 120), duration: duration)
        XCTAssertEqual(far.trimStart, 480, "The limit shortens from the other end")
        XCTAssertEqual(far.trimEnd, 600)
        let short = GIFTrimControls.movingStart(to: 10, options: GIFExportOptions(trimStart: 0, trimEnd: 30), duration: duration)
        XCTAssertEqual(short.trimStart, 10)
        XCTAssertEqual(short.trimEnd, 30, "Moving the start inside the clip leaves the end alone")
        let past = GIFTrimControls.movingStart(to: 999, options: GIFExportOptions(trimStart: 0, trimEnd: 30), duration: duration)
        XCTAssertEqual(past.trimStart, 599.9, accuracy: 0.0001)
        XCTAssertEqual(past.trimEnd, 600)
    }

    func testStagedURLsFollowTheDestinationExtension() {
        let movie = URL(fileURLWithPath: "/tmp/BelloBoxTests/clip.mov")
        XCTAssertEqual(RecordingFileStore.stagedURL(for: movie).pathExtension, "mov")
        XCTAssertEqual(RecordingFileStore.stagedURL(for: movie, pathExtension: "gif").pathExtension, "gif")
        XCTAssertEqual(RecordingFileStore.stagedURL(for: URL(fileURLWithPath: "/tmp/BelloBoxTests/clip.gif")).pathExtension, "gif")
        XCTAssertEqual(RecordingFileStore.stagedURL(for: movie).deletingLastPathComponent().path, "/tmp/BelloBoxTests")
        XCTAssertTrue(RecordingFileStore.stagedURL(for: movie).lastPathComponent.hasPrefix(".BelloBox-export-"))
    }
}

/// The recording flow around the converter: GIF mode converts after the movie is
/// safe, progress is real, cancelling or failing keeps the movie for review.
@MainActor
final class RecordingGIFFlowTests: XCTestCase {
    private func temporaryDefaults() -> UserDefaults {
        let suiteName = "BelloBoxTests.RecordingGIFFlow.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }

    private func gifOptions() -> RecordingOptions {
        var options = RecordingOptions.default
        options.countdownSeconds = 0
        options.outputFormat = .gif
        options.gif = GIFExportOptions(framesPerSecond: 10, maxWidth: 320)
        return options
    }

    func testGIFModeConvertsAfterStopWithTruthfulProgressAndKeepsTheMovie() async throws {
        let engine = StubEngine()
        let movie = URL(fileURLWithPath: "/tmp/BelloBoxTests-\(UUID().uuidString).mov")
        var conversions: [(URL, URL, GIFExportOptions)] = []
        let coordinator = RecordingCoordinator(settings: AppSettings(defaults: temporaryDefaults()),
            makeEngine: { _, _ in engine }, permissionProvider: { _ in .granted },
            transcodeGIF: { source, destination, options, progress in
                conversions.append((source, destination, options))
                progress(0.25)
                progress(0.75)
                try await Task.sleep(nanoseconds: 20_000_000)
                return GIFExportResult(url: destination, frameCount: 10, size: CGSize(width: 320, height: 200), duration: 1, fileSize: 1_234)
            })
        var states: [RecordingState] = []
        coordinator.onStateChange = { states.append($0) }
        await coordinator.start(target: .display(displayID: 1), options: gifOptions())
        guard case let .recording(runtime) = coordinator.state else { return XCTFail("Expected recording") }
        XCTAssertEqual(runtime.outputFormat, .gif, "The HUD can show the GIF badge")
        coordinator.stop()
        XCTAssertTrue(coordinator.isRecording, "Finishing and converting count as busy")
        engine.completeStop(with: movie)
        for _ in 0..<200 {
            if case .reviewing = coordinator.state { break }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        guard case let .reviewing(url, warning, gif) = coordinator.state else { return XCTFail("Expected review, got \(coordinator.state)") }
        XCTAssertEqual(url, movie, "The movie is what the review holds on to")
        XCTAssertNil(warning)
        XCTAssertEqual(gif, movie.deletingPathExtension().appendingPathExtension("gif"))
        XCTAssertEqual(conversions.count, 1)
        XCTAssertEqual(conversions.first?.0, movie)
        XCTAssertEqual(conversions.first?.2, GIFExportOptions(framesPerSecond: 10, maxWidth: 320))
        XCTAssertTrue(states.contains(.finishing))
        XCTAssertTrue(states.contains(.convertingToGIF(progress: 0)))
        XCTAssertTrue(states.contains(.convertingToGIF(progress: 0.25)))
        XCTAssertTrue(states.contains(.convertingToGIF(progress: 0.75)))
        XCTAssertFalse(coordinator.isRecording)
    }

    func testCancellingTheConversionReviewsTheMovieWithANote() async throws {
        let engine = StubEngine()
        let movie = URL(fileURLWithPath: "/tmp/BelloBoxTests-\(UUID().uuidString).mov")
        let conversionStarted = expectation(description: "conversion started")
        let coordinator = RecordingCoordinator(settings: AppSettings(defaults: temporaryDefaults()),
            makeEngine: { _, _ in engine }, permissionProvider: { _ in .granted },
            transcodeGIF: { _, destination, _, _ in
                conversionStarted.fulfill()
                try await Task.sleep(nanoseconds: 10_000_000_000)
                return GIFExportResult(url: destination, frameCount: 1, size: .zero, duration: 1, fileSize: 1)
            })
        await coordinator.start(target: .display(displayID: 1), options: gifOptions())
        coordinator.stop()
        engine.completeStop(with: movie)
        await fulfillment(of: [conversionStarted], timeout: 2)
        guard case .convertingToGIF = coordinator.state else { return XCTFail("Expected converting") }
        coordinator.stop()
        XCTAssertTrue({ if case .convertingToGIF = coordinator.state { return true }; return false }(), "Stop during conversion is ignored")
        coordinator.cancelGIFConversion()
        for _ in 0..<200 {
            if case .reviewing = coordinator.state { break }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        guard case let .reviewing(url, warning, gif) = coordinator.state else { return XCTFail("Expected review, got \(coordinator.state)") }
        XCTAssertEqual(url, movie)
        XCTAssertNil(gif)
        XCTAssertTrue(warning?.contains("cancelled") == true)
        XCTAssertTrue(warning?.contains("kept") == true)
    }

    func testAFailedConversionKeepsTheMovieAndExplains() async throws {
        let engine = StubEngine()
        let movie = URL(fileURLWithPath: "/tmp/BelloBoxTests-\(UUID().uuidString).mov")
        let coordinator = RecordingCoordinator(settings: AppSettings(defaults: temporaryDefaults()),
            makeEngine: { _, _ in engine }, permissionProvider: { _ in .granted },
            transcodeGIF: { _, _, _, _ in throw GIFTranscoderError.cannotWrite })
        await coordinator.start(target: .display(displayID: 1), options: gifOptions())
        coordinator.stop()
        engine.completeStop(with: movie)
        for _ in 0..<200 {
            if case .reviewing = coordinator.state { break }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        guard case let .reviewing(url, warning, gif) = coordinator.state else { return XCTFail("Expected review, got \(coordinator.state)") }
        XCTAssertEqual(url, movie)
        XCTAssertNil(gif)
        XCTAssertTrue(warning?.contains("could not be written") == true)
    }

    func testMovieModeNeverConverts() async throws {
        let engine = StubEngine()
        let movie = URL(fileURLWithPath: "/tmp/BelloBoxTests-\(UUID().uuidString).mov")
        var converted = false
        let coordinator = RecordingCoordinator(settings: AppSettings(defaults: temporaryDefaults()),
            makeEngine: { _, _ in engine }, permissionProvider: { _ in .granted },
            transcodeGIF: { _, destination, _, _ in converted = true; return GIFExportResult(url: destination, frameCount: 1, size: .zero, duration: 1, fileSize: 1) })
        var options = RecordingOptions.default
        options.countdownSeconds = 0
        await coordinator.start(target: .display(displayID: 1), options: options)
        coordinator.stop()
        engine.completeStop(with: movie)
        for _ in 0..<100 {
            if case .reviewing = coordinator.state { break }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        XCTAssertEqual(coordinator.state, .reviewing(movie))
        XCTAssertFalse(converted)
    }

    func testReviewExportsAGIFOnDemandAndCancellationKeepsTheMovie() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("BelloBoxReviewGIF-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let movie = try await SyntheticMediaFixture.writeMovie(to: directory.appendingPathComponent("review.mov"),
            size: CGSize(width: 160, height: 100), duration: 1, framesPerSecond: 10)
        let viewModel = RecordingReviewViewModel(fileURL: movie, gifOptions: GIFExportOptions(framesPerSecond: 10, maxWidth: 80))
        XCTAssertFalse(viewModel.hasGIF)
        XCTAssertFalse(viewModel.showsGIFPreview)
        viewModel.requestGIFExport()
        XCTAssertTrue(viewModel.showsGIFExport)
        for _ in 0..<200 where viewModel.sourceInfo == nil { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertEqual(viewModel.sourceInfo?.displaySize, CGSize(width: 160, height: 100))
        XCTAssertEqual(viewModel.mediaSummary, "1.0 s · 160 × 100")
        XCTAssertEqual(viewModel.gifPlan?.frameCount, 10)
        XCTAssertEqual(viewModel.gifOptions.trimEnd, 1)

        let destination = directory.appendingPathComponent("review.gif")
        await viewModel.exportGIF(to: destination)
        XCTAssertFalse(viewModel.isConverting)
        XCTAssertFalse(viewModel.showsGIFExport)
        XCTAssertNil(viewModel.errorMessage)
        XCTAssertEqual(viewModel.gifURL, destination)
        XCTAssertTrue(viewModel.hasGIF)
        XCTAssertTrue(viewModel.showsGIFPreview)
        XCTAssertTrue(viewModel.statusMessage?.hasPrefix("GIF saved to review.gif") == true)
        XCTAssertTrue(FileManager.default.fileExists(atPath: movie.path), "The movie stays")
        XCTAssertEqual(viewModel.conversionProgress, 1)

        // Cancellation through the injected transcoder.
        let slow = RecordingReviewViewModel(fileURL: movie, transcodeGIF: { _, destination, _, _ in
            try await Task.sleep(nanoseconds: 10_000_000_000)
            return GIFExportResult(url: destination, frameCount: 1, size: .zero, duration: 1, fileSize: 1)
        })
        let export = Task { await slow.exportGIF(to: directory.appendingPathComponent("slow.gif")) }
        for _ in 0..<100 where !slow.isConverting { try await Task.sleep(nanoseconds: 5_000_000) }
        slow.cancelGIFExport()
        await export.value
        XCTAssertFalse(slow.isConverting)
        XCTAssertNil(slow.gifURL)
        XCTAssertTrue(slow.statusMessage?.contains("cancelled") == true)
        XCTAssertTrue(FileManager.default.fileExists(atPath: movie.path))
    }

    func testStandaloneConverterLoadsInfoConvertsAndReportsTheResult() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("BelloBoxStandaloneGIF-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let movie = try await SyntheticMediaFixture.writeMovie(to: directory.appendingPathComponent("input.mov"),
            size: CGSize(width: 200, height: 120), duration: 0.5, framesPerSecond: 10)
        let viewModel = VideoToGIFViewModel(options: GIFExportOptions(framesPerSecond: 10, maxWidth: 100))
        XCTAssertNil(viewModel.plan)
        viewModel.load(movie)
        for _ in 0..<200 where viewModel.sourceInfo == nil { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertEqual(viewModel.sourceInfo?.displaySize, CGSize(width: 200, height: 120))
        XCTAssertEqual(viewModel.plan?.frameCount, 5)
        XCTAssertEqual(viewModel.plan?.outputSize, CGSize(width: 100, height: 60))
        await viewModel.convert(to: directory.appendingPathComponent("output.gif"))
        XCTAssertNil(viewModel.errorMessage)
        XCTAssertEqual(viewModel.result?.frameCount, 5)
        XCTAssertTrue(viewModel.statusMessage?.hasPrefix("Saved output.gif") == true)
        XCTAssertTrue(FileManager.default.fileExists(atPath: directory.appendingPathComponent("output.gif").path))

        let broken = directory.appendingPathComponent("broken.mov")
        try Data("nope".utf8).write(to: broken)
        viewModel.load(broken)
        for _ in 0..<200 where viewModel.errorMessage == nil { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertNotNil(viewModel.errorMessage)
        XCTAssertNil(viewModel.result, "Loading another file clears the previous result")
    }

    func testClosingTheReviewOrConverterCancelsWorkAndIgnoresLateResults() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("BelloBoxReviewClose-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let movie = try await SyntheticMediaFixture.writeMovie(to: directory.appendingPathComponent("close.mov"),
            size: CGSize(width: 80, height: 50), duration: 0.5, framesPerSecond: 10)
        let started = expectation(description: "export started")
        started.assertForOverFulfill = false
        let review = RecordingReviewViewModel(fileURL: movie, transcodeGIF: { _, destination, _, progress in
            started.fulfill()
            progress(0.2)
            try await Task.sleep(nanoseconds: 10_000_000_000)
            return GIFExportResult(url: destination, frameCount: 1, size: .zero, duration: 1, fileSize: 1)
        })
        var closed = 0
        review.onClose = { closed += 1 }
        let export = Task { await review.exportGIF(to: directory.appendingPathComponent("close.gif")) }
        await fulfillment(of: [started], timeout: 2)
        XCTAssertTrue(review.isConverting)
        review.close()
        XCTAssertEqual(closed, 1)
        XCTAssertFalse(review.isConverting)
        await export.value
        XCTAssertNil(review.statusMessage, "A cancelled export reports nothing after the window closed")
        XCTAssertNil(review.gifURL)
        XCTAssertEqual(review.conversionProgress, 0.2, "No progress arrives after closing")

        let converter = VideoToGIFViewModel()
        converter.load(movie)
        XCTAssertNotNil(converter.player)
        converter.close()
        try await Task.sleep(nanoseconds: 300_000_000)
        XCTAssertNil(converter.sourceInfo, "Metadata that arrives after closing is dropped")
        XCTAssertEqual(converter.player?.rate, 0)
    }

    func testAnimatedGIFPreviewPlaysOnceForOneShotFilesLoopsForeverForZeroAndReloadsARewrittenPath() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("BelloBoxGIFPreview-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        func write(_ name: String, color: CGColor, loops: Bool) throws -> URL {
            let url = directory.appendingPathComponent(name)
            let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, UTType.gif.identifier as CFString, 3, nil))
            if loops {
                CGImageDestinationSetProperties(destination, [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFLoopCount: 0]] as CFDictionary)
            }
            for _ in 0..<3 {
                let frame = ScreenshotTestHelpers.image(width: 8, height: 8) { context in
                    context.setFillColor(color); context.fill(CGRect(x: 0, y: 0, width: 8, height: 8))
                }
                CGImageDestinationAddImage(destination, frame, [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFDelayTime: 0.02]] as CFDictionary)
            }
            XCTAssertTrue(CGImageDestinationFinalize(destination))
            if !loops { XCTAssertFalse(try GIFLoopBlock.strip(at: url), "Omitting the property writes no block, so there is nothing to strip") }
            XCTAssertEqual(GIFLoopBlock.locate(in: try Data(contentsOf: url))?.loopCount, loops ? 0 : nil)
            return url
        }
        let once = try write("once.gif", color: CGColor(red: 1, green: 0, blue: 0, alpha: 1), loops: false)
        let view = AnimatedGIFNSView(frame: CGRect(x: 0, y: 0, width: 40, height: 40))
        view.reduceMotionOverride = true
        view.load(once, revision: 0)
        XCTAssertFalse(view.isPlaying, "Reduce Motion starts paused")
        XCTAssertEqual(view.playsRemaining, 1)
        XCTAssertNil(view.loopCount)
        view.advance(); view.advance()
        XCTAssertEqual(view.frameIndex, 2)
        view.advance()
        XCTAssertEqual(view.frameIndex, 2, "A one-shot GIF stops on its last frame")
        XCTAssertEqual(view.playsRemaining, 0)
        XCTAssertFalse(view.isPlaying)
        view.play()
        XCTAssertEqual(view.frameIndex, 0, "Play Again starts over, once")
        view.stop()

        let forever = try write("forever.gif", color: CGColor(red: 0, green: 0, blue: 1, alpha: 1), loops: true)
        view.load(forever, revision: 0)
        XCTAssertNil(view.playsRemaining)
        XCTAssertEqual(view.loopCount, 0)
        view.stop()
        for _ in 0..<7 { view.advance() }
        XCTAssertEqual(view.frameIndex, 1, "A looping GIF wraps around")
        XCTAssertGreaterThan(view.currentCentrePixel()?[2] ?? 0, 240)

        // Rewriting the same path with new content must be reloaded through the revision.
        _ = try write("forever.gif", color: CGColor(red: 0, green: 1, blue: 0, alpha: 1), loops: true)
        view.load(forever, revision: 1)
        XCTAssertGreaterThan(view.currentCentrePixel()?[1] ?? 0, 240, "The rewritten file is shown")
        XCTAssertEqual(view.revision, 1)
        view.reduceMotionOverride = false
        view.load(forever, revision: 2)
        XCTAssertTrue(view.isPlaying, "Without Reduce Motion the preview plays on load")
        view.stop()
    }

    func testConverterResultRowsFitWithoutClippingAnyLabel() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("BelloBoxConverterLayout-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let movie = directory.appendingPathComponent("a-rather-long-sample-portrait-recording-name.mov")
        try Data("x".utf8).write(to: movie)
        let viewModel = VideoToGIFViewModel(transcodeGIF: { _, destination, _, _ in
            GIFExportResult(url: destination, frameCount: 2_400, size: CGSize(width: 1080, height: 1920), duration: 120, fileSize: 123_456_789)
        })
        viewModel.load(movie)
        // Everything the rows can show at once, with the longest plausible values.
        let width = VideoToGIFView.preferredSize.width - 36
        func cardWidth() -> CGFloat {
            NSHostingView(rootView: VideoToGIFSourceCard(viewModel: viewModel)).fittingSize.width
        }
        XCTAssertLessThanOrEqual(cardWidth(), width, "The source row fits before conversion")
        let done = expectation(description: "converted")
        Task { await viewModel.convert(to: directory.appendingPathComponent("out.gif")); done.fulfill() }
        wait(for: [done], timeout: 5)
        XCTAssertNotNil(viewModel.result)
        XCTAssertEqual(VideoToGIFSourceCard.resultSummary(viewModel.result!), "2400 frames · 1080 × 1920 · 123.5 MB")
        XCTAssertLessThanOrEqual(cardWidth(), width, "With the result row every fixed-size label still fits; only the file name may shorten")
        let content = NSHostingView(rootView: VideoToGIFContent(viewModel: viewModel, onMinimize: {}))
        XCTAssertLessThanOrEqual(content.fittingSize.width, VideoToGIFView.preferredSize.width)
        XCTAssertLessThanOrEqual(content.fittingSize.height - VideoToGIFView.previewHeightRange.upperBound,
                                 VideoToGIFView.preferredSize.height - VideoToGIFView.previewHeightRange.lowerBound,
                                 "The result row fits above the footer once the preview has given up its slack")
    }

    /// The window is a fixed 560 × 600, so a taller state cannot push the header
    /// and footer out of their padding; the preview is the one child that yields.
    /// Checked on the real hosted layout, state by state, up to the worst case: a
    /// long movie (its note under the sliders), a result row and a two-line error.
    func testConverterPreviewGivesUpHeightSoEveryStateKeepsItsPadding() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("BelloBoxConverterHeight-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let short = try await SyntheticMediaFixture.writeMovie(to: directory.appendingPathComponent("short.mov"),
                                                                size: CGSize(width: 320, height: 200), duration: 4, framesPerSecond: 10)
        let long = try await SyntheticMediaFixture.writeMovie(to: directory.appendingPathComponent("long.mov"),
                                                               size: CGSize(width: 64, height: 48), duration: GIFExportOptions.maxDuration + 1, framesPerSecond: 1)
        let window = VideoToGIFView.preferredSize
        let previewHeights = VideoToGIFView.previewHeightRange
        let headerHeight = NSHostingView(rootView: PopupHeader(icon: "film.stack", title: "Video to GIF", subtitle: "x", onMinimize: {}, onClose: {})
            .frame(width: window.width - 36)).fittingSize.height
        let twoLines = "Could not write the GIF: the destination folder is on a volume that was ejected while the frames were being written. Choose another place and try again."

        let blocking = expectation(description: "conversion started")
        blocking.assertForOverFulfill = false
        let viewModel = VideoToGIFViewModel(transcodeGIF: { _, destination, _, _ in
            if destination.lastPathComponent == "blocked.gif" {
                blocking.fulfill()
                try await Task.sleep(nanoseconds: 10_000_000_000)  // until cancelled
            }
            return GIFExportResult(url: destination, frameCount: 60, size: CGSize(width: 640, height: 400), duration: 4, fileSize: 106_496)
        })

        /// Lays the window out for real and returns the preview's height. The
        /// natural content height (preview at its maximum) says how much the
        /// preview has to give; the hosted layout must show exactly that.
        @discardableResult
        func check(_ state: String, file: StaticString = #filePath, line: UInt = #line) throws -> CGFloat {
            let natural = NSHostingView(rootView: VideoToGIFContent(viewModel: viewModel, onMinimize: {})).fittingSize.height
            let overflow = max(0, natural - window.height)
            XCTAssertLessThanOrEqual(overflow, previewHeights.upperBound - previewHeights.lowerBound,
                                     "\(state): needs \(natural) pt, more than the preview can give", file: file, line: line)
            let host = NSHostingView(rootView: VideoToGIFView(viewModel: viewModel, onMinimize: {}))
            host.frame = CGRect(origin: .zero, size: window)
            host.layoutSubtreeIfNeeded()
            let previewView = try XCTUnwrap(Self.previewView(in: host), "\(state): no preview view was hosted", file: file, line: line)
            let frame = host.convert(previewView.bounds, from: previewView)
            let topInset = host.isFlipped ? frame.minY : host.bounds.height - frame.maxY
            XCTAssertEqual(frame.height, previewHeights.upperBound - overflow, accuracy: 1,
                           "\(state): the preview gives up exactly the overflow (natural \(natural) pt)", file: file, line: line)
            XCTAssertGreaterThanOrEqual(frame.height, previewHeights.lowerBound - 0.5, "\(state)", file: file, line: line)
            XCTAssertEqual(topInset, 18 + headerHeight + 12, accuracy: 1, "\(state): the header keeps its padding", file: file, line: line)
            XCTAssertEqual(frame.minX, 18, accuracy: 1, "\(state)", file: file, line: line)
            XCTAssertEqual(frame.width, window.width - 36, accuracy: 1, "\(state)", file: file, line: line)
            return frame.height
        }
        func awaitSourceInfo() async {
            for _ in 0..<250 where viewModel.sourceInfo == nil { try? await Task.sleep(nanoseconds: 20_000_000) }
            XCTAssertNotNil(viewModel.sourceInfo, "The movie's metadata should load")
        }

        viewModel.load(short)
        try check("reading the movie")
        await awaitSourceInfo()
        let beforeConversion = try check("short movie, before conversion")
        XCTAssertEqual(beforeConversion, previewHeights.upperBound, "Before conversion the preview takes the free space instead of leaving a blank band")

        let blockedExport = Task { await viewModel.convert(to: directory.appendingPathComponent("blocked.gif")) }
        await fulfillment(of: [blocking], timeout: 5)
        XCTAssertTrue(viewModel.isConverting)
        try check("converting")
        viewModel.cancel()
        await blockedExport.value
        XCTAssertEqual(viewModel.statusMessage, "Conversion cancelled. Nothing was written.")
        try check("cancelled, with a status line")

        await viewModel.convert(to: directory.appendingPathComponent("short.gif"))
        XCTAssertNotNil(viewModel.result)
        let withResult = try check("result row and one-line status")
        XCTAssertLessThan(withResult, previewHeights.upperBound, "The result row and status are paid for by the preview, not the padding")
        viewModel.errorMessage = twoLines
        let withError = try check("result row and two-line error")
        XCTAssertLessThan(withError, withResult)
        viewModel.showsResult = false
        try check("result row, movie preview, two-line error")

        viewModel.load(long)
        await awaitSourceInfo()
        XCTAssertGreaterThan(viewModel.sourceInfo?.duration ?? 0, GIFExportOptions.maxDuration, "The fixture is long enough to show the clip-limit note")
        try check("long movie with its clip-limit note")
        await viewModel.convert(to: directory.appendingPathComponent("long.gif"))
        XCTAssertNotNil(viewModel.result)
        viewModel.errorMessage = twoLines
        let worstCase = try check("long movie, result row and two-line error")
        XCTAssertGreaterThanOrEqual(worstCase, previewHeights.lowerBound, "Even the tallest state keeps the preview usable")
        viewModel.close()
    }

    private static func previewView(in view: NSView) -> NSView? {
        if view is AVPlayerView || view is AnimatedGIFNSView { return view }
        for subview in view.subviews {
            if let found = previewView(in: subview) { return found }
        }
        return nil
    }

    /// "Longest edge" is the recording card's label too, and the whole GIF row
    /// (both pickers, Loop and the note) still sits on one line at the card's width.
    func testRecordingOptionsGIFRowUsesTheConverterControlsAndFitsOnOneLine() {
        let row = NSHostingView(rootView: HStack(spacing: 14) {
            GIFFormatControls(options: .constant(.default))
            Text(RecordingOptionsBar.gifGuidance).font(.caption2)
        }.fixedSize())
        // The card's 14 pt and the section's 10 pt of horizontal padding on each side.
        XCTAssertLessThanOrEqual(row.fittingSize.width, RecordingOptionsBar.preferredSize.width - 48,
                                 "The GIF row would wrap or clip at \(RecordingOptionsBar.preferredSize.width) pt")
        XCTAssertEqual(RecordingOptionsBar.gifGuidance, "Silent · up to 120 s, trim after recording")
        // Stacked layout: the controls keep their natural width inside the narrowest card.
        let controls = NSHostingView(rootView: GIFFormatControls(options: .constant(.default)).fixedSize())
        XCTAssertLessThanOrEqual(controls.fittingSize.width, RecordingOptionsBar.minimumWidth - 48)
    }

    func testRecordingSurfacesFitTheirReservedSizes() {
        let defaults = temporaryDefaults()
        let settings = AppSettings(defaults: defaults)
        for format in RecordingOutputFormat.allCases {
            var options = RecordingOptions.default
            options.outputFormat = format
            let size = RecordingOptionsBar.preferredSize(for: format)
            let view = NSHostingView(rootView: RecordingOptionsBar(settings: settings, targetLabel: "Record Area", initialOptions: options, onStart: { _ in }, onCancel: {})
                .frame(width: size.width))
            view.layoutSubtreeIfNeeded()
            XCTAssertLessThanOrEqual(view.fittingSize.height, size.height, "\(format) options fit without a scrollbar")
            XCTAssertGreaterThan(view.fittingSize.height, size.height - 70, "\(format) options leave no large empty area")
        }
        let narrow = NSHostingView(rootView: RecordingOptionsBar(settings: settings, targetLabel: "Record Area", initialOptions: .default, compact: true, onStart: { _ in }, onCancel: {})
            .frame(width: RecordingOptionsBar.minimumWidth))
        narrow.layoutSubtreeIfNeeded()
        XCTAssertGreaterThan(narrow.fittingSize.height, RecordingOptionsBar.preferredSize.height, "The stacked layout is taller and scrolls")
        XCTAssertTrue(RecordingOptionsBar.usesCompactLayout(width: RecordingOptionsBar.minimumWidth))
        XCTAssertFalse(RecordingOptionsBar.usesCompactLayout(width: RecordingOptionsBar.preferredSize.width))

        // Every chip combination must fit the HUD width without truncation.
        for (mic, audio, clicks, keys, hidden, warning) in [
            (true, true, ClickOverlayMode.ringsAndLabels, KeystrokeCaptureMode.shortcutsOnly, false, nil as String?),
            (true, true, .ringsAndLabels, .allKeys, true, nil),
            (false, false, .off, .off, false, nil),
            (true, true, .ringsAndLabels, .maskedPrintable, false, "Input tracking could not start. Check Input Monitoring permission in System Settings."),
        ] {
            let runtime = RecordingRuntimeState(sessionID: RecordingSessionID(), startedAt: Date(), targetDescription: "Area", elapsed: 12,
                isMicEnabled: mic, isSystemAudioEnabled: audio, isInputOverlayEnabled: clicks.isEnabled || keys != .off, isSecureFieldHidden: hidden,
                clickOverlayMode: clicks, keystrokeMode: keys, inputOverlayWarning: warning, outputFormat: .gif)
            let hud = NSHostingView(rootView: RecordingHUDView(runtime: runtime, isPaused: false, onPauseResume: {}, onStop: {}, onInputOverlaysChange: { _, _ in }))
            XCTAssertLessThanOrEqual(hud.fittingSize.width, RecordingHUDView.preferredSize.width, "HUD chips overflow for \(RecordingHUDView.inputChipTitle(for: runtime)) hidden=\(hidden)")
            XCTAssertLessThanOrEqual(hud.fittingSize.height, RecordingHUDView.preferredSize.height)
        }
        let tracking = RecordingRuntimeState(sessionID: RecordingSessionID(), startedAt: Date(), targetDescription: "Area", elapsed: 0,
            isMicEnabled: false, isSystemAudioEnabled: false, isInputOverlayEnabled: true, isSecureFieldHidden: false,
            clickOverlayMode: .ringsAndLabels, keystrokeMode: .shortcutsOnly)
        XCTAssertEqual(RecordingHUDView.inputChipTitle(for: tracking), "Clicks + shortcuts")
        let conversion = NSHostingView(rootView: RecordingConversionView(model: RecordingConversionProgress(progress: 0.4), onCancel: {}))
        XCTAssertLessThanOrEqual(conversion.fittingSize.height, RecordingConversionView.preferredSize.height + 1)
        let review = NSHostingView(rootView: RecordingReviewView(viewModel: RecordingReviewViewModel(
            fileURL: URL(fileURLWithPath: "/unused.mov"), gifURL: URL(fileURLWithPath: "/unused.gif"))))
        review.frame = CGRect(origin: .zero, size: RecordingReviewView.preferredSize)
        review.layoutSubtreeIfNeeded()
        XCTAssertGreaterThan(review.fittingSize.width, 0)
        let converter = NSHostingView(rootView: VideoToGIFView(viewModel: VideoToGIFViewModel(), onMinimize: {}))
        XCTAssertLessThanOrEqual(converter.fittingSize.height, VideoToGIFView.preferredSize.height + 1)
    }

    func testGIFDefaultsPersistAndClamp() {
        let defaults = temporaryDefaults()
        let settings = AppSettings(defaults: defaults)
        XCTAssertEqual(settings.recordingOutputFormat, .movie)
        XCTAssertEqual(settings.gifExportOptions, .default)
        settings.recordingOutputFormat = .gif
        settings.gifExportOptions = GIFExportOptions(framesPerSecond: 99, maxWidth: 5, loops: false)
        XCTAssertEqual(settings.recordingGIFFrameRate, 20, "Clamped to the offered range")
        XCTAssertEqual(settings.recordingGIFMaxWidth, 64)
        let reloaded = AppSettings(defaults: defaults)
        XCTAssertEqual(reloaded.recordingOutputFormat, .gif)
        XCTAssertEqual(reloaded.gifExportOptions, GIFExportOptions(framesPerSecond: 20, maxWidth: 64, loops: false))
        XCTAssertEqual(reloaded.recordingOptions.outputFormat, .gif)
        XCTAssertEqual(reloaded.recordingOptions.gif.maxWidth, 64)
        reloaded.resetRecordingOptions()
        XCTAssertEqual(reloaded.recordingOutputFormat, .movie)
        XCTAssertEqual(reloaded.gifExportOptions, .default)
        defaults.set("bad-format", forKey: "recordingOutputFormat")
        XCTAssertEqual(AppSettings(defaults: defaults).recordingOutputFormat, .movie)
    }
}

private final class StubEngine: RecordingEngineControlling {
    var onFailure: ((Error) -> Void)?
    var onSecureFieldHiddenChange: ((Bool) -> Void)?
    private var stopContinuation: CheckedContinuation<URL, Error>?
    private var pendingStop: URL?

    func start() async throws -> RecordingRuntimeState {
        RecordingRuntimeState(sessionID: RecordingSessionID(), startedAt: Date(), targetDescription: "Stub", elapsed: 0,
                              isMicEnabled: false, isSystemAudioEnabled: false, isInputOverlayEnabled: false, isSecureFieldHidden: false)
    }
    func setPaused(_ paused: Bool) {}
    func updateInputOverlays(clicks: ClickOverlayMode, keys: KeystrokeCaptureMode) -> Bool { true }
    func stop() async throws -> URL {
        if let pendingStop { return pendingStop }
        return try await withCheckedThrowingContinuation { continuation in stopContinuation = continuation }
    }
    func cancel() {}
    func completeStop(with url: URL) {
        if let stopContinuation { stopContinuation.resume(returning: url); self.stopContinuation = nil } else { pendingStop = url }
    }
}

private extension RecordingPermissionState {
    static let granted = RecordingPermissionState(
        screenRecording: .granted, microphone: .granted, inputMonitoring: .granted, accessibility: .granted, systemAudio: .granted
    )
}
