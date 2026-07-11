import XCTest
import ImageIO
@testable import BelloBox

final class QRCodeGeneratorTests: XCTestCase {
    func testEncodableBounds() {
        XCTAssertFalse(QRCodeGenerator.isEncodable(""))
        XCTAssertTrue(QRCodeGenerator.isEncodable("hello"))
        XCTAssertTrue(QRCodeGenerator.isEncodable("https://belloware.com/bello-box.html"))
        XCTAssertFalse(QRCodeGenerator.isEncodable(String(repeating: "a", count: QRCodeGenerator.maxByteCount + 1)))
    }

    func testImageForValidText() {
        let image = QRCodeGenerator.image(for: "BelloBox", pixelSize: 256)
        XCTAssertNotNil(image)
        XCTAssertEqual(image?.size.width, 256)
        XCTAssertEqual(image?.size.height, 256)
    }

    func testImageNilForEmpty() {
        XCTAssertNil(QRCodeGenerator.image(for: ""))
        XCTAssertNil(QRCodeGenerator.image(for: "   "))
    }

    func testPNGDataHasSignature() throws {
        let data = try XCTUnwrap(QRCodeGenerator.pngData(for: "https://belloware.com", pixelSize: 128))
        XCTAssertGreaterThan(data.count, 8)
        // PNG magic number.
        XCTAssertEqual(Array(data.prefix(4)), [0x89, 0x50, 0x4E, 0x47])

        let source = try XCTUnwrap(CGImageSourceCreateWithData(data as CFData, nil))
        let properties = try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any])
        XCTAssertEqual(properties[kCGImagePropertyPixelWidth] as? Int, 128)
        XCTAssertEqual(properties[kCGImagePropertyPixelHeight] as? Int, 128)
    }

    @MainActor
    func testPopupViewModelReusesImageUntilTextChanges() throws {
        let viewModel = QRCodePopupViewModel(text: "Bello Box")
        let first = try XCTUnwrap(viewModel.image)
        let second = try XCTUnwrap(viewModel.image)

        XCTAssertTrue(first === second)

        viewModel.text = "Bello Box updated"
        let updated = try XCTUnwrap(viewModel.image)
        XCTAssertFalse(first === updated)
    }
}
