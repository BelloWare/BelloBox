import XCTest
@testable import BelloBox

final class AnnotationRendererTests: XCTestCase {
    @MainActor
    func testRedactionCoversTextAndShapesRegardlessOfAnnotationOrder() throws {
        let base = ScreenshotTestHelpers.image(width: 200, height: 100)
        let redaction = ScreenshotAnnotation(kind: .blur(CGRect(x: 0, y: 0, width: 200, height: 100)), style: .redaction)
        let marks: [ScreenshotAnnotation] = [
            .init(kind: .text("Private annotation", origin: CGPoint(x: 10, y: 10), maxWidth: 180)),
            .init(kind: .rectangle(CGRect(x: 10, y: 50, width: 100, height: 30))),
            .init(kind: .freehand(points: [CGPoint(x: 10, y: 70), CGPoint(x: 190, y: 70)])),
            .init(kind: .highlight(CGRect(x: 0, y: 0, width: 200, height: 100)), style: .highlight)
        ]
        let expected = try AnnotationRenderer.render(ScreenshotDocument(baseImage: base, scale: 1,
            source: .importedClipboard, annotations: [redaction]))
        for annotations in [marks + [redaction], [redaction] + marks] {
            let actual = try AnnotationRenderer.render(ScreenshotDocument(baseImage: base, scale: 1,
                source: .importedClipboard, annotations: annotations))
            XCTAssertEqual(ScreenshotTestHelpers.rgbaPixels(actual), ScreenshotTestHelpers.rgbaPixels(expected),
                "Redacted content must not expose annotation text, lines or shapes")
        }
    }

    @MainActor
    func testCanvasMatchesExportAtNativeAndReducedScales() throws {
        let annotations: [ScreenshotAnnotation] = [
            .init(kind: .freehand(points: [CGPoint(x: 10, y: 12), CGPoint(x: 60, y: 32), CGPoint(x: 100, y: 15)])),
            .init(kind: .arrow(start: CGPoint(x: 20, y: 55), end: CGPoint(x: 160, y: 75))),
            .init(kind: .rectangle(CGRect(x: 100, y: 20, width: 65, height: 35))),
            .init(kind: .text("Wrapped preview and export text", origin: CGPoint(x: 10, y: 95), maxWidth: 130)),
            .init(kind: .highlight(CGRect(x: 30, y: 10, width: 40, height: 130)), style: .highlight),
            .init(kind: .blur(CGRect(x: 135, y: 110, width: 35, height: 45)), style: .redaction)
        ]
        let size = CGSize(width: 200, height: 200)
        let transparent = ScreenshotTestHelpers.image(width: 200, height: 200, draw: { _ in })
        let document = ScreenshotDocument(baseImage: transparent, scale: 2, source: .importedClipboard, annotations: annotations)
        for scale: CGFloat in [1, 0.5, 2] {
            let canvas = try ScreenshotTestHelpers.annotationPreview(
                annotations: annotations, imageSize: size,
                viewSize: CGSize(width: size.width * scale, height: size.height * scale)
            )
            let export = try AnnotationRenderer.render(document, outputScale: scale)
            XCTAssertEqual(canvas.width, export.width)
            XCTAssertEqual(canvas.height, export.height)
            let a = ScreenshotTestHelpers.rgbaPixels(canvas)
            let b = ScreenshotTestHelpers.rgbaPixels(export)
            let difference = zip(a, b).reduce(0) { $0 + abs(Int($1.0) - Int($1.1)) }
            // Resampling an exported PNG can antialias edges differently from drawing
            // vectors at that scale. Position, line weight, text layout and paint order
            // must still agree across the full image.
            XCTAssertLessThan(Double(difference) / Double(a.count), scale == 1 ? 0.5 : 4.0, "Preview differs at scale \(scale)")
        }
    }

    @MainActor
    func testCanvasStrokeWidthScalesWithTheImage() throws {
        var style = AnnotationStyle.default
        style.lineWidth = 12
        let annotation = ScreenshotAnnotation(kind: .freehand(points: [CGPoint(x: 20, y: 50), CGPoint(x: 180, y: 50)]), style: style)
        for scale: CGFloat in [0.25, 0.5, 1, 2] {
            let image = try ScreenshotTestHelpers.annotationPreview(annotations: [annotation],
                imageSize: CGSize(width: 200, height: 100), viewSize: CGSize(width: 200 * scale, height: 100 * scale))
            let thickness = (0..<image.height).filter { ScreenshotTestHelpers.pixel(image, x: image.width / 2, y: $0)[3] >= 128 }.count
            XCTAssertEqual(CGFloat(thickness), style.lineWidth * scale, accuracy: 1)
        }
    }

