import CoreGraphics
import XCTest
@testable import BelloBox

@MainActor
final class SnapshotDocumentTests: XCTestCase {
    func testDocumentFromSnapshotCropsUsingCocoaCoordinates() throws {
        let image = ScreenshotTestHelpers.stripedImage(width: 100, height: 80)
        let snapshot = DisplaySnapshot(
            displayID: 1,
            screenFrame: CGRect(x: 0, y: 0, width: 100, height: 80),
            scale: 1,
            image: image
        )

        let document = try ScreenCaptureService().document(
            fromSnapshot: snapshot,
            cocoaRect: CGRect(x: 10, y: 20, width: 30, height: 15),
            source: .area(rect: CGRect(x: 10, y: 20, width: 30, height: 15), displayID: 1)
        )

        XCTAssertEqual(document.baseImage.width, 30)
        XCTAssertEqual(document.baseImage.height, 15)
        XCTAssertEqual(document.scale, 1)
    }
}
