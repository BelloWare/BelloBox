import XCTest
@testable import BelloBox

final class ImageStitcherTests: XCTestCase {
    func testTwoOverlappingFramesStitchIntoExpectedHeight() throws {
        let full = ScreenshotTestHelpers.stripedImage(width: 120, height: 300)
        let first = try XCTUnwrap(full.cropping(to: CGRect(x: 0, y: 0, width: 120, height: 200)))
        let second = try XCTUnwrap(full.cropping(to: CGRect(x: 0, y: 100, width: 120, height: 200)))
        let result = try ImageStitcher.stitch([first, second])
        XCTAssertEqual(result.image.height, 300, accuracy: 8)
    }

    func testUpwardFramesAreReorderedBeforeStitching() throws {
        let full = ScreenshotTestHelpers.stripedImage(width: 120, height: 300)
        let top = try XCTUnwrap(full.cropping(to: CGRect(x: 0, y: 0, width: 120, height: 200)))
        let bottom = try XCTUnwrap(full.cropping(to: CGRect(x: 0, y: 100, width: 120, height: 200)))
        var config = StitchConfig.default
        config.direction = .up

        let result = try ImageStitcher.stitch([bottom, top], config: config)

        XCTAssertEqual(result.image.height, 300, accuracy: 8)
        XCTAssertEqual(result.placements.map(\.frameIndex), [1, 0])
    }

    func testThreeOverlappingFramesStitchIntoExpectedHeight() throws {
        let full = ScreenshotTestHelpers.stripedImage(width: 120, height: 460)
        let first = try XCTUnwrap(full.cropping(to: CGRect(x: 0, y: 0, width: 120, height: 220)))
        let second = try XCTUnwrap(full.cropping(to: CGRect(x: 0, y: 120, width: 120, height: 220)))
        let third = try XCTUnwrap(full.cropping(to: CGRect(x: 0, y: 240, width: 120, height: 220)))
        let result = try ImageStitcher.stitch([first, second, third])
        XCTAssertEqual(result.image.height, 460, accuracy: 12)
        XCTAssertEqual(result.placements.count, 3)
    }

    func testNoOverlapReturnsWarningAndAppends() throws {
        let first = ScreenshotTestHelpers.image(width: 80, height: 160) { context in
            context.setFillColor(NSColor.red.cgColor); context.fill(CGRect(x: 0, y: 0, width: 80, height: 160))
        }
        let second = ScreenshotTestHelpers.image(width: 80, height: 160) { context in
            context.setFillColor(NSColor.blue.cgColor); context.fill(CGRect(x: 0, y: 0, width: 80, height: 160))
        }
        let result = try ImageStitcher.stitch([first, second])
        XCTAssertFalse(result.warnings.isEmpty)
        XCTAssertEqual(result.image.height, 320)
    }

    func testUnchangedFrameIsDetected() throws {
        let first = ScreenshotTestHelpers.stripedImage(width: 90, height: 180)
        let result = try ImageStitcher.stitch([first, first])
        XCTAssertTrue(ImageStitcher.appearsUnchanged(previous: first, current: first))
        XCTAssertTrue(result.warnings.contains { $0.contains("nearly unchanged") })
    }

    func testStitchWarningsAreActiveInScrollingDocument() throws {
        let image = ScreenshotTestHelpers.stripedImage(width: 90, height: 180)
        let result = StitchResult(
            image: image,
            placements: [],
            warnings: ["Frame 2 appears nearly unchanged from the previous frame."]
        )

        let document = ScrollCaptureEngine.makeDocument(
            from: result,
            target: ScrollCaptureTargetSummary(title: "Page", ownerName: "Browser", frame: nil),
            frameCount: 2,
            createdAt: Date(timeIntervalSince1970: 12)
        )

        XCTAssertEqual(document.ocrResults.count, 1)
        XCTAssertEqual(document.activeOCRResult?.warnings, result.warnings)
    }

    func testStickyHeaderIsRemovedWhenRepeatedConservatively() throws {
        let first = imageWithHeader(width: 100, height: 200, bodyColor: .red)
        let second = imageWithHeader(width: 100, height: 200, bodyColor: .blue)
        let result = try ImageStitcher.stitch([first, second])
        XCTAssertGreaterThanOrEqual(result.placements[1].croppedTop, 24)
        XCTAssertLessThan(result.image.height, 400)
    }

