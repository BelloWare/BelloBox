import XCTest
@testable import BelloBox

final class ImageExportServiceTests: XCTestCase {
    func testPNGOutputHasMagicBytes() throws {
        let data = try ImageExportService.pngData(from: ScreenshotTestHelpers.image(width: 8, height: 8))
        XCTAssertEqual(Array(data.prefix(8)), [137, 80, 78, 71, 13, 10, 26, 10])
    }

    func testJPEGOutputHasMagicBytes() throws {
        let data = try ImageExportService.jpegData(from: ScreenshotTestHelpers.image(width: 8, height: 8), quality: 0.8)
        XCTAssertEqual(Array(data.prefix(2)), [0xFF, 0xD8])
    }
}

