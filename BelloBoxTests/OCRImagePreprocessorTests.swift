import XCTest
@testable import BelloBox

final class OCRImagePreprocessorTests: XCTestCase {
    func testCropChangesOutputDimensions() throws {
        let doc = ScreenshotDocument(baseImage: ScreenshotTestHelpers.image(width: 100, height: 100), scale: 1, source: .importedClipboard, cropRect: CGRect(x: 10, y: 10, width: 40, height: 30))
        let prepared = try OCRImagePreprocessor.prepare(document: doc, options: .default, forExternalUpload: false)
        XCTAssertEqual(prepared.image.width, 40)
        XCTAssertEqual(prepared.image.height, 30)
    }

    func testExternalUploadAppliesRedactionAndDigestChanges() throws {
        let base = ScreenshotTestHelpers.stripedImage(width: 80, height: 80)
        let redacted = ScreenshotDocument(baseImage: base, scale: 1, source: .importedClipboard, annotations: [ScreenshotAnnotation(kind: .blur(CGRect(x: 5, y: 5, width: 20, height: 20)), style: .redaction)])
        let plain = ScreenshotDocument(baseImage: base, scale: 1, source: .importedClipboard)
        let a = try OCRImagePreprocessor.prepare(document: plain, options: .default, forExternalUpload: true)
        let b = try OCRImagePreprocessor.prepare(document: redacted, options: .default, forExternalUpload: true)
        XCTAssertNotEqual(a.digest, b.digest)
        XCTAssertFalse(b.appliedRedactions.isEmpty)
    }

    func testLargeExternalUploadIsDownscaledToMaxLongEdge() throws {
        var options = OCROptions.default
        options.maxUploadLongEdge = 100
        let doc = ScreenshotDocument(baseImage: ScreenshotTestHelpers.stripedImage(width: 400, height: 200), scale: 1, source: .importedClipboard)
        let prepared = try OCRImagePreprocessor.prepare(document: doc, options: options, forExternalUpload: true)
        XCTAssertEqual(prepared.image.width, 100)
        XCTAssertEqual(prepared.image.height, 50)
        XCTAssertTrue(prepared.warnings.contains { $0.contains("downscaled") })
    }

    func testDecorativeAnnotationsAreExcludedFromExternalUpload() throws {
        let base = ScreenshotTestHelpers.image(width: 80, height: 80)
        let plain = ScreenshotDocument(baseImage: base, scale: 1, source: .importedClipboard)
        let decorated = ScreenshotDocument(
            baseImage: base,
            scale: 1,
            source: .importedClipboard,
            annotations: [ScreenshotAnnotation(kind: .rectangle(CGRect(x: 10, y: 10, width: 40, height: 40)))]
        )
        let a = try OCRImagePreprocessor.prepare(document: plain, options: .default, forExternalUpload: true)
        let b = try OCRImagePreprocessor.prepare(document: decorated, options: .default, forExternalUpload: true)
        XCTAssertEqual(a.digest, b.digest)
    }
}
