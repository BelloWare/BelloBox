import AppKit
import SwiftUI
import XCTest
@testable import BelloBox

@MainActor
final class CaptureDimmingViewTests: XCTestCase {
    func testFullDimWhenThereIsNoSelection() {
        let view = CaptureDimmingView(frame: CGRect(x: 0, y: 0, width: 800, height: 500), contentsScale: 2)
        view.update(selection: nil)

        XCTAssertNil(view.selectionFrame)
        XCTAssertEqual(view.dimBandFrames.first, view.bounds)
    }

    func testSelectionCutsAHoleInTheDim() {
        let view = CaptureDimmingView(frame: CGRect(x: 0, y: 0, width: 800, height: 500), contentsScale: 2)
        let flipped = CGRect(x: 100, y: 50, width: 200, height: 100)
        let selection = CaptureDimmingView.flip(flipped, height: view.bounds.height)
        view.update(selection: selection, borderWidth: 2, label: "400 × 200")

        XCTAssertEqual(view.selectionFrame, CGRect(x: 100, y: 350, width: 200, height: 100))
        XCTAssertEqual(CaptureDimmingView.flip(view.selectionFrame!, height: view.bounds.height), flipped)
        let covered = view.dimBandFrames.reduce(CGFloat(0)) { $0 + $1.width * $1.height }
        XCTAssertEqual(covered, 800 * 500 - 200 * 100, accuracy: 0.001)
        XCTAssertTrue(view.dimBandFrames.allSatisfy { !$0.intersects(view.selectionFrame!.insetBy(dx: 0.5, dy: 0.5)) })
    }

    func testDimmingViewNeverInterceptsMouseEvents() {
        let view = CaptureDimmingView(frame: CGRect(x: 0, y: 0, width: 300, height: 200), contentsScale: 1)
        XCTAssertNil(view.hitTest(CGPoint(x: 10, y: 10)))
    }

    func testCaptureOverlayDimsEveryDisplayBeforeSelection() throws {
        let controller = CaptureOverlayController(
            screenCaptureService: ScreenCaptureService(),
            settings: AppSettings(defaults: temporaryDefaults()),
            macOCRService: MacVisionOCRService()
        )
        defer { controller.cancel() }

        // Area-only capture never highlights the window under the mouse, so the whole
        // display must be dimmed before the first drag.
        controller.beginScreenshotForTesting(snapshots: [], policy: .areaOnly, onError: { XCTFail($0) })

        XCTAssertEqual(controller.debugDimBandFrames.count, NSScreen.screens.count)
        for (bands, window) in zip(controller.debugDimBandFrames, controller.debugOverlayWindows) {
            XCTAssertEqual(bands.first?.size, window.frame.size)
        }
        XCTAssertTrue(controller.debugSelectionFrames.allSatisfy { $0 == nil })

        // The dim must really be attached and visible in each window's view tree, not
        // just modelled: a layer-hosting CaptureDimmingView with a visible full-size
        // dim layer at the bottom of the overlay view.
        for window in controller.debugOverlayWindows {
            let overlay = try XCTUnwrap(window.contentView)
            let dimming = try XCTUnwrap(overlay.subviews.compactMap { $0 as? CaptureDimmingView }.first)
            XCTAssertTrue(overlay.subviews.first === dimming, "dimming chrome must sit below any editor views")
            XCTAssertEqual(dimming.frame, overlay.bounds)
            let root = try XCTUnwrap(dimming.layer)
            let dimLayer = try XCTUnwrap(dimming.debugDimLayers.first)
            XCTAssertTrue(root.sublayers?.contains(where: { $0 === dimLayer }) == true)
            XCTAssertFalse(dimLayer.isHidden)
            XCTAssertEqual(dimLayer.frame, dimming.bounds)
            XCTAssertEqual(dimLayer.backgroundColor, CaptureDimmingView.dimColor.cgColor)
            XCTAssertTrue(dimming.debugDimLayers.dropFirst().allSatisfy(\.isHidden))
        }
    }

