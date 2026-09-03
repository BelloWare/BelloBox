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

    func testCaptureOverlayCanStartWithoutDisplaySnapshots() {
        let controller = makeController()
        defer { controller.cancel() }

        controller.beginScreenshotForTesting(
            snapshots: [],
            onError: { XCTFail("Unexpected capture overlay error: \($0)") },
            onCancel: {}
        )

        XCTAssertEqual(controller.debugOverlayWindowCount, NSScreen.screens.count)
        assertSystemOverlayPresentation(controller.debugOverlayWindows)
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

    func testNonactivatingCaptureOverlayStillHandlesEscape() throws {
        let controller = makeController()
        defer { controller.cancel() }
        let cancelled = expectation(description: "capture overlay cancelled by Escape")

        controller.beginScreenshotForTesting(
            snapshots: [],
            onError: { XCTFail("Unexpected capture overlay error: \($0)") },
            onCancel: { cancelled.fulfill() }
        )

        let keyWindow = try XCTUnwrap(controller.debugOverlayWindows.first(where: \.isKeyWindow))
        let event = try XCTUnwrap(
            NSEvent.keyEvent(
                with: .keyDown,
                location: .zero,
                modifierFlags: [],
                timestamp: 0,
                windowNumber: keyWindow.windowNumber,
                context: nil,
                characters: "\u{1B}",
                charactersIgnoringModifiers: "\u{1B}",
                isARepeat: false,
                keyCode: 53
            )
        )
        keyWindow.sendEvent(event)

        wait(for: [cancelled], timeout: 1)
        XCTAssertTrue(controller.debugOverlayWindows.isEmpty)
    }

    func testScrollHUDStaysOutsideTheSampledSelection() {
        let bounds = CGRect(x: 0, y: 0, width: 1000, height: 800)
        let size = CGSize(width: 510, height: 100) // 470 x 60 card plus a 20 pt transparent margin
        let padding: CGFloat = 20
        func visible(_ frame: CGRect) -> CGRect { frame.insetBy(dx: padding, dy: padding) }

        let selection = CGRect(x: 100, y: 100, width: 400, height: 300)
        let below = ScrollCaptureHUDLayout.frame(selection: selection, bounds: bounds, size: size, padding: padding)
        XCTAssertGreaterThanOrEqual(visible(below).minY, selection.maxY)
        XCTAssertFalse(visible(below).intersects(selection))

        let tall = CGRect(x: 100, y: 20, width: 400, height: 740) // no room above or below
        let side = ScrollCaptureHUDLayout.frame(selection: tall, bounds: bounds, size: size, padding: padding)
        XCTAssertFalse(visible(side).intersects(tall))
        XCTAssertGreaterThanOrEqual(visible(side).minX, tall.maxX)

        let wide = CGRect(x: 20, y: 20, width: 960, height: 740) // no room anywhere
        let inside = ScrollCaptureHUDLayout.frame(selection: wide, bounds: bounds, size: size, padding: padding)
        XCTAssertTrue(bounds.contains(visible(inside)))
        XCTAssertGreaterThan(visible(inside).minY, wide.midY)

        // The sampled region is then trimmed to stop above the card.
        let trimmed = ScrollCaptureHUDLayout.selectionAvoiding(hud: visible(inside), selection: wide, gap: 12, minimumHeight: 8)
        XCTAssertEqual(trimmed?.maxY, visible(inside).minY - 12)
        XCTAssertEqual(trimmed?.minY, wide.minY)
        XCTAssertFalse(trimmed!.intersects(visible(inside)))
        XCTAssertEqual(ScrollCaptureHUDLayout.selectionAvoiding(hud: visible(below), selection: selection, gap: 12, minimumHeight: 8), selection)
    }

    private func makeController() -> CaptureOverlayController {
        CaptureOverlayController(
            screenCaptureService: ScreenCaptureService(),
            settings: AppSettings(defaults: temporaryDefaults()),
            macOCRService: MacVisionOCRService()
        )
    }

    private func assertSystemOverlayPresentation(
        _ windows: [NSWindow],
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertFalse(windows.isEmpty, file: file, line: line)
        for window in windows {
            XCTAssertTrue(window.styleMask.contains(.nonactivatingPanel), file: file, line: line)
            XCTAssertEqual(window.level, .screenSaver, file: file, line: line)
            XCTAssertTrue(window.collectionBehavior.contains(.canJoinAllSpaces), file: file, line: line)
            XCTAssertTrue(window.collectionBehavior.contains(.canJoinAllApplications), file: file, line: line)
            XCTAssertTrue(window.collectionBehavior.contains(.fullScreenAuxiliary), file: file, line: line)
            XCTAssertTrue(window.collectionBehavior.contains(.stationary), file: file, line: line)
            XCTAssertTrue(window.collectionBehavior.contains(.ignoresCycle), file: file, line: line)
            XCTAssertTrue((window as? NSPanel)?.isFloatingPanel == true, file: file, line: line)
            XCTAssertTrue(window.isVisible, file: file, line: line)
            XCTAssertEqual(window.frame, window.screen?.frame, file: file, line: line)
        }
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