    func testRepeatedMiddleContentIsPreserved() throws {
        let first = imageWithMiddleBand(width: 100, height: 200, topColor: .red, bottomColor: .blue)
        let second = imageWithMiddleBand(width: 100, height: 200, topColor: .green, bottomColor: .purple)
        let result = try ImageStitcher.stitch([first, second])
        XCTAssertEqual(result.placements[1].croppedTop, 0)
        XCTAssertEqual(result.image.height, 400)
    }

    func testWidthMismatchIsNormalizedToFirstFrameWidth() throws {
        let first = ScreenshotTestHelpers.stripedImage(width: 120, height: 180)
        let second = ScreenshotTestHelpers.stripedImage(width: 90, height: 180)
        let result = try ImageStitcher.stitch([first, second])
        XCTAssertEqual(result.image.width, 120)
    }

    func testStickyFooterIsKeptOnceAndContentUnderItIsRecovered() throws {
        // A 1000-row document of pseudo-random rows smoothed over five neighbours (no
        // periodicity); every frame shows 300 rows with a fixed 40-row bar painted over
        // its bottom, like a status or input bar.
        var state: UInt32 = 0x2545_F491
        func next() -> CGFloat {
            state ^= state << 13
            state ^= state >> 17
            state ^= state << 5
            return CGFloat(state % 1000) / 999
        }
        let raw: [[CGFloat]] = (0..<1000).map { _ in [next(), next(), next()] }
        let document = ScreenshotTestHelpers.image(width: 120, height: 1000) { context in
            for y in 0..<1000 {
                var color: [CGFloat] = [0, 0, 0]
                var count: CGFloat = 0
                for neighbour in max(0, y - 2)...min(999, y + 2) {
                    for channel in 0..<3 { color[channel] += raw[neighbour][channel] }
                    count += 1
                }
                context.setFillColor(CGColor(red: color[0] / count, green: color[1] / count, blue: color[2] / count, alpha: 1))
                context.fill(CGRect(x: 0, y: 1000 - 1 - y, width: 120, height: 1))
            }
        }
        func frame(at offset: Int) -> CGImage {
            let window = document.cropping(to: CGRect(x: 0, y: offset, width: 120, height: 300))!
            return ScreenshotTestHelpers.image(width: 120, height: 300) { context in
                context.draw(window, in: CGRect(x: 0, y: 0, width: 120, height: 300))
                context.setFillColor(NSColor(calibratedWhite: 0.12, alpha: 1).cgColor)
                context.fill(CGRect(x: 0, y: 0, width: 120, height: 40))
                context.setFillColor(NSColor.orange.cgColor)
                context.fill(CGRect(x: 10, y: 12, width: 50, height: 16))
            }
        }
        // 150-row steps leave 110 rows of visible content overlap above the bar.
        let frames = [frame(at: 0), frame(at: 150), frame(at: 300)]

        XCTAssertEqual(ImageStitcher.repeatedFooterHeight(first: frames[0], current: frames[1]), 40)
        let result = try ImageStitcher.stitch(frames)

        // 300 + 150 + 150 rows of content; the bar appears once, at the very bottom.
        XCTAssertEqual(result.image.height, 600)
        // The bar (40 rows, measured exactly) plus the seam slack is cropped from every
        // frame but the last; the next frame draws those rows instead.
        XCTAssertEqual(result.placements.map(\.croppedBottom), [40 + ImageStitcher.seamSlackRows, 40 + ImageStitcher.seamSlackRows, 0])
        XCTAssertTrue(result.warnings.isEmpty, "\(result.warnings)")
        // Row 280 was hidden under the first frame's bar and must come from the second
        // frame (document row 280); row 450 sat under the second frame's bar.
        XCTAssertEqual(ScreenshotTestHelpers.pixel(result.image, x: 60, y: 280), ScreenshotTestHelpers.pixel(document, x: 60, y: 280))
        XCTAssertEqual(ScreenshotTestHelpers.pixel(result.image, x: 60, y: 450), ScreenshotTestHelpers.pixel(document, x: 60, y: 450))
        // The bar itself is at the bottom of the output.
        XCTAssertEqual(ScreenshotTestHelpers.pixel(result.image, x: 100, y: 590), ScreenshotTestHelpers.pixel(frames[2], x: 100, y: 290))
    }

