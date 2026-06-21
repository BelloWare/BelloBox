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

    func testReverseDragSelectsSameCocoaRect() {
        let selection = CaptureSelectionResolver.resolve(
            startLocal: CGPoint(x: 400, y: 280),
            endLocal: CGPoint(x: 100, y: 80),
            hoveredWindow: nil,
            screenFrame: screen,
            displayID: displayID
        )

        XCTAssertEqual(selection, .area(CaptureArea(
            cocoaRect: CGRect(x: 100, y: 620, width: 300, height: 200),
            displayID: displayID
        )))
    }

    func testDragOutsideScreenClampsToDisplayBounds() {
        let selection = CaptureSelectionResolver.resolve(
            startLocal: CGPoint(x: 100, y: 80),
            endLocal: CGPoint(x: 1600, y: -30),
            hoveredWindow: nil,
            screenFrame: screen,
            displayID: displayID
        )

        XCTAssertEqual(selection, .area(CaptureArea(
            cocoaRect: CGRect(x: 100, y: 820, width: 1340, height: 80),
            displayID: displayID
        )))
    }

    func testReverseDragOutsideScreenClampsToDisplayBounds() {
        let selection = CaptureSelectionResolver.resolve(
            startLocal: CGPoint(x: 1500, y: 930),
            endLocal: CGPoint(x: -20, y: 300),
            hoveredWindow: nil,
            screenFrame: screen,
            displayID: displayID
        )

        XCTAssertEqual(selection, .area(CaptureArea(
            cocoaRect: CGRect(x: 0, y: 0, width: 1440, height: 600),
            displayID: displayID
        )))
    }

    func testDragOnDisplayAbovePrimaryUsesThatDisplayFrame() {
        let upperScreen = CGRect(x: 0, y: 900, width: 1440, height: 900)

        let selection = CaptureSelectionResolver.resolve(
            startLocal: CGPoint(x: 50, y: 100),
            endLocal: CGPoint(x: 250, y: 300),
            hoveredWindow: nil,
            screenFrame: upperScreen,
            displayID: displayID
        )

        XCTAssertEqual(selection, .area(CaptureArea(
            cocoaRect: CGRect(x: 50, y: 1500, width: 200, height: 200),
            displayID: displayID
        )))
    }

    func testDragOnDisplayBelowPrimaryUsesThatDisplayFrame() {
        let lowerScreen = CGRect(x: 0, y: -900, width: 1440, height: 900)

        let selection = CaptureSelectionResolver.resolve(
            startLocal: CGPoint(x: 50, y: 100),
            endLocal: CGPoint(x: 250, y: 300),
            hoveredWindow: nil,
            screenFrame: lowerScreen,
            displayID: displayID
        )

        XCTAssertEqual(selection, .area(CaptureArea(
            cocoaRect: CGRect(x: 50, y: -300, width: 200, height: 200),
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

    func testTinyDragOnBlankScreenStillSelectsDisplay() {
        let selection = CaptureSelectionResolver.resolve(
            startLocal: CGPoint(x: 200, y: 200),
            endLocal: CGPoint(x: 207, y: 207),
            hoveredWindow: nil,
            screenFrame: screen,
            displayID: displayID
        )

        XCTAssertEqual(selection, .display(CaptureDisplay(displayID: displayID, frame: screen)))
    }

    func testTinyDragOnHoveredWindowStillSelectsWindow() {
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
            endLocal: CGPoint(x: 127, y: 177),
            hoveredWindow: window,
            screenFrame: screen,
            displayID: displayID
        )

        XCTAssertEqual(selection, .window(window))
    }

    func testSkinnyDragReturnsNilInsteadOfArea() {
        let selection = CaptureSelectionResolver.resolve(
            startLocal: CGPoint(x: 200, y: 200),
            endLocal: CGPoint(x: 360, y: 207),
            hoveredWindow: nil,
            screenFrame: screen,
            displayID: displayID
        )

        XCTAssertNil(selection)
    }
}
