import XCTest
@testable import BelloBox

final class OCRTileSegmenterTests: XCTestCase {
    func testTallImageSplitsIntoOverlappingTiles() {
        let image = ScreenshotTestHelpers.image(width: 100, height: 1000)
        let tiles = OCRTileSegmenter.tiles(from: image, maxTileHeight: 400, overlap: 100)
        XCTAssertEqual(tiles.map(\.yOffset), [0, 300, 600])
    }

    func testDuplicateOverlapLinesAreRemoved() {
        let regions = [
            OCRTextRegion(kind: .line, text: "same", boundingBox: CGRectCodable(CGRect(x: 0, y: 100, width: 100, height: 20))),
            OCRTextRegion(kind: .line, text: "same", boundingBox: CGRectCodable(CGRect(x: 0, y: 105, width: 100, height: 20))),
            OCRTextRegion(kind: .line, text: "other", boundingBox: CGRectCodable(CGRect(x: 0, y: 200, width: 100, height: 20))),
        ]
        XCTAssertEqual(OCRTileSegmenter.deduplicateOverlapRegions(regions).map(\.text), ["same", "other"])
    }
}

