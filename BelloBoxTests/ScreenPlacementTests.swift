import XCTest
@testable import BelloBox

final class ScreenPlacementTests: XCTestCase {
    func testClampKeepsPopupInsideVisibleFrameWhenItFits() {
        let origin = ScreenPlacement.clamp(
            origin: CGPoint(x: 500, y: -100),
            size: CGSize(width: 240, height: 160),
            visibleFrame: CGRect(x: 0, y: 0, width: 800, height: 600)
        )

        XCTAssertEqual(origin, CGPoint(x: 500, y: 6))
    }

    func testClampAnchorsOversizedPopupAtVisibleInset() {
        let origin = ScreenPlacement.clamp(
            origin: CGPoint(x: 500, y: 500),
            size: CGSize(width: 900, height: 700),
            visibleFrame: CGRect(x: 0, y: 0, width: 800, height: 600)
        )

        XCTAssertEqual(origin, CGPoint(x: 6, y: 6))
    }

    func testClampKeepsPopupInsideNegativeOriginVisibleFrame() {
        let origin = ScreenPlacement.clamp(
            origin: CGPoint(x: -1700, y: 500),
            size: CGSize(width: 300, height: 180),
            visibleFrame: CGRect(x: -1600, y: 0, width: 1600, height: 900)
        )

        XCTAssertEqual(origin, CGPoint(x: -1594, y: 500))
    }
}
