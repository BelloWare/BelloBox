import XCTest
@testable import BelloBox

final class AnnotationRendererTests: XCTestCase {
    func testRectangleAnnotationChangesPixels() throws {
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 80, height: 80),
            scale: 1,
            source: .importedClipboard,
            annotations: [ScreenshotAnnotation(kind: .rectangle(CGRect(x: 10, y: 10, width: 40, height: 40)))]
        )
        let rendered = try AnnotationRenderer.render(doc)
        XCTAssertNotEqual(ScreenshotTestHelpers.pixel(rendered, x: 10, y: 10), ScreenshotTestHelpers.pixel(doc.baseImage, x: 10, y: 10))
    }

    func testCropReducesDimensions() throws {
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 100, height: 90),
            scale: 1,
            source: .importedClipboard,
            cropRect: CGRect(x: 10, y: 12, width: 40, height: 30)
        )
        let rendered = try AnnotationRenderer.render(doc)
        XCTAssertEqual(rendered.width, 40)
        XCTAssertEqual(rendered.height, 30)
    }

    func testHighlightAnnotationIsTranslucent() throws {
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 70, height: 70),
            scale: 1,
            source: .importedClipboard,
            annotations: [ScreenshotAnnotation(kind: .highlight(CGRect(x: 10, y: 10, width: 30, height: 30)), style: .highlight)]
        )
        let rendered = try AnnotationRenderer.render(doc)
        let pixel = ScreenshotTestHelpers.pixel(rendered, x: 20, y: 20)
        XCTAssertNotEqual(pixel, ScreenshotTestHelpers.pixel(doc.baseImage, x: 20, y: 20))
        XCTAssertGreaterThan(pixel[0], 100)
    }

    func testTextAnnotationRendersWithoutCrashing() throws {
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 160, height: 80),
            scale: 1,
            source: .importedClipboard,
            annotations: [ScreenshotAnnotation(kind: .text("Hello", origin: CGPoint(x: 14, y: 22), maxWidth: 120))]
        )
        let rendered = try AnnotationRenderer.render(doc)
        XCTAssertEqual(rendered.width, 160)
        XCTAssertEqual(rendered.height, 80)
    }

    func testRedactionChangesUnderlyingPixelsForOCRUpload() throws {
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.stripedImage(width: 80, height: 80),
            scale: 1,
            source: .importedClipboard,
            annotations: [ScreenshotAnnotation(kind: .blur(CGRect(x: 20, y: 20, width: 20, height: 20)), style: .redaction)]
        )
        let rendered = try AnnotationRenderer.renderForExternalOCRUpload(doc, target: .fullImage)
        XCTAssertNotEqual(ScreenshotTestHelpers.pixel(rendered, x: 25, y: 25), ScreenshotTestHelpers.pixel(doc.baseImage, x: 25, y: 25))
    }

    func testExternalOCRUploadExcludesDecorativeAnnotations() throws {
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 80, height: 80),
            scale: 1,
            source: .importedClipboard,
            annotations: [ScreenshotAnnotation(kind: .rectangle(CGRect(x: 10, y: 10, width: 40, height: 40)))]
        )
        let rendered = try AnnotationRenderer.renderForExternalOCRUpload(doc, target: .fullImage)
        XCTAssertEqual(ScreenshotTestHelpers.pixel(rendered, x: 10, y: 10), ScreenshotTestHelpers.pixel(doc.baseImage, x: 10, y: 10))
    }
}
