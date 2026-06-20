import XCTest
@testable import BelloBox

final class OCRBoundingBoxConversionTests: XCTestCase {
    func testVisionNormalizedBoxConvertsToTopLeftImagePixels() {
        let rect = OCRBoundingBoxConverter.imagePixelRect(
            fromVisionNormalizedBox: CGRect(x: 0.25, y: 0.2, width: 0.5, height: 0.3),
            imageSize: CGSize(width: 400, height: 200)
        )
        XCTAssertEqual(rect, CGRect(x: 100, y: 100, width: 200, height: 60))
    }
}

