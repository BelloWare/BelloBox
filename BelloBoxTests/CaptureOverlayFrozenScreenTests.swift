import AppKit
import XCTest
@testable import BelloBox

@MainActor
final class CaptureOverlayFrozenScreenTests: XCTestCase {
    func testScreenshotOverlayFreezesEveryDisplayBeforeShowingWindows() throws {
        try XCTSkipUnless(ScreenCapturePermission.isTrusted, "Screen Recording permission is required to freeze displays.")
        let controller = makeController()
        defer { controller.cancel() }

        controller.beginScreenshot(policy: .areaOnly, onError: { XCTFail("Unexpected overlay error: \($0)") })

        // Nothing may appear on screen until the displays are frozen, otherwise the
        // window under the pointer loses its hover state before the capture.
        XCTAssertEqual(controller.debugOverlayWindowCount, 0)

        let deadline = Date().addingTimeInterval(15)
        while controller.debugOverlayWindowCount == 0, Date() < deadline {
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        }

        XCTAssertEqual(controller.debugOverlayWindowCount, NSScreen.screens.count)
        XCTAssertGreaterThanOrEqual(controller.debugOverlayViewsWithSnapshotCount, 1)
        XCTAssertEqual(controller.debugOverlayViewsWithSnapshotCount, controller.debugSnapshots.count)
    }

    func testNoOverlayWindowExistsWhenSnapshotsComplete() throws {
        try XCTSkipUnless(ScreenCapturePermission.isTrusted, "Screen Recording permission is required to freeze displays.")
        let controller = makeController()
        defer { controller.cancel() }
        var observed: (windows: Int, snapshots: Int)?
        controller.debugSnapshotPhaseObserver = { observed = ($0, $1) }

        controller.beginScreenshot(policy: .areaOnly, onError: { XCTFail("Unexpected overlay error: \($0)") })
        let deadline = Date().addingTimeInterval(15)
        while observed == nil, Date() < deadline {
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        }

        let phase = try XCTUnwrap(observed)
        XCTAssertEqual(phase.windows, 0, "overlay windows must not exist before the displays are frozen")
        XCTAssertGreaterThanOrEqual(phase.snapshots, 1)
    }

    func testCancellingWhileFreezingNeverShowsWindows() throws {
        try XCTSkipUnless(ScreenCapturePermission.isTrusted, "Screen Recording permission is required to freeze displays.")
        let controller = makeController()
        controller.beginScreenshot(policy: .areaOnly, onError: { XCTFail("Unexpected overlay error: \($0)") })
        controller.cancel()

        RunLoop.main.run(until: Date().addingTimeInterval(1.5))

        XCTAssertEqual(controller.debugOverlayWindowCount, 0)
    }

    func testCaptureDisplaySnapshotsCoversEveryScreen() async throws {
        try XCTSkipUnless(ScreenCapturePermission.isTrusted, "Screen Recording permission is required to capture displays.")
        let snapshots = try await ScreenCaptureService().captureDisplaySnapshots(
            options: CaptureOptions(includeCursor: false, hideBelloBoxWindows: false, delayAfterHidingOverlays: 0)
        )

        XCTAssertGreaterThanOrEqual(snapshots.count, 1)
        XCTAssertLessThanOrEqual(snapshots.count, NSScreen.screens.count)
        for snapshot in snapshots {
            let screen = try XCTUnwrap(NSScreen.screens.first { ScreenCoordinateSpace.displayID(for: $0) == snapshot.displayID })
            XCTAssertEqual(snapshot.screenFrame, screen.frame)
            let expected = ScreenCoordinateSpace.displayPixelSize(for: snapshot.displayID, fallbackScreen: screen)
            XCTAssertEqual(CGFloat(snapshot.image.width), expected.width, accuracy: 2)
            XCTAssertEqual(CGFloat(snapshot.image.height), expected.height, accuracy: 2)
        }
    }

    private func makeController() -> CaptureOverlayController {
        let suiteName = "BelloBoxTests.FrozenScreen.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return CaptureOverlayController(
            screenCaptureService: ScreenCaptureService(),
            settings: AppSettings(defaults: defaults),
            macOCRService: MacVisionOCRService()
        )
    }
}
