import CoreGraphics
import XCTest
@testable import BelloBox

@MainActor
final class ScrollCaptureEngineTests: XCTestCase {
    private let width = 160
    private let viewport = 300

    func testStaticContentNeverAppendsAFrame() {
        let content = makeContent(height: 1200)
        let engine = makeEngine(initialFrame: window(content, offset: 0))
        engine.start()

        for _ in 0..<5 { engine.processSample(window(content, offset: 0)) }

        XCTAssertEqual(engine.frames.count, 1)
    }

    func testScrolledContentIsAppendedOnceItSettles() {
        let content = makeContent(height: 1200)
        let engine = makeEngine(initialFrame: window(content, offset: 0))
        engine.start()

        engine.processSample(window(content, offset: 90)) // moved, overlap 70%: wait for it to settle
        XCTAssertEqual(engine.frames.count, 1)
        engine.processSample(window(content, offset: 90)) // same again: settled
        XCTAssertEqual(engine.frames.count, 2)
        engine.processSample(window(content, offset: 90)) // still the appended frame: nothing
        XCTAssertEqual(engine.frames.count, 2)
    }

    func testFastScrollIsAppendedBeforeItSettlesWhileOverlapRemains() {
        let content = makeContent(height: 1200)
        let engine = makeEngine(initialFrame: window(content, offset: 0))
        engine.start()

        engine.processSample(window(content, offset: 180)) // overlap 120 px = 40% < 45%: append now
        XCTAssertEqual(engine.frames.count, 2)
    }

    func testContinuouslyChangingContentIsNeverAppended() {
        let engine = makeEngine(initialFrame: ScreenshotTestHelpers.stripedImage(width: width, height: viewport))
        engine.start()

        for seed in 1...6 {
            engine.processSample(noise(seed: seed))
        }

        XCTAssertEqual(engine.frames.count, 1)
    }

    func testScrollingBackwardsIsIgnoredAndReported() {
        let content = makeContent(height: 1200)
        let engine = makeEngine(initialFrame: window(content, offset: 400))
        engine.start()

        engine.processSample(window(content, offset: 300))
        engine.processSample(window(content, offset: 300))

        XCTAssertEqual(engine.frames.count, 1, "backwards scroll must not be appended: \(engine.debugEvents)")
        XCTAssertEqual(engine.message, "Scroll down to capture more.")
    }

    func testWithoutInitialFrameTheFirstSampleOpensTheSequence() {
        let content = makeContent(height: 1200)
        let engine = ScrollCaptureEngine(
            area: CaptureArea(cocoaRect: CGRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(viewport)), displayID: 1),
            summary: ScrollCaptureTargetSummary(title: "Test", ownerName: nil, frame: nil),
            initialFrame: nil,
            captureSample: { throw CancellationError() },
            postScroll: { _ in }
        )
        engine.start()
        XCTAssertFalse(engine.canFinish)