    func testOffGridAndTallFootersComeOutExact() throws {
        for barHeight in [44, 120] {
            let document = smoothRandomDocument(width: 120, height: 1000, seed: UInt32(1000 + barHeight))
            func frame(at offset: Int) -> CGImage {
                let window = document.cropping(to: CGRect(x: 0, y: offset, width: 120, height: 300))!
                return ScreenshotTestHelpers.image(width: 120, height: 300) { context in
                    context.draw(window, in: CGRect(x: 0, y: 0, width: 120, height: 300))
                    context.setFillColor(NSColor(calibratedWhite: 0.14, alpha: 1).cgColor)
                    context.fill(CGRect(x: 0, y: 0, width: 120, height: barHeight))
                    context.setFillColor(NSColor.systemTeal.cgColor)
                    context.fill(CGRect(x: 10, y: 8, width: 60, height: 12))
                }
            }
            // Steps that leave at least 100 rows of visible content overlap above the bar.
            let step = barHeight == 44 ? 150 : 80
            let frames = [frame(at: 0), frame(at: step), frame(at: 2 * step)]

            let result = try ImageStitcher.stitch(frames)
            XCTAssertEqual(result.image.height, 300 + 2 * step, "bar \(barHeight): \(result.placements)")
            XCTAssertTrue(result.warnings.isEmpty, "bar \(barHeight): \(result.warnings)")
            // No sliver of the bar at either seam, and the rows under it recovered (rows
            // under the last frame's own bar stay covered, as on the real page).
            let visible = 300 - barHeight
            for y in [visible - 2, visible + 2, visible + step - 2, visible + step + 2] {
                XCTAssertEqual(ScreenshotTestHelpers.pixel(result.image, x: 60, y: y), ScreenshotTestHelpers.pixel(document, x: 60, y: y), "bar \(barHeight) row \(y)")
            }
            // The bar survives once, at the bottom.
            XCTAssertEqual(ScreenshotTestHelpers.pixel(result.image, x: 100, y: 300 + 2 * step - 4), ScreenshotTestHelpers.pixel(frames[2], x: 100, y: 296))
        }
    }

    func testBlankBottomMarginIsNotTreatedAsABar() throws {
        // Sparse page: textured rows, then a 90-row blank band at the bottom of the
        // first frame that also appears at the bottom of the next frame.
        let textured = smoothRandomDocument(width: 120, height: 900, seed: 77)
        let document = ScreenshotTestHelpers.image(width: 120, height: 900) { context in
            context.setFillColor(NSColor.white.cgColor)
            context.fill(CGRect(x: 0, y: 0, width: 120, height: 900))
            for band in [(0, 210), (300, 510), (600, 810)] {
                let crop = textured.cropping(to: CGRect(x: 0, y: band.0, width: 120, height: band.1 - band.0))!
                context.draw(crop, in: CGRect(x: 0, y: 900 - band.1, width: 120, height: band.1 - band.0))
            }
        }
        let a = try XCTUnwrap(document.cropping(to: CGRect(x: 0, y: 0, width: 120, height: 300)))
        let b = try XCTUnwrap(document.cropping(to: CGRect(x: 0, y: 190, width: 120, height: 300)))

        XCTAssertNil(ImageStitcher.repeatedFooterHeight(first: a, current: b))
        let result = try ImageStitcher.stitch([a, b])
        XCTAssertEqual(result.image.height, 490)
        XCTAssertTrue(result.warnings.isEmpty, "\(result.warnings)")
    }

