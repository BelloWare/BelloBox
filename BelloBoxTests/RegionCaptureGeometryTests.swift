import XCTest
@testable import BelloBox

final class RegionCaptureGeometryTests: XCTestCase {
    func testClampedSelectionRectKeepsDragInsideBounds() {
        let bounds = CGRect(x: 0, y: 0, width: 300, height: 200)

        let rect = RegionCaptureGeometry.clampedSelectionRect(
            from: CGPoint(x: 40, y: 30),
            to: CGPoint(x: 360, y: -20),
            bounds: bounds
        )

        XCTAssertEqual(rect, CGRect(x: 40, y: 0, width: 260, height: 30))
    }

    func testClampedSelectionRectHandlesReverseDragOutsideBounds() {
        let bounds = CGRect(x: 0, y: 0, width: 300, height: 200)

        let rect = RegionCaptureGeometry.clampedSelectionRect(
            from: CGPoint(x: 320, y: 260),
            to: CGPoint(x: -12, y: 70),
            bounds: bounds
        )

        XCTAssertEqual(rect, CGRect(x: 0, y: 70, width: 300, height: 130))
    }

    func testLocalFlippedRectConvertsToGlobalCocoaCoordinates() {
        let screenFrame = CGRect(x: 0, y: 0, width: 1440, height: 900)
        let local = CGRect(x: 100, y: 80, width: 300, height: 200)

        let cocoa = RegionCaptureGeometry.localFlippedRectToGlobalCocoa(local, screenFrame: screenFrame)

        XCTAssertEqual(cocoa, CGRect(x: 100, y: 620, width: 300, height: 200))
    }

    func testGlobalCocoaWindowRectConvertsToLocalFlippedCoordinates() {
        let screenFrame = CGRect(x: 0, y: 0, width: 1440, height: 900)
        let window = CGRect(x: 120, y: 500, width: 360, height: 240)

        let local = RegionCaptureGeometry.globalCocoaRectToLocalFlipped(window, screenFrame: screenFrame)

        XCTAssertEqual(local, CGRect(x: 120, y: 160, width: 360, height: 240))
    }

    func testGlobalCocoaPointConvertsToLocalFlippedOnNegativeOriginDisplay() {
        let screenFrame = CGRect(x: -1280, y: 0, width: 1280, height: 800)
        let point = CGPoint(x: -1180, y: 680)

        let local = RegionCaptureGeometry.globalCocoaPointToLocalFlipped(point, screenFrame: screenFrame)

        XCTAssertEqual(local, CGPoint(x: 100, y: 120))
        XCTAssertEqual(RegionCaptureGeometry.localFlippedPointToGlobalCocoa(local, screenFrame: screenFrame), point)
    }

    func testGlobalCocoaPointConvertsToLocalFlippedOnDisplayAbovePrimary() {
        let screenFrame = CGRect(x: 0, y: 900, width: 1440, height: 900)
        let point = CGPoint(x: 50, y: 1700)

        let local = RegionCaptureGeometry.globalCocoaPointToLocalFlipped(point, screenFrame: screenFrame)

        XCTAssertEqual(local, CGPoint(x: 50, y: 100))
        XCTAssertEqual(RegionCaptureGeometry.localFlippedPointToGlobalCocoa(local, screenFrame: screenFrame), point)
    }
}
