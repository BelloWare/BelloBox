import AppKit
import XCTest
@testable import BelloBox

@MainActor
final class CaptureOverlayFrozenScreenTests: XCTestCase {
    func testSelectionAndEditorKeepTheSameDimLayersVisible() throws {
        let controller = makeController()
        defer { controller.cancel() }
        let screen = try XCTUnwrap(NSScreen.main)
        let id = try XCTUnwrap(ScreenCoordinateSpace.displayID(for: screen))
        let snapshot = DisplaySnapshot(displayID: id, screenFrame: screen.frame, scale: 1,
            image: ScreenshotTestHelpers.image(width: Int(screen.frame.width), height: Int(screen.frame.height)))
        controller.beginScreenshotForTesting(snapshots: [snapshot], policy: .areaOnly, onError: { XCTFail($0) })
        let window = try XCTUnwrap(controller.debugOverlayWindows.first { $0.frame == screen.frame })
        let view = try XCTUnwrap(window.contentView)
        let dimming = try XCTUnwrap(view.subviews.first as? CaptureDimmingView)
        let start = CGPoint(x: 100, y: 100)
        let end = CGPoint(x: 350, y: 280)
        for (type, point) in [(NSEvent.EventType.leftMouseDown, start), (.leftMouseDragged, end), (.leftMouseUp, end)] {
            let event = try XCTUnwrap(NSEvent.mouseEvent(with: type,
                location: view.convert(point, to: nil), modifierFlags: [], timestamp: 0,
                windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
            switch type {
            case .leftMouseDown: view.mouseDown(with: event)
            case .leftMouseDragged: view.mouseDragged(with: event)
            default: view.mouseUp(with: event)
            }
        }
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        XCTAssertEqual(controller.debugOverlayOrderOutCount, 0)
        XCTAssertTrue(window.isVisible)
        XCTAssertTrue(view.subviews.first === dimming)
        XCTAssertTrue(dimming.debugDimLayers.contains { !$0.isHidden && !$0.frame.isEmpty })
        XCTAssertNotNil(controller.debugActiveScreenshotViewModel)
    }

    func testFrozenSnapshotsCombineWindowAcrossMixedScaleDisplays() throws {
        let red = ScreenshotTestHelpers.image(width: 100, height: 100) { context in
            context.setFillColor(NSColor.red.cgColor); context.fill(CGRect(x: 0, y: 0, width: 100, height: 100))
        }
        let blue = ScreenshotTestHelpers.image(width: 200, height: 200) { context in
            context.setFillColor(NSColor.blue.cgColor); context.fill(CGRect(x: 0, y: 0, width: 200, height: 200))
        }
        let snapshots = [
            DisplaySnapshot(displayID: 1, screenFrame: CGRect(x: -100, y: 0, width: 100, height: 100), scale: 1, image: red),
            DisplaySnapshot(displayID: 2, screenFrame: CGRect(x: 0, y: 0, width: 100, height: 100), scale: 2, image: blue)
        ]
        let document = try ScreenCaptureService().document(fromSnapshots: snapshots,
            cocoaRect: CGRect(x: -40, y: 20, width: 80, height: 50), source: .importedClipboard)
        XCTAssertEqual(document.imageSize, CGSize(width: 160, height: 100))
        XCTAssertEqual(document.scale, 2)
        XCTAssertEqual(ScreenshotTestHelpers.pixel(document.baseImage, x: 20, y: 30), [255, 0, 0, 255])
        XCTAssertEqual(ScreenshotTestHelpers.pixel(document.baseImage, x: 120, y: 30), [0, 0, 255, 255])
    }

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
