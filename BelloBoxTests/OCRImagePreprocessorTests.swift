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

    func testAppliedRedactionsAreReportedInCroppedImageCoordinates() throws {
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.stripedImage(width: 120, height: 90),
            scale: 1,
            source: .importedClipboard,
            annotations: [
                ScreenshotAnnotation(kind: .blur(CGRect(x: 30, y: 20, width: 50, height: 40)), style: .redaction),
            ],
            cropRect: CGRect(x: 20, y: 10, width: 80, height: 60)
        )

        let prepared = try OCRImagePreprocessor.prepare(document: doc, options: .default, forExternalUpload: true)

        XCTAssertEqual(prepared.appliedCrop, CGRect(x: 20, y: 10, width: 80, height: 60))
        XCTAssertEqual(prepared.appliedRedactions, [CGRect(x: 10, y: 10, width: 50, height: 40)])
    }

    func testAppliedRedactionsClipToCropAndDropOutsideRedactions() throws {
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.stripedImage(width: 120, height: 90),
            scale: 1,
            source: .importedClipboard,
            annotations: [
                ScreenshotAnnotation(kind: .blur(CGRect(x: 10, y: 0, width: 30, height: 30)), style: .redaction),
                ScreenshotAnnotation(kind: .blur(CGRect(x: 102, y: 75, width: 10, height: 8)), style: .redaction),
            ],
            cropRect: CGRect(x: 20, y: 10, width: 80, height: 60)
        )

        let prepared = try OCRImagePreprocessor.prepare(document: doc, options: .default, forExternalUpload: true)

        XCTAssertEqual(prepared.appliedRedactions, [CGRect(x: 0, y: 0, width: 20, height: 20)])
    }

    func testAppliedRedactionsAreReportedInDownscaledImageCoordinates() throws {
        var options = OCROptions.default
        options.maxUploadLongEdge = 100
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.stripedImage(width: 400, height: 200),
            scale: 1,
            source: .importedClipboard,
            annotations: [
                ScreenshotAnnotation(kind: .blur(CGRect(x: 40, y: 20, width: 80, height: 40)), style: .redaction),
            ]
        )

        let prepared = try OCRImagePreprocessor.prepare(document: doc, options: options, forExternalUpload: true)

        XCTAssertNil(prepared.appliedCrop)
        XCTAssertEqual(prepared.appliedRedactions, [CGRect(x: 10, y: 5, width: 20, height: 10)])
    }

    func testAppliedRedactionsAccountForCropAndDownscaleTogether() throws {
        var options = OCROptions.default
        options.maxUploadLongEdge = 100
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.stripedImage(width: 300, height: 240),
            scale: 1,
            source: .importedClipboard,
            annotations: [
                ScreenshotAnnotation(kind: .blur(CGRect(x: 80, y: 60, width: 120, height: 60)), style: .redaction),
            ],
            cropRect: CGRect(x: 60, y: 40, width: 200, height: 120)
        )

        let prepared = try OCRImagePreprocessor.prepare(document: doc, options: options, forExternalUpload: true)

        XCTAssertEqual(prepared.image.width, 100)
        XCTAssertEqual(prepared.image.height, 60)
        XCTAssertEqual(prepared.appliedRedactions, [CGRect(x: 10, y: 10, width: 60, height: 30)])
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
