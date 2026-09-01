import CoreGraphics
import XCTest
@testable import BelloBox

final class SelectionResizeGeometryTests: XCTestCase {
    private let bounds = CGRect(x: 0, y: 0, width: 1000, height: 600)
    private let start = CGRect(x: 200, y: 150, width: 300, height: 200)

    func testHandlePositionsSitOnTheSelectionEdges() {
        XCTAssertEqual(SelectionResizeGeometry.handlePosition(.topLeft, in: start), CGPoint(x: 200, y: 150))
        XCTAssertEqual(SelectionResizeGeometry.handlePosition(.top, in: start), CGPoint(x: 350, y: 150))
        XCTAssertEqual(SelectionResizeGeometry.handlePosition(.bottomRight, in: start), CGPoint(x: 500, y: 350))
        XCTAssertEqual(SelectionResizeGeometry.handlePosition(.left, in: start), CGPoint(x: 200, y: 250))
    }

    func testDraggingRightHandleOnlyMovesRightEdge() {
        let rect = SelectionResizeGeometry.resizedRect(
            from: start, handle: .right, translation: CGSize(width: 40, height: 99), bounds: bounds, minimumSize: 8
        )
        XCTAssertEqual(rect, CGRect(x: 200, y: 150, width: 340, height: 200))
    }

    func testDraggingTopLeftCornerMovesBothEdges() {
        let rect = SelectionResizeGeometry.resizedRect(
            from: start, handle: .topLeft, translation: CGSize(width: -50, height: -30), bounds: bounds, minimumSize: 8
        )
        XCTAssertEqual(rect, CGRect(x: 150, y: 120, width: 350, height: 230))
    }

    func testResizeIsClampedToBounds() {
        let rect = SelectionResizeGeometry.resizedRect(
            from: start, handle: .bottomRight, translation: CGSize(width: 5000, height: 5000), bounds: bounds, minimumSize: 8
        )
        XCTAssertEqual(rect, CGRect(x: 200, y: 150, width: 800, height: 450))
    }

    func testResizeNeverCollapsesBelowMinimumSize() {
        let rect = SelectionResizeGeometry.resizedRect(
            from: start, handle: .left, translation: CGSize(width: 900, height: 0), bounds: bounds, minimumSize: 8
        )
        XCTAssertEqual(rect.maxX, 500)
        XCTAssertEqual(rect.width, 8)

        let vertical = SelectionResizeGeometry.resizedRect(
            from: start, handle: .bottom, translation: CGSize(width: 0, height: -900), bounds: bounds, minimumSize: 8
        )
        XCTAssertEqual(vertical.minY, 150)
        XCTAssertEqual(vertical.height, 8)
    }

    func testMoveKeepsSizeAndStaysInsideBounds() {
        let moved = SelectionResizeGeometry.movedRect(from: start, translation: CGSize(width: 30, height: -20), bounds: bounds)
        XCTAssertEqual(moved, CGRect(x: 230, y: 130, width: 300, height: 200))

        let clampedMove = SelectionResizeGeometry.movedRect(from: start, translation: CGSize(width: 5000, height: -5000), bounds: bounds)
        XCTAssertEqual(clampedMove, CGRect(x: 700, y: 0, width: 300, height: 200))
    }

    func testClampedGrowsDegenerateRectsInPlace() {
        let imageBounds = CGRect(x: 0, y: 0, width: 400, height: 300)
        let tiny = SelectionResizeGeometry.clamped(
            CGRect(x: 395, y: 295, width: 1, height: 1), in: imageBounds, minimumSize: CGSize(width: 16, height: 16)
        )
        XCTAssertEqual(tiny, CGRect(x: 384, y: 284, width: 16, height: 16))

        let outside = SelectionResizeGeometry.clamped(
            CGRect(x: -50, y: -50, width: 100, height: 100), in: imageBounds, minimumSize: CGSize(width: 16, height: 16)
        )
        XCTAssertEqual(outside, CGRect(x: 0, y: 0, width: 50, height: 50))
    }

    func testDimBandsTileEverythingOutsideTheSelection() {
        let bands = CaptureOverlayDimGeometry.bands(bounds: bounds, selection: start)
        XCTAssertEqual(bands.count, 4)
        let coveredArea = bands.reduce(CGFloat(0)) { $0 + $1.width * $1.height }
        XCTAssertEqual(coveredArea, bounds.width * bounds.height - start.width * start.height, accuracy: 0.001)
        for band in bands where !band.isEmpty {
            XCTAssertTrue(bounds.contains(band))
            XCTAssertFalse(band.intersects(start.insetBy(dx: 0.5, dy: 0.5)))
        }
        for (index, lhs) in bands.enumerated() {
            for rhs in bands.dropFirst(index + 1) where !lhs.isEmpty && !rhs.isEmpty {
                XCTAssertTrue(lhs.intersection(rhs).isEmpty || lhs.intersection(rhs).width == 0 || lhs.intersection(rhs).height == 0)
            }
        }
    }

    func testDimBandsCoverEverythingWithoutASelection() {
        let bands = CaptureOverlayDimGeometry.bands(bounds: bounds, selection: nil)
        XCTAssertEqual(bands.first, bounds)
        XCTAssertTrue(bands.dropFirst().allSatisfy(\.isEmpty))

        let outside = CaptureOverlayDimGeometry.bands(bounds: bounds, selection: CGRect(x: 2000, y: 0, width: 10, height: 10))
        XCTAssertEqual(outside.first, bounds)
    }
}
