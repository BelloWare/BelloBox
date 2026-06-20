import XCTest
@testable import BelloBox

final class RegionCaptureGeometryTests: XCTestCase {
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
}
