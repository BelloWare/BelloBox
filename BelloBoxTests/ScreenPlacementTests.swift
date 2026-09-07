import XCTest
@testable import BelloBox

final class ScreenPlacementTests: XCTestCase {
    func testLegacyScrollbarsDoNotClipTheOtherEdgeOfATool() {
        let both = ToolViewportLayout(minimum: CGSize(width: 500, height: 500),
                                      available: CGSize(width: 500, height: 480), scrollerWidth: 15)
        XCTAssertTrue(both.axes.contains(.horizontal))
        XCTAssertTrue(both.axes.contains(.vertical))
        XCTAssertEqual(both.contentSize, CGSize(width: 500, height: 500))
        let vertical = ToolViewportLayout(minimum: CGSize(width: 480, height: 500),
                                          available: CGSize(width: 500, height: 480), scrollerWidth: 15)
        XCTAssertFalse(vertical.axes.contains(.horizontal))
        XCTAssertTrue(vertical.axes.contains(.vertical))
        XCTAssertEqual(vertical.contentSize.width, 485)
        let normal = ToolViewportLayout(minimum: CGSize(width: 500, height: 500),
                                        available: CGSize(width: 720, height: 760), scrollerWidth: 15)
        XCTAssertTrue(normal.axes.isEmpty)
        XCTAssertEqual(normal.contentSize, CGSize(width: 720, height: 760))
    }

    func testAllPopupRoutesFitAndEmptySelectionCenters() {
        let visible = CGRect(x: -1024, y: -620, width: 1024, height: 600)
        for size in [CGSize(width: 1040, height: 760), CGSize(width: 520, height: 620),
                     CGSize(width: 720, height: 760), CGSize(width: 560, height: 600)] {
            let centered = ScreenPlacement.popupFrame(size: size, anchorRect: nil, visibleFrame: visible)
            XCTAssertEqual(centered.midX, visible.midX)
            XCTAssertEqual(centered.midY, visible.midY)
            for anchor in [CGRect(x: -1020, y: -615, width: 60, height: 20),
                           CGRect(x: -90, y: -45, width: 70, height: 20)] {
                let frame = ScreenPlacement.popupFrame(size: size, anchorRect: anchor, visibleFrame: visible)
                XCTAssertTrue(visible.insetBy(dx: 6, dy: 6).contains(frame))
                XCTAssertEqual(frame.size, centered.size)
            }
        }
    }

    func testSelectionPopupKeepsItsUsualBelowOrAbovePlacementWhenSpaceAllows() {
        let visible = CGRect(x: 0, y: 0, width: 1200, height: 900)
        let size = CGSize(width: 460, height: 380)
        let below = ScreenPlacement.popupFrame(size: size, anchorRect: CGRect(x: 100, y: 700, width: 80, height: 20), visibleFrame: visible)
        XCTAssertEqual(below, CGRect(x: 100, y: 308, width: 460, height: 380))
        let above = ScreenPlacement.popupFrame(size: size, anchorRect: CGRect(x: 100, y: 100, width: 80, height: 20), visibleFrame: visible)
        XCTAssertEqual(above, CGRect(x: 100, y: 132, width: 460, height: 380))
    }

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
