import XCTest
@testable import BelloBox

final class OCRBoundingBoxConverterTests: XCTestCase {
    func testVisionNormalizedBoxConvertsToTopLeftImagePixels() {
        let rect = OCRBoundingBoxConverter.imagePixelRect(
            fromVisionNormalizedBox: CGRect(x: 0.25, y: 0.10, width: 0.50, height: 0.20),
            imageSize: CGSize(width: 400, height: 300)
        )

        XCTAssertEqual(rect, CGRect(x: 100, y: 210, width: 200, height: 60))
    }

    func testRegionsSortByReadingOrder() {
        let bottomLeft = OCRTextRegion(
            kind: .line,
            text: "bottom left",
            boundingBox: CGRectCodable(CGRect(x: 10, y: 90, width: 80, height: 20))
        )
        let topRight = OCRTextRegion(
            kind: .line,
            text: "top right",
            boundingBox: CGRectCodable(CGRect(x: 120, y: 10, width: 80, height: 20))
        )
        let topLeft = OCRTextRegion(
            kind: .line,
            text: "top left",
            boundingBox: CGRectCodable(CGRect(x: 10, y: 12, width: 80, height: 20))
        )

        let sorted = [bottomLeft, topRight, topLeft].sortedByReadingOrder()

        XCTAssertEqual(sorted.map(\.text), ["top left", "top right", "bottom left"])
    }

    func testRegionsSortByReadingOrderBucketsAmbiguousRowsDeterministically() {
        let topRight = OCRTextRegion(
            kind: .line,
            text: "top right",
            boundingBox: CGRectCodable(CGRect(x: 80, y: 0, width: 50, height: 20))
        )
        let topLeftLower = OCRTextRegion(
            kind: .line,
            text: "top left lower",
            boundingBox: CGRectCodable(CGRect(x: 10, y: 8, width: 50, height: 20))
        )
        let nextLineLeft = OCRTextRegion(
            kind: .line,
            text: "next line",
            boundingBox: CGRectCodable(CGRect(x: 10, y: 16, width: 50, height: 20))
        )

        let sorted = [nextLineLeft, topRight, topLeftLower].sortedByReadingOrder()

        XCTAssertEqual(sorted.map(\.text), ["top left lower", "top right", "next line"])
    }
}
