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

        let document = ScrollCaptureCoordinator.makeDocument(
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