    func testLiveSelectionCutsSnapshotAwayAndKeepsBorderOutside() {
        let view = CaptureDimmingView(frame: CGRect(x: 0, y: 0, width: 800, height: 500), contentsScale: 2)
        view.snapshotImage = ScreenshotTestHelpers.image(width: 80, height: 50)
        let selection = CGRect(x: 100, y: 100, width: 200, height: 100)

        view.update(selection: selection, borderWidth: 2, label: "x", showsLiveContentInSelection: true)
        XCTAssertTrue(view.debugSnapshotIsMasked)
        XCTAssertTrue(view.showsLiveContentInSelection)
        XCTAssertEqual(view.selectionFrame, selection)

        view.update(selection: selection, borderWidth: 2, label: nil, showsLiveContentInSelection: false)
        XCTAssertFalse(view.debugSnapshotIsMasked)
        XCTAssertFalse(view.showsLiveContentInSelection)
    }

    func testRedactionPreviewIsFullyOpaque() throws {
        let image = try ScreenshotTestHelpers.annotationPreview(
            annotations: [ScreenshotAnnotation(kind: .blur(CGRect(x: 0, y: 0, width: 40, height: 40)), style: .redaction)],
            imageSize: CGSize(width: 40, height: 40), viewSize: CGSize(width: 40, height: 40)
        )
        XCTAssertEqual(image.width, 40)
        XCTAssertEqual(image.height, 40)
        let expected = UInt8((AnnotationStyle.redactionFillColor.red * 255).rounded())
        for (x, y) in [(2, 2), (20, 20), (37, 37), (5, 30), (30, 5)] {
            let pixel = ScreenshotTestHelpers.pixel(image, x: x, y: y)
            XCTAssertEqual(pixel[3], 255, "preview mask must be opaque at (\(x), \(y))")
            XCTAssertEqual(pixel[0], pixel[1], "preview mask must be neutral grey at (\(x), \(y))")
            XCTAssertEqual(pixel[1], pixel[2], "preview mask must be neutral grey at (\(x), \(y))")
            XCTAssertGreaterThanOrEqual(pixel[0], expected - 1)
            XCTAssertLessThan(pixel[0], 90, "preview mask must stay dark at (\(x), \(y))")
        }
        XCTAssertEqual(AnnotationStyle.redactionFillColor.alpha, 1)
    }

    func testLockedSelectionOpensHoleOnlyOnItsDisplay() throws {
        let controller = CaptureOverlayController(
            screenCaptureService: ScreenCaptureService(),
            settings: AppSettings(defaults: temporaryDefaults()),
            macOCRService: MacVisionOCRService()
        )
        defer { controller.cancel() }
        let screen = try XCTUnwrap(NSScreen.main)
        let displayID = try XCTUnwrap(ScreenCoordinateSpace.displayID(for: screen))
        let snapshot = DisplaySnapshot(
            displayID: displayID,
            screenFrame: screen.frame,
            scale: 1,
            image: ScreenshotTestHelpers.image(width: Int(screen.frame.width), height: Int(screen.frame.height))
        )
        let rect = CGRect(x: screen.frame.minX + 100, y: screen.frame.minY + 120, width: 240, height: 160)

        controller.beginScreenshotForTesting(
            snapshots: [snapshot],
            initialSelection: .area(CaptureArea(cocoaRect: rect, displayID: displayID)),
            onError: { XCTFail($0) }
        )
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))

        let frames = controller.debugSelectionFrames.compactMap { $0 }
        XCTAssertEqual(frames.count, 1)
        let expected = RegionCaptureGeometry.globalCocoaRectToLocalFlipped(rect, screenFrame: screen.frame)
        XCTAssertEqual(frames.first?.origin.x ?? -1, expected.origin.x, accuracy: 1)
        XCTAssertEqual(frames.first?.origin.y ?? -1, expected.origin.y, accuracy: 1)
        XCTAssertEqual(frames.first?.width ?? -1, expected.width, accuracy: 1)
        XCTAssertEqual(frames.first?.height ?? -1, expected.height, accuracy: 1)

        let viewModel = try XCTUnwrap(controller.debugActiveScreenshotViewModel)
        XCTAssertTrue(viewModel.supportsSelectionAdjustment)
        XCTAssertEqual(viewModel.visibleImageSize, CGSize(width: 240, height: 160))

        // Resizing through the view model moves the dim cut-out with it.
        viewModel.setSelectionCropRect(CGRect(x: 100, y: 100, width: 300, height: 200))
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        let resized = try XCTUnwrap(controller.debugSelectionFrames.compactMap { $0 }.first)
        XCTAssertEqual(resized.width, 300, accuracy: 1)
        XCTAssertEqual(resized.height, 200, accuracy: 1)
    }

    private func temporaryDefaults() -> UserDefaults {
        let suiteName = "BelloBoxTests.CaptureDimming.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
