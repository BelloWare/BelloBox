import CoreGraphics
import XCTest
@testable import BelloBox

final class CaptureSelectionResolverTests: XCTestCase {
    private let screen = CGRect(x: 0, y: 0, width: 1440, height: 900)
    private let displayID = CGDirectDisplayID(42)

    func testClickOnHoveredWindowSelectsWindow() {
        let window = CaptureWindow(
            windowID: 7,
            title: "Editor",
            ownerName: "Code",
            ownerBundleID: nil,
            ownerProcessID: nil,
            frame: CGRect(x: 100, y: 500, width: 400, height: 250)
        )

        let selection = CaptureSelectionResolver.resolve(
            startLocal: CGPoint(x: 120, y: 170),
            endLocal: CGPoint(x: 122, y: 172),
            hoveredWindow: window,
            screenFrame: screen,
            displayID: displayID
        )

        XCTAssertEqual(selection, .window(window))
    }

    func testDragSelectsAreaWithCorrectCocoaRect() {
        let selection = CaptureSelectionResolver.resolve(
            startLocal: CGPoint(x: 100, y: 80),
            endLocal: CGPoint(x: 400, y: 280),
            hoveredWindow: nil,
            screenFrame: screen,
            displayID: displayID
        )

        XCTAssertEqual(selection, .area(CaptureArea(
            cocoaRect: CGRect(x: 100, y: 620, width: 300, height: 200),
            displayID: displayID
        )))
    }

    func testClickOnBlankScreenSelectsDisplay() {
        let selection = CaptureSelectionResolver.resolve(
            startLocal: CGPoint(x: 200, y: 200),
            endLocal: CGPoint(x: 202, y: 202),
            hoveredWindow: nil,
            screenFrame: screen,
            displayID: displayID
        )

        XCTAssertEqual(selection, .display(CaptureDisplay(displayID: displayID, frame: screen)))
    }
}
