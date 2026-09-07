import XCTest
@testable import BelloBox

final class ScreenPlacementTests: XCTestCase {
    func testCenteredPopupUsesTheVisibleFrameOnAnyDisplayAndFitsSmallScreens() {
        for visible in [CGRect(x: 0, y: 48, width: 1728, height: 919),
                        CGRect(x: -1920, y: 100, width: 1920, height: 1000),
                        CGRect(x: 200, y: -900, width: 640, height: 580)] {
            let frame = ScreenPlacement.centeredFrame(size: CGSize(width: 720, height: 760), visibleFrame: visible)
            XCTAssertEqual(frame.midX, visible.midX)
            XCTAssertEqual(frame.midY, visible.midY)
            XCTAssertTrue(visible.insetBy(dx: 6, dy: 6).contains(frame))
        }
    }

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
