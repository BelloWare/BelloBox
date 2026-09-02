import CoreGraphics
import XCTest
@testable import BelloBox

final class CaptureWindowOcclusionTests: XCTestCase {
    private let screens = [CGRect(x: 0, y: 0, width: 1440, height: 900)]

    func testWindowBehindAnotherAppsWindowIsOccluded() {
        let entries = [
            entry(number: 90, pid: 999, bounds: CGRect(x: 0, y: 0, width: 1440, height: 900), layer: 1000), // our own overlay: ignored
            entry(number: 20, pid: 200, bounds: CGRect(x: 300, y: 200, width: 400, height: 300)),
            entry(number: 10, pid: 100, bounds: CGRect(x: 100, y: 100, width: 800, height: 600)),
        ]
        let frame = ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(CGRect(x: 100, y: 100, width: 800, height: 600), screenFrames: screens)
        XCTAssertTrue(CaptureWindowCatalog.isOccluded(windowID: 10, frame: frame, entries: entries, ownPID: 999, screenFrames: screens))
    }

    func testOwnRegularWindowCountsAsOccluder() {
        let entries = [
            entry(number: 90, pid: 999, bounds: CGRect(x: 0, y: 0, width: 1440, height: 900), layer: 1000), // capture overlay
            entry(number: 70, pid: 999, bounds: CGRect(x: 200, y: 150, width: 300, height: 200), layer: 3), // our World Clock panel
            entry(number: 10, pid: 100, bounds: CGRect(x: 100, y: 100, width: 800, height: 600)),
        ]
        let frame = ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(CGRect(x: 100, y: 100, width: 800, height: 600), screenFrames: screens)
        XCTAssertTrue(CaptureWindowCatalog.isOccluded(windowID: 10, frame: frame, entries: entries, ownPID: 999, screenFrames: screens))
    }

    func testFrontmostWindowIsNotOccluded() {
        let entries = [
            entry(number: 90, pid: 999, bounds: CGRect(x: 0, y: 0, width: 1440, height: 900), layer: 1000),
            entry(number: 10, pid: 100, bounds: CGRect(x: 100, y: 100, width: 800, height: 600)),
            entry(number: 20, pid: 200, bounds: CGRect(x: 300, y: 200, width: 400, height: 300)),
        ]
        let frame = ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(CGRect(x: 100, y: 100, width: 800, height: 600), screenFrames: screens)
        XCTAssertFalse(CaptureWindowCatalog.isOccluded(windowID: 10, frame: frame, entries: entries, ownPID: 999, screenFrames: screens))
    }

    func testNonOverlappingOrInvisibleWindowsDoNotOcclude() {
        let entries = [
            entry(number: 30, pid: 300, bounds: CGRect(x: 1000, y: 750, width: 300, height: 100)), // beside it
            entry(number: 40, pid: 400, bounds: CGRect(x: 100, y: 100, width: 800, height: 600), alpha: 0), // invisible
            entry(number: 50, pid: 500, bounds: CGRect(x: 100, y: 100, width: 800, height: 600), layer: 3000), // non-selectable layer
            entry(number: 10, pid: 100, bounds: CGRect(x: 100, y: 100, width: 800, height: 600)),
        ]
        let frame = ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(CGRect(x: 100, y: 100, width: 800, height: 600), screenFrames: screens)
        XCTAssertFalse(CaptureWindowCatalog.isOccluded(windowID: 10, frame: frame, entries: entries, ownPID: 999, screenFrames: screens))
    }

    func testAlphaMaskKeepsSnapshotPixelsAndTakesLiveShape() throws {
        let frozen = ScreenshotTestHelpers.stripedImage(width: 40, height: 40)
        let shape = ScreenshotTestHelpers.image(width: 40, height: 40) { context in
            context.clear(CGRect(x: 0, y: 0, width: 40, height: 40))
            context.setFillColor(CGColor(red: 0, green: 0, blue: 1, alpha: 1))
            context.fill(CGRect(x: 10, y: 0, width: 30, height: 40)) // left 10 px column transparent
        }
        let masked = try XCTUnwrap(ImageAlphaMask.apply(shapeOf: shape, to: frozen))
        XCTAssertEqual(masked.width, 40)
        XCTAssertEqual(ScreenshotTestHelpers.pixel(masked, x: 5, y: 20)[3], 0, "transparent in the live shape -> transparent")
        XCTAssertEqual(ScreenshotTestHelpers.pixel(masked, x: 30, y: 20), ScreenshotTestHelpers.pixel(frozen, x: 30, y: 20), "opaque in the live shape -> frozen pixel kept")
        XCTAssertNil(ImageAlphaMask.apply(shapeOf: ScreenshotTestHelpers.image(width: 20, height: 40), to: frozen))
    }

    private func entry(number: UInt32, pid: pid_t, bounds: CGRect, alpha: Double = 1, layer: Int = 0) -> [String: Any] {
        [
            kCGWindowNumber as String: NSNumber(value: number),
            kCGWindowOwnerPID as String: NSNumber(value: pid),
            kCGWindowLayer as String: NSNumber(value: layer),
            kCGWindowAlpha as String: NSNumber(value: alpha),
            kCGWindowBounds as String: bounds.dictionaryRepresentation as NSDictionary,
        ]
    }
}
