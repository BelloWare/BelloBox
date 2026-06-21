import AppKit
import CoreGraphics
import XCTest
@testable import BelloBox

@MainActor
final class CaptureOverlayAccessoryLayoutTests: XCTestCase {
    func testAccessoryPrefersAboveSelectionWhenThereIsRoom() {
        let frame = CaptureOverlayAccessoryLayout.frame(
            selection: CGRect(x: 200, y: 300, width: 300, height: 200),
            bounds: CGRect(x: 0, y: 0, width: 1000, height: 800),
            preferredSize: CGSize(width: 500, height: 60)
        )

        XCTAssertLessThan(frame.maxY, 300)
        XCTAssertGreaterThanOrEqual(frame.minX, 12)
    }

    func testAccessoryFallsBelowSelectionNearTopEdge() {
        let frame = CaptureOverlayAccessoryLayout.frame(
            selection: CGRect(x: 40, y: 20, width: 300, height: 200),
            bounds: CGRect(x: 0, y: 0, width: 600, height: 500),
            preferredSize: CGSize(width: 500, height: 80)
        )

        XCTAssertGreaterThan(frame.minY, 220)
        XCTAssertGreaterThanOrEqual(frame.minX, 12)
        XCTAssertLessThanOrEqual(frame.maxX, 588)
    }

    func testCaptureOverlayCancelsUncommittedSelectionWhenAppResignsActive() throws {
        let controller = makeController()
        defer { controller.cancel() }
        let cancelled = expectation(description: "capture overlay cancelled")

        controller.beginScreenshotForTesting(
            snapshots: [try snapshotForMainScreen()],
            onError: { XCTFail("Unexpected capture overlay error: \($0)") },
            onCancel: { cancelled.fulfill() }
        )

        NotificationCenter.default.post(name: NSApplication.didResignActiveNotification, object: NSApp)

        wait(for: [cancelled], timeout: 1)
    }

    func testCaptureOverlayKeepsInlineEditorWhenAppResignsActiveAfterSelection() throws {
        let controller = makeController()
        defer { controller.cancel() }
        let cancelled = expectation(description: "capture overlay should not cancel locked selection")
        cancelled.isInverted = true
        let snapshot = try snapshotForMainScreen()
        let rect = snapshot.screenFrame.insetBy(dx: max(20, snapshot.screenFrame.width * 0.35), dy: max(20, snapshot.screenFrame.height * 0.35))

        controller.beginScreenshotForTesting(
            snapshots: [snapshot],
            initialSelection: .area(CaptureArea(cocoaRect: rect, displayID: snapshot.displayID)),
            onError: { XCTFail("Unexpected capture overlay error: \($0)") },
            onCancel: { cancelled.fulfill() }
        )

        NotificationCenter.default.post(name: NSApplication.didResignActiveNotification, object: NSApp)

        wait(for: [cancelled], timeout: 0.2)
    }

    private func makeController() -> CaptureOverlayController {
        CaptureOverlayController(
            screenCaptureService: ScreenCaptureService(),
            settings: AppSettings(defaults: temporaryDefaults()),
            macOCRService: MacVisionOCRService()
        )
    }

    private func snapshotForMainScreen() throws -> DisplaySnapshot {
        let screen = try XCTUnwrap(NSScreen.main)
        let displayID = try XCTUnwrap(ScreenCoordinateSpace.displayID(for: screen))
        return DisplaySnapshot(
            displayID: displayID,
            screenFrame: screen.frame,
            scale: 1,
            image: ScreenshotTestHelpers.image(width: 320, height: 200)
        )
    }

    private func temporaryDefaults() -> UserDefaults {
        let suiteName = "BelloBoxTests.CaptureOverlay.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
