import CoreGraphics
import XCTest
@testable import BelloBox

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
}