    func testBarThatDisappearsOnTheLastFrameIsStillRemovedFromEarlierFrames() throws {
        let document = smoothRandomDocument(width: 120, height: 1000, seed: 5)
        func frame(at offset: Int, bar: Bool) -> CGImage {
            let window = document.cropping(to: CGRect(x: 0, y: offset, width: 120, height: 300))!
            return ScreenshotTestHelpers.image(width: 120, height: 300) { context in
                context.draw(window, in: CGRect(x: 0, y: 0, width: 120, height: 300))
                guard bar else { return }
                context.setFillColor(NSColor(calibratedWhite: 0.14, alpha: 1).cgColor)
                context.fill(CGRect(x: 0, y: 0, width: 120, height: 40))
                context.setFillColor(NSColor.systemPink.cgColor)
                context.fill(CGRect(x: 10, y: 8, width: 60, height: 12))
            }
        }
        let frames = [frame(at: 0, bar: true), frame(at: 150, bar: true), frame(at: 300, bar: false)]

        let result = try ImageStitcher.stitch(frames)

        XCTAssertEqual(result.image.height, 600, "\(result.placements)")
        XCTAssertTrue(result.warnings.isEmpty, "\(result.warnings)")
        for y in [258, 262, 408, 412, 500] {
            XCTAssertEqual(ScreenshotTestHelpers.pixel(result.image, x: 60, y: y), ScreenshotTestHelpers.pixel(document, x: 60, y: y), "row \(y)")
        }
    }

    /// Pseudo-random rows smoothed over five neighbours: locally similar, globally unique.
    private func smoothRandomDocument(width: Int, height: Int, seed: UInt32) -> CGImage {
        var state = seed | 1
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
                context.fill(CGRect(x: 0, y: height - 1 - y, width: width, height: 1))
            }
        }
    }

    func testBlankSeamPrefersTheLargerOverlap() throws {
        // Content: 200 textured rows, 100 blank rows, 200 textured rows. Frame A shows rows
        // 0..300 (bottom third blank); frame B shows rows 200..500 (top third blank). Every
        // overlap up to 100 rows ties at zero; the true overlap is the largest, 100.
        let content = ScreenshotTestHelpers.image(width: 120, height: 500) { context in
            context.setFillColor(NSColor.white.cgColor)
            context.fill(CGRect(x: 0, y: 0, width: 120, height: 500))
            for y in 0..<500 where y < 200 || y >= 300 {
                let value = CGFloat((y * 53) % 97) / 97
                context.setFillColor(NSColor(calibratedRed: value, green: 0.2, blue: 1 - value, alpha: 1).cgColor)
                context.fill(CGRect(x: 0, y: 500 - 1 - y, width: 120, height: 1))
            }
        }
        let a = try XCTUnwrap(content.cropping(to: CGRect(x: 0, y: 0, width: 120, height: 300)))
        let b = try XCTUnwrap(content.cropping(to: CGRect(x: 0, y: 200, width: 120, height: 300)))

        let match = try XCTUnwrap(ImageStitcher.bestOverlap(previous: a, current: b, config: .default))
        XCTAssertEqual(match.overlap, 100)
        XCTAssertEqual(try ImageStitcher.stitch([a, b]).image.height, 500)
    }

    func testStitchRespectsCancellationBeforeWork() async {
        let image = ScreenshotTestHelpers.stripedImage(width: 120, height: 180)
        let task = Task {
            while !Task.isCancelled {
                await Task.yield()
            }
            _ = try ImageStitcher.stitch([image])
        }

        task.cancel()

        do {
            try await task.value
            XCTFail("Expected stitch to throw CancellationError.")
        } catch is CancellationError {
            // Expected.
        } catch {
            XCTFail("Expected CancellationError, got \(error).")
        }
    }

    private func imageWithHeader(width: Int, height: Int, bodyColor: NSColor) -> CGImage {
        ScreenshotTestHelpers.image(width: width, height: height) { context in
            context.setFillColor(NSColor(calibratedWhite: 0.22, alpha: 1).cgColor)
            context.fill(CGRect(x: 0, y: height - 32, width: width, height: 32))
            context.setFillColor(bodyColor.cgColor)
            context.fill(CGRect(x: 0, y: 0, width: width, height: height - 32))
        }
    }

    private func imageWithMiddleBand(width: Int, height: Int, topColor: NSColor, bottomColor: NSColor) -> CGImage {
        ScreenshotTestHelpers.image(width: width, height: height) { context in
            context.setFillColor(bottomColor.cgColor)
            context.fill(CGRect(x: 0, y: 0, width: width, height: 80))
            context.setFillColor(NSColor(calibratedWhite: 0.45, alpha: 1).cgColor)
            context.fill(CGRect(x: 0, y: 80, width: width, height: 40))
            context.setFillColor(topColor.cgColor)
            context.fill(CGRect(x: 0, y: 120, width: width, height: height - 120))
        }
    }
}
