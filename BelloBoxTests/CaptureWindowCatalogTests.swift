import AppKit
import CoreGraphics
import XCTest
@testable import BelloBox

final class CaptureWindowCatalogTests: XCTestCase {
    private let ownPID = pid_t(100)
    private let otherPID = pid_t(200)
    private let screenFrames = [CGRect(x: 0, y: 0, width: 1440, height: 900)]

    func testLayerZeroWindowUsesIndependentWindowCapture() {
        let windows = CaptureWindowCatalog.windows(
            from: [
                entry(
                    id: 7,
                    ownerPID: otherPID,
                    layer: CGWindowLevelForKey(.normalWindow),
                    bounds: CGRect(x: 100, y: 120, width: 500, height: 320),
                    title: "Editor",
                    ownerName: "Code"
                )
            ],
            ownPID: ownPID,
            screenFrames: screenFrames
        )

        XCTAssertEqual(windows.count, 1)
        XCTAssertEqual(windows[0].captureMode, .independentWindow)
        XCTAssertEqual(windows[0].layer, Int(CGWindowLevelForKey(.normalWindow)))
        XCTAssertFalse(windows[0].allowsVisibleFrameFallback)
        XCTAssertEqual(windows[0].frame, CGRect(x: 100, y: 460, width: 500, height: 320))
    }

    func testMenuAndStatusBarLayersUseVisibleFrameCapture() {
        let windows = CaptureWindowCatalog.windows(
            from: [
                entry(
                    id: 8,
                    ownerPID: otherPID,
                    layer: CGWindowLevelForKey(.mainMenuWindow),
                    bounds: CGRect(x: 0, y: 0, width: 1440, height: 24),
                    title: "Menu Bar",
                    ownerName: "SystemUIServer"
                ),
                entry(
                    id: 9,
                    ownerPID: otherPID,
                    layer: CGWindowLevelForKey(.statusWindow),
                    bounds: CGRect(x: 1310, y: 3, width: 34, height: 18),
                    title: "Status Item",
                    ownerName: "SystemUIServer"
                ),
            ],
            ownPID: ownPID,
            screenFrames: screenFrames
        )

        XCTAssertEqual(windows.map(\.windowID), [8, 9])
        XCTAssertTrue(windows.allSatisfy { $0.captureMode == .visibleFrame })
        XCTAssertTrue(windows.allSatisfy { $0.allowsVisibleFrameFallback })
        XCTAssertEqual(windows[0].frame, CGRect(x: 0, y: 876, width: 1440, height: 24))
        XCTAssertEqual(windows[1].frame, CGRect(x: 1310, y: 879, width: 34, height: 18))
    }

    func testPopupMenuLayerIsSelectableButUnrelatedHighLayerIsIgnored() {
        let windows = CaptureWindowCatalog.windows(
            from: [
                entry(
                    id: 10,
                    ownerPID: otherPID,
                    layer: CGWindowLevelForKey(.popUpMenuWindow),
                    bounds: CGRect(x: 960, y: 40, width: 220, height: 260),
                    title: "Menu",
                    ownerName: "SystemUIServer"
                ),
                entry(
                    id: 11,
                    ownerPID: otherPID,
                    layer: CGWindowLevelForKey(.screenSaverWindow),
                    bounds: CGRect(x: 0, y: 0, width: 1440, height: 900),
                    title: "Overlay",
                    ownerName: "Other"
                ),
            ],
            ownPID: ownPID,
            screenFrames: screenFrames
        )

        XCTAssertEqual(windows.map(\.windowID), [10])
        XCTAssertEqual(windows[0].captureMode, .visibleFrame)
    }

    func testOwnAndTinyWindowsAreIgnored() {
        let windows = CaptureWindowCatalog.windows(
            from: [
                entry(
                    id: 12,
                    ownerPID: ownPID,
                    layer: CGWindowLevelForKey(.normalWindow),
                    bounds: CGRect(x: 100, y: 100, width: 400, height: 300),
                    title: "Bello Box",
                    ownerName: "Bello Box"
                ),
                entry(
                    id: 13,
                    ownerPID: otherPID,
                    layer: CGWindowLevelForKey(.statusWindow),
                    bounds: CGRect(x: 1200, y: 3, width: 7, height: 18),
                    title: "Tiny",
                    ownerName: "SystemUIServer"
                ),
            ],
            ownPID: ownPID,
            screenFrames: screenFrames
        )

        XCTAssertTrue(windows.isEmpty)
    }

    func testFullScreenWindowAllowsVisibleFrameFallback() {
        let windows = CaptureWindowCatalog.windows(
            from: [
                entry(
                    id: 14,
                    ownerPID: otherPID,
                    layer: CGWindowLevelForKey(.normalWindow),
                    bounds: CGRect(x: 0, y: 0, width: 1440, height: 900),
                    title: "Full Screen",
                    ownerName: "Preview"
                )
            ],
            ownPID: ownPID,
            screenFrames: screenFrames
        )

        XCTAssertEqual(windows.count, 1)
        XCTAssertEqual(windows[0].captureMode, .independentWindow)
        XCTAssertTrue(windows[0].allowsVisibleFrameFallback)
    }

    private func entry(
        id: UInt32,
        ownerPID: pid_t,
        layer: Int32,
        bounds: CGRect,
        title: String,
        ownerName: String,
        alpha: Double = 1
    ) -> [String: Any] {
        [
            kCGWindowNumber as String: NSNumber(value: id),
            kCGWindowOwnerPID as String: NSNumber(value: ownerPID),
            kCGWindowLayer as String: NSNumber(value: layer),
            kCGWindowAlpha as String: NSNumber(value: alpha),
            kCGWindowBounds as String: bounds.dictionaryRepresentation ?? [:],
            kCGWindowName as String: title,
            kCGWindowOwnerName as String: ownerName,
        ]
    }
}
