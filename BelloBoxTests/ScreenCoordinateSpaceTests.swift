import XCTest
@testable import BelloBox

final class ScreenCoordinateSpaceTests: XCTestCase {
    func testCocoaToDisplayPixelsOnOneXScreen() {
        let screen = CGRect(x: 0, y: 0, width: 1000, height: 800)
        let rect = CGRect(x: 100, y: 200, width: 300, height: 120)
        let pixels = ScreenCoordinateSpace.cocoaRectToDisplayPixelRect(rect, screenFrame: screen, scale: 1)
        XCTAssertEqual(pixels, CGRect(x: 100, y: 480, width: 300, height: 120))
    }

    func testCocoaToDisplayPixelsOnRetinaScreen() {
        let screen = CGRect(x: 0, y: 0, width: 1000, height: 800)
        let rect = CGRect(x: 100, y: 200, width: 300, height: 120)
        let pixels = ScreenCoordinateSpace.cocoaRectToDisplayPixelRect(rect, screenFrame: screen, scale: 2)
        XCTAssertEqual(pixels, CGRect(x: 200, y: 960, width: 600, height: 240))
    }

    func testSecondaryDisplayWithNegativeOriginRoundTrip() {
        let screen = CGRect(x: -1200, y: 80, width: 1200, height: 700)
        let rect = CGRect(x: -1000, y: 200, width: 400, height: 180)
        let pixels = ScreenCoordinateSpace.cocoaRectToDisplayPixelRect(rect, screenFrame: screen, scale: 2)
        let roundTrip = ScreenCoordinateSpace.displayPixelRectToCocoaRect(pixels, screenFrame: screen, scale: 2)
        XCTAssertEqual(roundTrip.origin.x, rect.origin.x, accuracy: 0.5)
        XCTAssertEqual(roundTrip.origin.y, rect.origin.y, accuracy: 0.5)
        XCTAssertEqual(roundTrip.width, rect.width, accuracy: 0.5)
        XCTAssertEqual(roundTrip.height, rect.height, accuracy: 0.5)
    }
}