    func testRedactionHatchingDoesNotPaintOutsideItsBounds() throws {
        let base = ScreenshotTestHelpers.image(width: 80, height: 80)
        let document = ScreenshotDocument(baseImage: base, scale: 1, source: .importedClipboard,
            annotations: [.init(kind: .blur(CGRect(x: 10, y: 10, width: 20, height: 40)), style: .redaction)])
        let rendered = try AnnotationRenderer.render(document)
        for y in 0..<80 {
            for x in 31..<80 {
                XCTAssertEqual(ScreenshotTestHelpers.pixel(rendered, x: x, y: y), [255, 255, 255, 255])
            }
        }
    }

    func testPenTapRendersADot() throws {
        let document = ScreenshotDocument(baseImage: ScreenshotTestHelpers.image(width: 30, height: 30),
            scale: 1, source: .importedClipboard,
            annotations: [.init(kind: .freehand(points: [CGPoint(x: 15, y: 15)]))])
        let rendered = try AnnotationRenderer.render(document)
        XCTAssertNotEqual(ScreenshotTestHelpers.pixel(rendered, x: 15, y: 15), [255, 255, 255, 255])
    }

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

    func testWrappedTextAnnotationRendersBeyondFirstLine() throws {
        let text = Array(repeating: "wrapped", count: 28).joined(separator: " ")
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 140, height: 220),
            scale: 1,
            source: .importedClipboard,
            annotations: [
                ScreenshotAnnotation(
                    kind: .text(text, origin: CGPoint(x: 10, y: 8), maxWidth: 54),
                    style: AnnotationStyle(
                        strokeColor: CodableColor(red: 0, green: 0, blue: 0, alpha: 1),
                        fillColor: nil,
                        lineWidth: 1,
                        opacity: 1,
                        fontSize: 18
                    )
                )
            ]
        )

        let rendered = try AnnotationRenderer.render(doc)
        var changedPixels = 0
        for y in 80..<180 {
            for x in 10..<70 where ScreenshotTestHelpers.pixel(rendered, x: x, y: y) != ScreenshotTestHelpers.pixel(doc.baseImage, x: x, y: y) {
                changedPixels += 1
            }
        }
        XCTAssertGreaterThan(changedPixels, 0)
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

    func testRedactionIsFullyOpaqueInExport() throws {
        let doc = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.stripedImage(width: 80, height: 80),
            scale: 1,
            source: .importedClipboard,
            annotations: [
                ScreenshotAnnotation(
                    kind: .blur(CGRect(x: 20, y: 20, width: 20, height: 20)),
                    style: AnnotationStyle(
                        strokeColor: CodableColor(red: 0, green: 0, blue: 0, alpha: 1),
                        fillColor: CodableColor(red: 0.16, green: 0.16, blue: 0.16, alpha: 0.4),
                        lineWidth: 0,
                        opacity: 0.3,
                        fontSize: 18
                    )
                ),
            ]
        )
        let rendered = try AnnotationRenderer.render(doc)
        for (x, y) in [(21, 21), (30, 30), (39, 39), (25, 35)] {
            let pixel = ScreenshotTestHelpers.pixel(rendered, x: x, y: y)
            XCTAssertEqual(pixel[3], 255, "redaction must be opaque at (\(x), \(y))")
            XCTAssertEqual(pixel[0], pixel[1], "redaction must be neutral grey at (\(x), \(y))")
            XCTAssertEqual(pixel[1], pixel[2], "redaction must be neutral grey at (\(x), \(y))")
            XCTAssertLessThan(pixel[0], 90, "redaction must stay dark at (\(x), \(y))")
        }
        XCTAssertEqual(ScreenshotTestHelpers.pixel(rendered, x: 10, y: 10), ScreenshotTestHelpers.pixel(doc.baseImage, x: 10, y: 10))
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