        engine.processSample(window(content, offset: 0))
        XCTAssertEqual(engine.frames.count, 1)
        XCTAssertTrue(engine.canFinish)
        engine.processSample(window(content, offset: 0))
        XCTAssertEqual(engine.frames.count, 1)
    }

    func testStickyHeaderDoesNotPreventMatching() async throws {
        let content = makeContent(height: 1200)
        let engine = makeEngine(initialFrame: withHeader(window(content, offset: 0)))
        engine.start()

        engine.processSample(withHeader(window(content, offset: 100)))
        engine.processSample(withHeader(window(content, offset: 100)))

        XCTAssertEqual(engine.frames.count, 2)
        XCTAssertTrue(engine.debugEvents.contains { $0.hasPrefix("moved") }, "the header must be skipped so the scroll is matched: \(engine.debugEvents)")
        let document = try await engine.finish()
        XCTAssertEqual(document.baseImage.height, viewport + 100)
    }

    func testStickyFooterDoesNotPreventMatching() async throws {
        let content = makeContent(height: 1200)
        let engine = makeEngine(initialFrame: withFooter(window(content, offset: 0)))
        engine.start()

        engine.processSample(withFooter(window(content, offset: 100)))
        engine.processSample(withFooter(window(content, offset: 100)))

        XCTAssertEqual(engine.frames.count, 2)
        XCTAssertTrue(engine.debugEvents.contains { $0.hasPrefix("moved") }, "the footer must be skipped so the scroll is matched: \(engine.debugEvents)")
        let document = try await engine.finish()
        XCTAssertEqual(document.baseImage.height, viewport + 100)
    }

    func testInPlaceChangeIsIgnoredButAFullPageJumpIsKept() {
        let content = makeContent(height: 1200)
        let engine = makeEngine(initialFrame: window(content, offset: 0))
        engine.start()

        // A widget expanded in place: a fifth of the frame changed, nothing scrolled.
        let changed = replacingRows(of: window(content, offset: 0), from: 100, count: 60, with: noise(seed: 4))
        engine.processSample(changed)
        engine.processSample(changed)
        XCTAssertEqual(engine.frames.count, 1, "in-place change must not be appended: \(engine.debugEvents)")

        // Jumped more than a page: unrelated content that then settles is kept.
        let far = window(content, offset: 700)
        engine.processSample(far)
        engine.processSample(far)
        XCTAssertEqual(engine.frames.count, 2, "a settled full-page jump must be appended: \(engine.debugEvents)")
    }

    func testFinishDropsTrailingFramesWhenTheOutputWouldBeTooTall() async throws {
        let content = makeContent(height: 1200)
        var configuration = ScrollCaptureEngine.Configuration()
        configuration.stitch.maxOutputHeightPx = 400
        let engine = makeEngine(initialFrame: window(content, offset: 0), configuration: configuration)
        engine.start()
        for offset in [90, 180] {
            engine.processSample(window(content, offset: offset))
            engine.processSample(window(content, offset: offset))
        }
        XCTAssertEqual(engine.frames.count, 3)

        let document = try await engine.finish()

        XCTAssertEqual(document.baseImage.height, viewport + 90)
        XCTAssertEqual(document.source.scrollingFrameCount, 2)
        XCTAssertTrue(document.activeOCRResult?.warnings.contains { $0.contains("left out") } == true)
    }

    func testPreviewMatchesTheStitcherWhenTheMeasuredFooterChanges() async throws {
        // Blank padding right above the bar in the first two frames makes their footer
        // measure taller (100 rows) than the third frame's (40). The engine must match
        // each frame against the previous frame's footer, as the stitcher does, or the
        // preview seam lands 60 rows off.
        let content = blanking(makeContent(height: 1400), rows: [200..<260, 300..<360])
        let engine = makeEngine(initialFrame: withFooter(window(content, offset: 0)))
        engine.start()
        for offset in [100, 200] {
            engine.processSample(withFooter(window(content, offset: offset)))
            engine.processSample(withFooter(window(content, offset: offset)))
        }
        XCTAssertEqual(engine.frames.count, 3)

        let document = try await engine.finish()
        XCTAssertEqual(document.baseImage.height, viewport + 200)
        XCTAssertEqual(engine.previewRowCount, document.baseImage.height)
    }

    func testStitchFailureIsReportedWhenWatchingResumes() async throws {
        let content = makeContent(height: 1200)
        var configuration = ScrollCaptureEngine.Configuration()
        configuration.stitch.maxOutputHeightPx = 100 // even a single frame is too tall
        let engine = makeEngine(initialFrame: window(content, offset: 0), configuration: configuration)
        engine.start()
        engine.processSample(window(content, offset: 90))
        engine.processSample(window(content, offset: 90))
        XCTAssertEqual(engine.frames.count, 2)

        do {
            _ = try await engine.finish()
            XCTFail("stitching should fail when a single frame exceeds the height limit")
        } catch {
            guard case .failed = engine.phase else { return XCTFail("phase should be failed, was \(engine.phase)") }
        }

        engine.resumeWatching()
        XCTAssertEqual(engine.phase, .watching)
        XCTAssertTrue(engine.message?.hasPrefix("Stitching failed") == true, "message: \(engine.message ?? "nil")")
        XCTAssertTrue(engine.canFinish)
    }

    func testStopsAtMaximumFrameCount() {
        let content = makeContent(height: 1200)
        var configuration = ScrollCaptureEngine.Configuration()
        configuration.maxFrames = 3
        let engine = makeEngine(initialFrame: window(content, offset: 0), configuration: configuration)
        engine.start()

        for offset in [90, 180, 270, 360] {
            engine.processSample(window(content, offset: offset))
            engine.processSample(window(content, offset: offset))
        }

        XCTAssertEqual(engine.frames.count, 3)
        XCTAssertEqual(engine.message, "Maximum of 3 frames reached. Press Done to stitch.")
    }

    func testFinishStitchesFramesIntoATallerImage() async throws {
        let content = makeContent(height: 1200)
        let engine = makeEngine(initialFrame: window(content, offset: 0))
        engine.start()
        engine.processSample(window(content, offset: 90))
        engine.processSample(window(content, offset: 90))
        engine.processSample(window(content, offset: 180))
        engine.processSample(window(content, offset: 180))

        let document = try await engine.finish()

        XCTAssertEqual(engine.phase, .finished)
        XCTAssertEqual(document.baseImage.width, width)
        XCTAssertEqual(document.baseImage.height, viewport + 180)
        XCTAssertEqual(document.source.scrollingFrameCount, 3)
    }

    func testAutoScrollFlipsDirectionWhenNeededAndStopsAtTheEnd() async {
        let content = makeContent(height: 1200)
        let page = FakeScrollablePage(content: content, viewport: viewport, maxOffset: 1200 - viewport)
        var configuration = ScrollCaptureEngine.Configuration()
        configuration.sampleInterval = 0.01
        configuration.autoScrollTimeout = 0.25
        let engine = ScrollCaptureEngine(
            area: CaptureArea(cocoaRect: CGRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(viewport)), displayID: 1),
            summary: ScrollCaptureTargetSummary(title: "Fake", ownerName: nil, frame: nil),
            initialFrame: page.currentFrame(),
            configuration: configuration,
            captureSample: { page.currentFrame() },
            // "Natural" scrolling: a positive request moves the content the other way.
            postScroll: { points in page.scroll(by: -points) },
            isAccessibilityTrusted: { true }
        )
        engine.start()
        engine.toggleAutoScroll()

        let deadline = Date().addingTimeInterval(20)
        while engine.isAutoScrolling, Date() < deadline {
            try? await Task.sleep(nanoseconds: 20_000_000)
        }

        XCTAssertFalse(engine.isAutoScrolling)
        XCTAssertTrue(engine.reachedEnd)
        XCTAssertEqual(page.offset, 1200 - viewport)
        // 900 px of travel in 165 px steps (55% of 300) plus the first frame.
        XCTAssertEqual(engine.frames.count, 7)
        engine.stop()
    }

    func testAutoScrollProbeAtTheEndLeavesThePageWhereItWas() async {
        let content = makeContent(height: 1200)
        let page = FakeScrollablePage(content: content, viewport: viewport, maxOffset: 1200 - viewport)
        var configuration = ScrollCaptureEngine.Configuration()
        configuration.sampleInterval = 0.01
        configuration.autoScrollTimeout = 0.25
        let engine = ScrollCaptureEngine(
            area: CaptureArea(cocoaRect: CGRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(viewport)), displayID: 1),
            summary: ScrollCaptureTargetSummary(title: "Fake", ownerName: nil, frame: nil),
            initialFrame: page.currentFrame(),
            configuration: configuration,
            captureSample: { page.currentFrame() },
            postScroll: { points in page.scroll(by: points) }, // wheel direction already correct
            isAccessibilityTrusted: { true }
        )
        engine.start()
        engine.toggleAutoScroll()

        let deadline = Date().addingTimeInterval(20)
        while engine.isAutoScrolling, Date() < deadline {
            try? await Task.sleep(nanoseconds: 20_000_000)
        }

        XCTAssertTrue(engine.reachedEnd)
        XCTAssertEqual(engine.frames.count, 7)
        // The probe in the other direction scrolled up once; the undo must bring the page
        // back to the bottom, not push it further up.
        XCTAssertEqual(page.offset, 1200 - viewport, "events: \(engine.debugEvents)")
        engine.stop()
    }

    func testPreviewStripFollowsAppendedFrames() {
        let content = makeContent(height: 1200)
        let engine = makeEngine(initialFrame: withFooter(window(content, offset: 0)))
        engine.start()
        XCTAssertEqual(engine.previewPieces.count, 1)
        XCTAssertEqual(engine.previewRowCount, viewport)
        XCTAssertEqual(engine.previewPieces.first?.image.width, ScrollCaptureEngine.previewWidth)

        engine.processSample(withFooter(window(content, offset: 100)))
        engine.processSample(withFooter(window(content, offset: 100)))

        XCTAssertEqual(engine.frames.count, 2)
        XCTAssertEqual(engine.previewPieces.count, 2)
        // The first piece lost its bar (measured within a few rows) and the seam slack
        // rows, which the second frame draws instead, exactly as the stitcher does; the
        // second adds the new rows plus the bar it now shows.
        let firstPieceEnd = engine.previewPieces[0].toRow
        XCTAssertGreaterThanOrEqual(firstPieceEnd, viewport - 48 - ImageStitcher.seamSlackRows)
        XCTAssertLessThanOrEqual(firstPieceEnd, viewport - 40 - ImageStitcher.seamSlackRows)
        XCTAssertEqual(engine.previewPieces[1].rowCount, viewport + 100 - firstPieceEnd)
        XCTAssertEqual(engine.previewRowCount, viewport + 100)
    }

    func testPreviewHeightMatchesTheStitchedScreenshot() async throws {
        let content = makeContent(height: 1400)
        let engine = makeEngine(initialFrame: withFooter(window(content, offset: 0)))
        engine.start()
        // Two overlapping scrolls, then a jump of more than a page (no overlap: the frame
        // is appended without compaction, bar and all).
        for offset in [120, 260, 700] {
            engine.processSample(withFooter(window(content, offset: offset)))
            engine.processSample(withFooter(window(content, offset: offset)))
        }
        XCTAssertEqual(engine.frames.count, 4)
        XCTAssertEqual(engine.previewPieces.count, 4)

        let document = try await engine.finish()
        XCTAssertEqual(document.baseImage.height, engine.previewRowCount)
    }

    // MARK: - Helpers

    private func makeEngine(
        initialFrame: CGImage,
        configuration: ScrollCaptureEngine.Configuration = ScrollCaptureEngine.Configuration()
    ) -> ScrollCaptureEngine {
        ScrollCaptureEngine(
            area: CaptureArea(cocoaRect: CGRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(viewport)), displayID: 1),
            summary: ScrollCaptureTargetSummary(title: "Test", ownerName: nil, frame: nil),
            initialFrame: initialFrame,
            configuration: configuration,
            captureSample: { throw CancellationError() },
            postScroll: { _ in }
        )
    }

    /// Rows are pseudo-random colours smoothed over five neighbouring rows: short-range
    /// similarity (so the coarse overlap search can lock on within a few rows) but no
    /// long-range structure, so unrelated regions never resemble each other and overlaps
    /// are only found at the true offset.
    private func makeContent(height: Int) -> CGImage {
        let width = self.width
        var state: UInt32 = 0x9E37_79B9
        func next() -> CGFloat {
            state ^= state << 13
            state ^= state >> 17
            state ^= state << 5
            return CGFloat(state % 1000) / 999
        }
        let raw: [[CGFloat]] = (0..<height).map { _ in [next(), next(), next()] }
        return ScreenshotTestHelpers.image(width: width, height: height) { context in
            for y in 0..<height {
                var color: [CGFloat] = [0, 0, 0]
                var count: CGFloat = 0
                for neighbour in max(0, y - 2)...min(height - 1, y + 2) {
                    for channel in 0..<3 { color[channel] += raw[neighbour][channel] }
                    count += 1
                }
                context.setFillColor(CGColor(red: color[0] / count, green: color[1] / count, blue: color[2] / count, alpha: 1))
                context.fill(CGRect(x: 0, y: y, width: width, height: 1))
            }
        }
    }

    private func window(_ content: CGImage, offset: Int) -> CGImage {
        content.cropping(to: CGRect(x: 0, y: offset, width: width, height: viewport))!
    }

    /// Paints a fixed 40-row toolbar over the top of a frame, like browser chrome.
    private func withHeader(_ frame: CGImage) -> CGImage {
        let width = self.width
        let viewport = self.viewport
        return ScreenshotTestHelpers.image(width: width, height: viewport) { context in
            context.draw(frame, in: CGRect(x: 0, y: 0, width: width, height: viewport))
            context.setFillColor(CGColor(red: 0.15, green: 0.15, blue: 0.18, alpha: 1))
            context.fill(CGRect(x: 0, y: viewport - 40, width: width, height: 40))
            context.setFillColor(CGColor(red: 0.9, green: 0.4, blue: 0.1, alpha: 1))
            context.fill(CGRect(x: 12, y: viewport - 28, width: 60, height: 14))
        }
    }

    /// Paints a fixed 40-row bar over the bottom of a frame, like a status or input bar.
    private func withFooter(_ frame: CGImage) -> CGImage {
        let width = self.width
        let viewport = self.viewport
        return ScreenshotTestHelpers.image(width: width, height: viewport) { context in
            context.draw(frame, in: CGRect(x: 0, y: 0, width: width, height: viewport))
            context.setFillColor(CGColor(red: 0.12, green: 0.12, blue: 0.14, alpha: 1))
            context.fill(CGRect(x: 0, y: 0, width: width, height: 40))
            context.setFillColor(CGColor(red: 0.2, green: 0.7, blue: 0.9, alpha: 1))
            context.fill(CGRect(x: 12, y: 12, width: 60, height: 14))
        }
    }

    /// `content` with the given rows (from the top) painted white, like blank padding.
    private func blanking(_ content: CGImage, rows ranges: [Range<Int>]) -> CGImage {
        let width = self.width
        let height = content.height
        return ScreenshotTestHelpers.image(width: width, height: height) { context in
            context.draw(content, in: CGRect(x: 0, y: 0, width: width, height: height))
            context.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
            for range in ranges {
                context.fill(CGRect(x: 0, y: height - range.upperBound, width: width, height: range.count))
            }
        }
    }

    /// Replaces `count` rows starting at `start` (from the top) with rows of `source`.
    private func replacingRows(of frame: CGImage, from start: Int, count: Int, with source: CGImage) -> CGImage {
        let width = self.width
        let viewport = self.viewport
        let patch = source.cropping(to: CGRect(x: 0, y: start, width: width, height: count))!
        return ScreenshotTestHelpers.image(width: width, height: viewport) { context in
            context.draw(frame, in: CGRect(x: 0, y: 0, width: width, height: viewport))
            context.draw(patch, in: CGRect(x: 0, y: viewport - start - count, width: width, height: count))
        }
    }

    private func noise(seed: Int) -> CGImage {
        let width = self.width
        let viewport = self.viewport
        return ScreenshotTestHelpers.image(width: width, height: viewport) { context in
            for y in 0..<viewport {
                let value = CGFloat((y * seed * 13 + seed * 97) % 255) / 255
                context.setFillColor(CGColor(red: value, green: 1 - value, blue: CGFloat(seed % 3) / 2, alpha: 1))
                context.fill(CGRect(x: 0, y: y, width: width, height: 1))
            }
        }
    }
}

@MainActor
private final class FakeScrollablePage {
    private let content: CGImage
    private let viewport: Int
    private let maxOffset: Int
    private(set) var offset = 0

    init(content: CGImage, viewport: Int, maxOffset: Int) {
        self.content = content
        self.viewport = viewport
        self.maxOffset = maxOffset
    }

    func scroll(by points: CGFloat) {
        offset = min(max(offset + Int(points.rounded()), 0), maxOffset)
    }

    func currentFrame() -> CGImage {
        content.cropping(to: CGRect(x: 0, y: offset, width: content.width, height: viewport))!
    }
}
