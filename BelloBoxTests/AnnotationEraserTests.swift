import AppKit
import SwiftUI
import XCTest
@testable import BelloBox

/// The eraser removes only the part of an annotation under the brush. The screenshot
/// pixels underneath come back, everything else stays (edges of thick strokes,
/// arrowheads, unbrushed mask), and one drag is one undo step. Nothing is ever
/// deleted on the eraser's behalf.
@MainActor
final class AnnotationEraserTests: XCTestCase {
    private let white: [UInt8] = [255, 255, 255, 255]

    private func makeViewModel(base: CGImage? = nil, annotations: [ScreenshotAnnotation] = [], crop: CGRect? = nil) -> ScreenshotPopupViewModel {
        let name = "BelloBoxTests.Eraser.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.set(false, forKey: "screenshotAutoCopy")
        let image = base ?? ScreenshotTestHelpers.image(width: 200, height: 100)
        return ScreenshotPopupViewModel(
            document: ScreenshotDocument(baseImage: image, scale: 1, source: .importedClipboard, annotations: annotations, cropRect: crop),
            settings: AppSettings(defaults: defaults)
        )
    }

    private func style(lineWidth: CGFloat) -> AnnotationStyle {
        var style = AnnotationStyle.default
        style.lineWidth = lineWidth
        return style
    }

    private func arrow(lineWidth: CGFloat = 6) -> ScreenshotAnnotation {
        ScreenshotAnnotation(kind: .arrow(start: CGPoint(x: 10, y: 50), end: CGPoint(x: 190, y: 50)), style: style(lineWidth: lineWidth))
    }

    /// One drag of the eraser through the given visible points.
    private func drag(_ viewModel: ScreenshotPopupViewModel, through points: [CGPoint]) {
        viewModel.beginErasing()
        for point in points { viewModel.erase(toVisiblePoint: point) }
        viewModel.endErasing()
    }

    private func isPainted(_ image: CGImage, x: Int, y: Int) -> Bool {
        ScreenshotTestHelpers.pixel(image, x: x, y: y) != white
    }

    func testStyleObserversNormalizeWithoutRecursingForAnyValue() {
        let viewModel = makeViewModel()
        viewModel.eraserWidth = 20
        XCTAssertEqual(viewModel.eraserWidth, 20)
        viewModel.eraserWidth = 20
        viewModel.eraserWidth = 0
        XCTAssertEqual(viewModel.eraserWidth, ScreenshotPopupViewModel.eraserWidthRange.lowerBound)
        viewModel.eraserWidth = 10_000
        XCTAssertEqual(viewModel.eraserWidth, ScreenshotPopupViewModel.eraserWidthRange.upperBound)
        viewModel.eraserWidth = .nan
        XCTAssertEqual(viewModel.eraserWidth, 24)
        viewModel.eraserWidth = -.infinity
        XCTAssertEqual(viewModel.eraserWidth, 24)
        for preset in AnnotationStyle.maskFillPresets {
            viewModel.maskStyle = .mask(fill: preset.color, pattern: .stripes)
            XCTAssertEqual(viewModel.maskStyle.maskFill, preset.color)
            viewModel.maskStyle = .mask(fill: preset.color, pattern: .stripes)
        }
        var translucent = AnnotationStyle.mask(fill: CodableColor(red: 0.5, green: 0.2, blue: 0.1, alpha: 1), pattern: .dots)
        translucent.fillColor?.alpha = 0.3
        translucent.opacity = 0.2
        translucent.lineWidth = 9
        viewModel.maskStyle = translucent
        XCTAssertEqual(viewModel.maskStyle, .mask(fill: CodableColor(red: 0.5, green: 0.2, blue: 0.1, alpha: 1), pattern: .dots))
        XCTAssertEqual(viewModel.maskStyle.fillColor?.alpha, 1)
        viewModel.maskStyle = .redaction
        XCTAssertEqual(viewModel.maskStyle, .redaction)
    }

    func testErasingTheMiddleOfAnArrowKeepsBothEndsAndRevealsTheScreenshotPixels() throws {
        let base = ScreenshotTestHelpers.stripedImage(width: 200, height: 100)
        let viewModel = makeViewModel(base: base, annotations: [arrow()])
        viewModel.activeTool = .eraser
        viewModel.eraserWidth = 20
        let before = try AnnotationRenderer.render(viewModel.document)
        XCTAssertNotEqual(ScreenshotTestHelpers.pixel(before, x: 100, y: 50), ScreenshotTestHelpers.pixel(base, x: 100, y: 50))

        drag(viewModel, through: [CGPoint(x: 100, y: 30), CGPoint(x: 100, y: 50), CGPoint(x: 100, y: 70)])

        let annotation = try XCTUnwrap(viewModel.document.annotations.first)
        XCTAssertEqual(annotation.erasures.count, 1, "A continuous drag is one stroke")
        XCTAssertEqual(annotation.erasures.first?.points.count, 3)
        XCTAssertEqual(annotation.erasures.first?.width, 20)
        let after = try AnnotationRenderer.render(viewModel.document)
        XCTAssertEqual(ScreenshotTestHelpers.pixel(after, x: 100, y: 50), ScreenshotTestHelpers.pixel(base, x: 100, y: 50),
                       "The screenshot pixel is revealed under the brush")
        XCTAssertEqual(ScreenshotTestHelpers.pixel(after, x: 102, y: 48), ScreenshotTestHelpers.pixel(base, x: 102, y: 48))
        XCTAssertEqual(ScreenshotTestHelpers.pixel(after, x: 40, y: 50), ScreenshotTestHelpers.pixel(before, x: 40, y: 50), "The left part is retained")
        XCTAssertEqual(ScreenshotTestHelpers.pixel(after, x: 160, y: 50), ScreenshotTestHelpers.pixel(before, x: 160, y: 50), "The right part is retained")
        XCTAssertEqual(ScreenshotTestHelpers.pixel(after, x: 186, y: 50), ScreenshotTestHelpers.pixel(before, x: 186, y: 50), "The arrowhead is retained")
        XCTAssertEqual(ScreenshotTestHelpers.pixel(after, x: 100, y: 10), ScreenshotTestHelpers.pixel(base, x: 100, y: 10), "Pixels away from annotations never change")
    }

    func testANarrowBrushAlongAThickStrokeLeavesBothPaintedEdges() throws {
        let pen = ScreenshotAnnotation(kind: .freehand(points: [CGPoint(x: 20, y: 50), CGPoint(x: 180, y: 50)]), style: style(lineWidth: 14))
        let viewModel = makeViewModel(annotations: [pen])
        viewModel.eraserWidth = 6
        drag(viewModel, through: stride(from: 40, through: 160, by: 10).map { CGPoint(x: CGFloat($0), y: 50) })
        XCTAssertEqual(viewModel.document.annotations.count, 1, "Only the brushed part goes; the stroke stays")
        let rendered = try AnnotationRenderer.render(viewModel.document)
        for x in [60, 100, 140] {
            XCTAssertFalse(isPainted(rendered, x: x, y: 50), "centre erased at x=\(x)")
            XCTAssertTrue(isPainted(rendered, x: x, y: 45), "upper edge retained at x=\(x)")
            XCTAssertTrue(isPainted(rendered, x: x, y: 55), "lower edge retained at x=\(x)")
        }
        XCTAssertTrue(isPainted(rendered, x: 25, y: 50), "the unbrushed start is intact")
        XCTAssertTrue(isPainted(rendered, x: 175, y: 50), "the unbrushed end is intact")
    }

    func testErasingAnArrowShaftKeepsTheUntouchedArrowhead() throws {
        let viewModel = makeViewModel(annotations: [arrow(lineWidth: 6)])
        viewModel.eraserWidth = 20
        drag(viewModel, through: stride(from: 20, through: 150, by: 10).map { CGPoint(x: CGFloat($0), y: 50) })
        XCTAssertEqual(viewModel.document.annotations.count, 1, "The arrow is kept even though most of the shaft is gone")
        let rendered = try AnnotationRenderer.render(viewModel.document)
        for x in [30, 80, 140] { XCTAssertFalse(isPainted(rendered, x: x, y: 50), "shaft erased at x=\(x)") }
        XCTAssertTrue(isPainted(rendered, x: 186, y: 50), "the arrowhead tip is untouched")
        XCTAssertTrue(isPainted(rendered, x: 178, y: 46), "the arrowhead barbs are untouched")
        XCTAssertTrue(isPainted(rendered, x: 170, y: 50), "the shaft just before the head is untouched")
    }

    func testSparseTapsOnAMaskLeaveTheUnbrushedFillCovering() throws {
        let base = ScreenshotTestHelpers.stripedImage(width: 200, height: 100)
        let mask = ScreenshotAnnotation(kind: .blur(CGRect(x: 20, y: 20, width: 60, height: 60)), style: .redaction)
        let viewModel = makeViewModel(base: base, annotations: [mask])
        viewModel.activeTool = .eraser
        viewModel.eraserWidth = 8
        for y in [30, 50, 70] {
            for x in [30, 50, 70] {
                viewModel.handleCanvasTap(visiblePoint: CGPoint(x: CGFloat(x), y: CGFloat(y)))
            }
        }
        XCTAssertEqual(viewModel.document.annotations.count, 1, "A mask with holes is still a mask")
        XCTAssertEqual(viewModel.document.annotations.first?.erasures.count, 9)
        let rendered = try AnnotationRenderer.render(viewModel.document)
        XCTAssertEqual(ScreenshotTestHelpers.pixel(rendered, x: 30, y: 30), ScreenshotTestHelpers.pixel(base, x: 30, y: 30), "a tapped spot shows the screenshot")
        for (x, y) in [(40, 40), (60, 60), (40, 60), (25, 25), (75, 75)] {
            let pixel = ScreenshotTestHelpers.pixel(rendered, x: x, y: y)
            XCTAssertNotEqual(pixel, ScreenshotTestHelpers.pixel(base, x: x, y: y), "between taps the mask still hides \(x),\(y)")
            XCTAssertEqual(pixel[3], 255)
            XCTAssertLessThan(pixel[0], 90)
        }
    }

    func testABrushGrazingAThickFilledRectangleStrokeErasesThatPaint() throws {
        var filled = style(lineWidth: 8)
        filled.fillColor = CodableColor(red: 0.2, green: 0.4, blue: 0.9, alpha: 1)
        let rectangle = ScreenshotAnnotation(kind: .rectangle(CGRect(x: 50, y: 30, width: 60, height: 40)), style: filled)
        let viewModel = makeViewModel(annotations: [rectangle])
        viewModel.eraserWidth = 8
        let before = try AnnotationRenderer.render(viewModel.document)
        XCTAssertTrue(isPainted(before, x: 47, y: 50), "the stroke extends 4 px outside the rect")
        drag(viewModel, through: [CGPoint(x: 44, y: 35), CGPoint(x: 44, y: 65)])
        XCTAssertEqual(viewModel.document.annotations.first?.erasures.count, 1, "grazing the outer stroke counts as a hit")
        let after = try AnnotationRenderer.render(viewModel.document)
        XCTAssertFalse(isPainted(after, x: 47, y: 50), "the grazed stroke is gone")
        XCTAssertTrue(isPainted(after, x: 80, y: 50), "the fill is intact")
        XCTAssertTrue(isPainted(after, x: 108, y: 50), "the far edge is intact")
        drag(viewModel, through: [CGPoint(x: 30, y: 35), CGPoint(x: 30, y: 65)])
        XCTAssertEqual(viewModel.document.annotations.first?.erasures.count, 1, "a pass clear of the paint records nothing")
    }

    func testCrossingErasuresNeverRestoreErasedPixels() throws {
        let pen = ScreenshotAnnotation(kind: .freehand(points: [CGPoint(x: 20, y: 50), CGPoint(x: 180, y: 50)]), style: style(lineWidth: 10))
        let viewModel = makeViewModel(annotations: [pen])
        viewModel.eraserWidth = 12
        drag(viewModel, through: [CGPoint(x: 60, y: 50), CGPoint(x: 140, y: 50)])
        drag(viewModel, through: [CGPoint(x: 100, y: 20), CGPoint(x: 100, y: 80)])
        XCTAssertEqual(viewModel.document.annotations.first?.erasures.count, 2)
        let rendered = try AnnotationRenderer.render(viewModel.document)
        XCTAssertFalse(isPainted(rendered, x: 100, y: 50), "the crossing point stays erased")
        XCTAssertFalse(isPainted(rendered, x: 70, y: 50), "erased by the first pass only")
        XCTAssertFalse(isPainted(rendered, x: 130, y: 50))
        XCTAssertTrue(isPainted(rendered, x: 30, y: 50))
        XCTAssertTrue(isPainted(rendered, x: 170, y: 50))
    }

    func testAContinuousDragErasesTheWholeSegmentAndAGestureIsOneUndoStep() throws {
        let viewModel = makeViewModel(annotations: [arrow()])
        viewModel.eraserWidth = 16
        XCTAssertFalse(viewModel.canUndo)
        drag(viewModel, through: [CGPoint(x: 60, y: 50), CGPoint(x: 70, y: 50), CGPoint(x: 80, y: 50), CGPoint(x: 120, y: 50), CGPoint(x: 130, y: 50)])
        let erased = try AnnotationRenderer.render(viewModel.document)
        for x in [70, 100, 125] { XCTAssertFalse(isPainted(erased, x: x, y: 50), "one continuous drag erases everything along it, x=\(x)") }
        XCTAssertTrue(isPainted(erased, x: 40, y: 50))
        XCTAssertTrue(isPainted(erased, x: 150, y: 50))
        XCTAssertTrue(viewModel.canUndo)
        viewModel.undo()
        XCTAssertFalse(viewModel.canUndo, "The whole drag was one undo step")
        XCTAssertTrue(viewModel.document.annotations.first?.erasures.isEmpty == true)
        XCTAssertTrue(isPainted(try AnnotationRenderer.render(viewModel.document), x: 70, y: 50))
        XCTAssertTrue(viewModel.canRedo)
        viewModel.redo()
        XCTAssertEqual(ScreenshotTestHelpers.rgbaPixels(try AnnotationRenderer.render(viewModel.document)), ScreenshotTestHelpers.rgbaPixels(erased))
    }

    func testLiftingTheBrushLeavesTheGapPaintedAndMakesASecondUndoStep() throws {
        let viewModel = makeViewModel(annotations: [arrow()])
        viewModel.eraserWidth = 16
        drag(viewModel, through: [CGPoint(x: 60, y: 50), CGPoint(x: 80, y: 50)])
        drag(viewModel, through: [CGPoint(x: 120, y: 50), CGPoint(x: 130, y: 50)])
        let rendered = try AnnotationRenderer.render(viewModel.document)
        XCTAssertFalse(isPainted(rendered, x: 70, y: 50))
        XCTAssertTrue(isPainted(rendered, x: 100, y: 50), "the brush was lifted between the passes")
        XCTAssertFalse(isPainted(rendered, x: 125, y: 50))
        XCTAssertEqual(viewModel.document.annotations.first?.erasures.count, 2)
        viewModel.undo()
        XCTAssertTrue(viewModel.canUndo, "Two gestures are two undo steps")
        XCTAssertTrue(isPainted(try AnnotationRenderer.render(viewModel.document), x: 125, y: 50))
        XCTAssertFalse(isPainted(try AnnotationRenderer.render(viewModel.document), x: 70, y: 50))
    }

    func testEraserOverNothingRecordsNoUndoStep() {
        let viewModel = makeViewModel(annotations: [arrow()])
        viewModel.eraserWidth = 10
        drag(viewModel, through: [CGPoint(x: 100, y: 10), CGPoint(x: 120, y: 10)])
        XCTAssertFalse(viewModel.canUndo)
        XCTAssertTrue(viewModel.document.annotations.first?.erasures.isEmpty == true)
        XCTAssertFalse(AnnotationGeometry.brush(from: CGPoint(x: 100, y: 10), to: CGPoint(x: 120, y: 10), radius: 5, touches: arrow()))
        let diagonal = ScreenshotAnnotation(kind: .arrow(start: CGPoint(x: 0, y: 0), end: CGPoint(x: 100, y: 100)), style: style(lineWidth: 6))
        XCTAssertFalse(AnnotationGeometry.brush(from: CGPoint(x: 80, y: 10), to: CGPoint(x: 90, y: 10), radius: 6, touches: diagonal),
                       "Inside the bounding box but away from the shaft is not a hit")
        XCTAssertTrue(AnnotationGeometry.brush(from: CGPoint(x: 50, y: 40), to: CGPoint(x: 50, y: 60), radius: 6, touches: diagonal))
    }

    func testErasuresFollowCropAndScaleAndMatchTheCanvas() throws {
        let viewModel = makeViewModel(annotations: [arrow()])
        viewModel.eraserWidth = 20
        drag(viewModel, through: [CGPoint(x: 100, y: 40), CGPoint(x: 100, y: 60)])
        viewModel.applyVisibleCrop(CGRect(x: 50, y: 0, width: 100, height: 100))
        let cropped = try AnnotationRenderer.render(viewModel.document)
        XCTAssertEqual(cropped.width, 100)
        XCTAssertEqual(ScreenshotTestHelpers.pixel(cropped, x: 50, y: 50), white, "The hole shifts with the crop")
        XCTAssertNotEqual(ScreenshotTestHelpers.pixel(cropped, x: 20, y: 50), white)
        XCTAssertNotEqual(ScreenshotTestHelpers.pixel(cropped, x: 80, y: 50), white)
        // Erasing in the visible (cropped) space lands on the right document pixels.
        drag(viewModel, through: [CGPoint(x: 20, y: 40), CGPoint(x: 20, y: 60)])
        XCTAssertEqual(ScreenshotTestHelpers.pixel(try AnnotationRenderer.render(viewModel.document), x: 20, y: 50), white)
        XCTAssertEqual(viewModel.document.annotations.first?.erasures.last?.points.first, CGPoint(x: 70, y: 40), "Stored in document pixels")

        let annotations = viewModel.visibleAnnotations
        XCTAssertEqual(annotations.first?.erasures.first?.points.first, CGPoint(x: 50, y: 40), "Visible copies shift erasures with the crop")
        let size = CGSize(width: 100, height: 100)
        let transparent = ScreenshotTestHelpers.image(width: 100, height: 100, draw: { _ in })
        let document = ScreenshotDocument(baseImage: transparent, scale: 1, source: .importedClipboard, annotations: annotations)
        for scale: CGFloat in [1, 0.5, 2] {
            let canvas = try ScreenshotTestHelpers.annotationPreview(annotations: annotations, imageSize: size,
                viewSize: CGSize(width: size.width * scale, height: size.height * scale))
            let export = try AnnotationRenderer.render(document, outputScale: scale)
            let a = ScreenshotTestHelpers.rgbaPixels(canvas), b = ScreenshotTestHelpers.rgbaPixels(export)
            let difference = zip(a, b).reduce(0) { $0 + abs(Int($1.0) - Int($1.1)) }
            XCTAssertLessThan(Double(difference) / Double(a.count), scale == 1 ? 0.5 : 4.0, "Canvas and export agree at \(scale)")
            XCTAssertEqual(ScreenshotTestHelpers.pixel(canvas, x: Int(50 * scale), y: Int(50 * scale))[3], 0, "The hole is transparent on the canvas at \(scale)")
        }
    }

    func testAnnotationsDrawnAfterErasingCoverTheErasedArea() throws {
        let viewModel = makeViewModel(annotations: [arrow()])
        viewModel.eraserWidth = 20
        drag(viewModel, through: [CGPoint(x: 100, y: 40), CGPoint(x: 100, y: 60)])
        XCTAssertEqual(ScreenshotTestHelpers.pixel(try AnnotationRenderer.render(viewModel.document), x: 100, y: 50), white)
        viewModel.addVisibleAnnotation(.highlight(CGRect(x: 80, y: 30, width: 40, height: 40)))
        let rendered = try AnnotationRenderer.render(viewModel.document)
        XCTAssertNotEqual(ScreenshotTestHelpers.pixel(rendered, x: 100, y: 50), white, "A later highlight is not punched by an earlier erasure")
        XCTAssertTrue(viewModel.document.annotations.last?.erasures.isEmpty == true)
        viewModel.addVisibleAnnotation(.rectangle(CGRect(x: 90, y: 40, width: 20, height: 20)))
        XCTAssertNotEqual(ScreenshotTestHelpers.pixel(try AnnotationRenderer.render(viewModel.document), x: 90, y: 50), white)
    }

    func testErasingPartOfAMaskRevealsOnlyThatPartInExportAndOCRUpload() throws {
        let base = ScreenshotTestHelpers.stripedImage(width: 200, height: 100)
        let mask = ScreenshotAnnotation(kind: .blur(CGRect(x: 20, y: 20, width: 60, height: 60)), style: .redaction)
        let viewModel = makeViewModel(base: base, annotations: [mask])
        viewModel.activeTool = .eraser
        viewModel.eraserWidth = 16
        viewModel.handleCanvasTap(visiblePoint: CGPoint(x: 50, y: 50))
        XCTAssertEqual(viewModel.document.annotations.first?.erasures.count, 1)
        for rendered in [try AnnotationRenderer.render(viewModel.document),
                         try AnnotationRenderer.renderForExternalOCRUpload(viewModel.document, target: .fullImage)] {
            XCTAssertEqual(ScreenshotTestHelpers.pixel(rendered, x: 50, y: 50), ScreenshotTestHelpers.pixel(base, x: 50, y: 50), "Erased part shows the screenshot")
            let masked = ScreenshotTestHelpers.pixel(rendered, x: 25, y: 25)
            XCTAssertNotEqual(masked, ScreenshotTestHelpers.pixel(base, x: 25, y: 25), "The rest of the mask still hides the pixels")
            XCTAssertEqual(masked[3], 255)
            XCTAssertLessThan(masked[0], 90)
        }
    }

    func testMovingAnErasedTextLabelCarriesItsHoleAlong() throws {
        let text = ScreenshotAnnotation(kind: .text("Hello world", origin: CGPoint(x: 10, y: 10), maxWidth: 150))
        let viewModel = makeViewModel(annotations: [text])
        viewModel.activeTool = .select
        viewModel.eraserWidth = 12
        drag(viewModel, through: [CGPoint(x: 20, y: 20), CGPoint(x: 30, y: 20)])
        XCTAssertEqual(viewModel.document.annotations.first?.erasures.count, 1)
        viewModel.beginMovingTextAnnotation(id: text.id)
        viewModel.moveTextAnnotation(id: text.id, toVisibleOrigin: CGPoint(x: 40, y: 30))
        viewModel.endMovingTextAnnotation(id: text.id)
        let moved = try XCTUnwrap(viewModel.document.annotations.first)
        guard case let .text(_, origin, _) = moved.kind else { return XCTFail("Expected text") }
        XCTAssertEqual(origin, CGPoint(x: 40, y: 30))
        XCTAssertEqual(moved.erasures.first?.points.first, CGPoint(x: 50, y: 40), "Holes move with the label")
        viewModel.removeAnnotation(id: text.id)
        XCTAssertTrue(viewModel.document.annotations.isEmpty, "Whole-object deletion is an explicit action")
    }

    func testMaskFillsStayOpaqueForEveryPresetAndPattern() throws {
        let base = ScreenshotTestHelpers.stripedImage(width: 80, height: 80)
        let rect = CGRect(x: 10, y: 10, width: 50, height: 50)
        var renders: [String: [UInt8]] = [:]
        for preset in AnnotationStyle.maskFillPresets {
            for pattern in MaskPattern.allCases {
                let style = AnnotationStyle.mask(fill: preset.color, pattern: pattern)
                XCTAssertEqual(style.fillColor?.alpha, 1)
                let document = ScreenshotDocument(baseImage: base, scale: 1, source: .importedClipboard,
                    annotations: [ScreenshotAnnotation(kind: .blur(rect), style: style)])
                let rendered = try AnnotationRenderer.render(document)
                let upload = try AnnotationRenderer.renderForExternalOCRUpload(document, target: .fullImage)
                XCTAssertEqual(ScreenshotTestHelpers.rgbaPixels(rendered), ScreenshotTestHelpers.rgbaPixels(upload), "Live, export and OCR coverage agree")
                for y in stride(from: 11, to: 59, by: 3) {
                    for x in stride(from: 11, to: 59, by: 3) {
                        let pixel = ScreenshotTestHelpers.pixel(rendered, x: x, y: y)
                        XCTAssertEqual(pixel[3], 255, "\(preset.name) \(pattern) opaque at \(x),\(y)")
                        XCTAssertNotEqual(pixel, ScreenshotTestHelpers.pixel(base, x: x, y: y), "\(preset.name) \(pattern) hides \(x),\(y)")
                    }
                }
                XCTAssertEqual(ScreenshotTestHelpers.pixel(rendered, x: 70, y: 70), ScreenshotTestHelpers.pixel(base, x: 70, y: 70), "Nothing outside the mask changes")
                renders["\(preset.name)-\(pattern.rawValue)"] = ScreenshotTestHelpers.rgbaPixels(rendered)
            }
            XCTAssertNotEqual(renders["\(preset.name)-solid"], renders["\(preset.name)-stripes"], "\(preset.name) stripes are visible")
            XCTAssertNotEqual(renders["\(preset.name)-solid"], renders["\(preset.name)-dots"], "\(preset.name) dots are visible")
        }
        // A translucent request is still rendered opaque.
        var translucent = AnnotationStyle.mask(fill: CodableColor(red: 1, green: 1, blue: 1, alpha: 0.2), pattern: .solid)
        translucent.opacity = 0.2
        let document = ScreenshotDocument(baseImage: base, scale: 1, source: .importedClipboard,
            annotations: [ScreenshotAnnotation(kind: .blur(rect), style: translucent)])
        let pixel = ScreenshotTestHelpers.pixel(try AnnotationRenderer.render(document), x: 30, y: 30)
        XCTAssertEqual(pixel, [255, 255, 255, 255])
    }

    func testMaskStyleChosenInTheToolbarIsUsedForNewMasksAndNeverTranslucent() throws {
        let viewModel = makeViewModel(base: ScreenshotTestHelpers.stripedImage(width: 200, height: 100))
        viewModel.maskStyle = .mask(fill: CodableColor(red: 0.98, green: 0.97, blue: 0.95, alpha: 0.3), pattern: .dots)
        XCTAssertEqual(viewModel.maskStyle.fillColor?.alpha, 1)
        viewModel.addVisibleAnnotation(.blur(CGRect(x: 10, y: 10, width: 40, height: 40)))
        let annotation = try XCTUnwrap(viewModel.document.annotations.first)
        XCTAssertEqual(annotation.style.maskPattern, .dots)
        XCTAssertEqual(annotation.style.maskFill, viewModel.maskStyle.maskFill)
        let pixel = ScreenshotTestHelpers.pixel(try AnnotationRenderer.render(viewModel.document), x: 12, y: 12)
        XCTAssertEqual(pixel[3], 255)
        XCTAssertGreaterThan(pixel[0], 200, "A light mask is light, not blended with the stripes")
    }

    func testZoomIsAppliedLiterallyAndCentringPaddingNeverChangesMagnification() {
        let image = CGSize(width: 1000, height: 500)
        let viewport = ImageViewport(imageSize: image, viewSize: CGSize(width: 1000, height: 600), scale: 0.25)
        XCTAssertEqual(viewport.fittedImageRect, CGRect(x: 375, y: 237.5, width: 250, height: 125), "25% is a quarter, centred in the padded view")
        XCTAssertEqual(viewport.scale, 0.25, accuracy: 0.0001)
        XCTAssertEqual(viewport.viewPointToImagePoint(CGPoint(x: 375, y: 237.5)), .zero)
        XCTAssertEqual(viewport.viewPointToImagePoint(CGPoint(x: 625, y: 362.5)), CGPoint(x: 1000, y: 500))
        XCTAssertEqual(viewport.imagePointToViewPoint(CGPoint(x: 500, y: 250)), CGPoint(x: 500, y: 300))
        let actual = ImageViewport(imageSize: CGSize(width: 300, height: 200), viewSize: CGSize(width: 1040, height: 700), scale: 1)
        XCTAssertEqual(actual.fittedImageRect.size, CGSize(width: 300, height: 200), "Actual Size never enlarges a small image")
        XCTAssertEqual(actual.scale, 1)
        let fitted = ImageViewport(imageSize: image, viewSize: CGSize(width: 1000, height: 600))
        XCTAssertEqual(fitted.fittedImageRect.size, CGSize(width: 1000, height: 500), "Fit is unchanged")
        XCTAssertEqual(CanvasZoom.scale(1).scale(imageSize: CGSize(width: 300, height: 200), available: CGSize(width: 1040, height: 700)), 1)
        XCTAssertEqual(CanvasZoom.scale(0.25).scale(imageSize: image, available: CGSize(width: 1000, height: 600)), 0.25)

        // Legacy scrollers shrink the viewport only on the axis that will overflow.
        let tall = CGSize(width: 300, height: 2400)
        let available = ZoomableCanvasScrollView.availableSize(bounds: CGSize(width: 1000, height: 600), zoom: .fitWidth, imageSize: tall, legacyScrollerWidth: 15)
        XCTAssertEqual(available, CGSize(width: 985, height: 600), "Fit Width uses the width left of the vertical scroller")
        XCTAssertEqual(CanvasZoom.fitWidth.contentSize(imageSize: tall, available: available).width, 985, "No horizontal overflow")
        let fit = ZoomableCanvasScrollView.availableSize(bounds: CGSize(width: 1000, height: 600), zoom: .fit, imageSize: tall, legacyScrollerWidth: 15)
        XCTAssertEqual(fit, CGSize(width: 1000, height: 600), "Fit never needs a scroller")
        let overlay = ZoomableCanvasScrollView.availableSize(bounds: CGSize(width: 1000, height: 600), zoom: .fitWidth, imageSize: tall, legacyScrollerWidth: 0)
        XCTAssertEqual(overlay, CGSize(width: 1000, height: 600), "Overlay scrollers take no width")
    }

    func testZoomHostLaysOutTheRequestedScaleAndReportsIt() async throws {
        let viewModel = makeViewModel(base: ScreenshotTestHelpers.stripedImage(width: 1000, height: 500))
        let host = ZoomableCanvasScrollView(viewModel: viewModel)
        host.frame = CGRect(x: 0, y: 0, width: 1000, height: 600)
        host.apply(zoom: .fit, imageSize: viewModel.visibleImageSize)
        host.layoutSubtreeIfNeeded()
        XCTAssertEqual(host.renderScale, 1, accuracy: 0.001, "1000×500 fits a 1000×600 viewport at 100%")
        viewModel.zoom = .scale(0.25)
        host.apply(zoom: .scale(0.25), imageSize: viewModel.visibleImageSize)
        XCTAssertEqual(host.renderScale, 0.25, accuracy: 0.001, "25% really shrinks below fit")
        XCTAssertEqual(host.documentView?.frame.size, CGSize(width: 1000, height: 600), "The document is padded to the viewport, not magnified")
        for _ in 0..<50 where viewModel.displayedScale != 0.25 { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertEqual(viewModel.displayedScale, 0.25, accuracy: 0.001, "The Fit label follows the laid-out scale")
        XCTAssertEqual(viewModel.zoomLabel, "25%")
        viewModel.zoomToFit()
        host.apply(zoom: .fit, imageSize: viewModel.visibleImageSize)
        for _ in 0..<50 where viewModel.displayedScale != 1 { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertEqual(viewModel.zoomLabel, "100% · Fit")

        let small = makeViewModel(base: ScreenshotTestHelpers.stripedImage(width: 300, height: 200))
        let smallHost = ZoomableCanvasScrollView(viewModel: small)
        smallHost.frame = CGRect(x: 0, y: 0, width: 1040, height: 700)
        smallHost.apply(zoom: .scale(1), imageSize: small.visibleImageSize)
        XCTAssertEqual(smallHost.renderScale, 1, "Actual Size does not enlarge a small image")
        smallHost.apply(zoom: .scale(3), imageSize: small.visibleImageSize)
        XCTAssertEqual(smallHost.renderScale, 3)
        XCTAssertEqual(smallHost.documentView?.frame.size, CGSize(width: 1040, height: 700), "300% of 300×200 still fits the viewport")

        let tall = makeViewModel(base: ScreenshotTestHelpers.stripedImage(width: 300, height: 2400))
        let tallHost = ZoomableCanvasScrollView(viewModel: tall)
        tallHost.frame = CGRect(x: 0, y: 0, width: 1000, height: 600)
        tallHost.apply(zoom: .fitWidth, imageSize: tall.visibleImageSize)
        tallHost.layoutSubtreeIfNeeded()
        let document = try XCTUnwrap(tallHost.documentView)
        XCTAssertLessThanOrEqual(document.frame.width, tallHost.contentView.bounds.width + 0.5, "Fit Width never overflows horizontally")
        XCTAssertGreaterThan(document.frame.height, 4000, "Fit Width of a tall page scrolls vertically")
        XCTAssertEqual(tallHost.renderScale, document.frame.width / 300, accuracy: 0.01)
    }

    func testDefaultZoomFitsWidthOnlyForTallScrollingCaptures() {
        let tall = ScreenshotDocument(baseImage: ScreenshotTestHelpers.image(width: 300, height: 2400), scale: 1,
            source: .scrolling(target: ScrollCaptureTargetSummary(title: nil, ownerName: nil, frame: nil), frameCount: 8))
        XCTAssertEqual(ScreenshotPopupViewModel.initialZoom(for: tall), .fitWidth)
        let wide = ScreenshotDocument(baseImage: ScreenshotTestHelpers.image(width: 2400, height: 300), scale: 1,
            source: .scrolling(target: ScrollCaptureTargetSummary(title: nil, ownerName: nil, frame: nil), frameCount: 8))
        XCTAssertEqual(ScreenshotPopupViewModel.initialZoom(for: wide), .fit)
        let plain = ScreenshotDocument(baseImage: ScreenshotTestHelpers.image(width: 300, height: 2400), scale: 1, source: .importedClipboard)
        XCTAssertEqual(ScreenshotPopupViewModel.initialZoom(for: plain), .fit)
        XCTAssertEqual(CanvasZoom.zoomedIn(from: 0.6), .scale(0.75))
        XCTAssertEqual(CanvasZoom.zoomedOut(from: 0.6), .scale(0.5))
        XCTAssertEqual(CanvasZoom.zoomedIn(from: 4), .scale(4))
        XCTAssertEqual(CanvasZoom.zoomedOut(from: 0.1), .scale(0.25))
        let viewModel = makeViewModel()
        viewModel.reportDisplayedScale(0.5)
        viewModel.zoomIn()
        XCTAssertEqual(viewModel.zoom, .scale(0.75))
        XCTAssertEqual(viewModel.zoomLabel, "75%")
        viewModel.zoomToFit()
        viewModel.reportDisplayedScale(0.33)
        XCTAssertEqual(viewModel.zoomLabel, "33% · Fit")
        viewModel.zoomToActualSize()
        XCTAssertEqual(viewModel.zoom, .scale(1))
    }

    func testCaptureNotesKeepEveryNoteReachableWithIncompleteCaptureFirst() throws {
        let notes = [
            "The last 2 frames were left out to keep the screenshot within the maximum height.",
            "Frame 2 could not be matched to the frame before it, so the picture may be missing or repeating content at that seam.",
            "Frame 3 could not be matched to the frame before it, so the picture may be missing or repeating content at that seam.",
            "Frame 4 could not be matched to the frame before it, so the picture may be missing or repeating content at that seam.",
            "Frame 5 looks almost the same as the frame before it, so it adds little or nothing.",
        ]
        let document = ScreenshotDocument(baseImage: ScreenshotTestHelpers.image(width: 200, height: 1200), scale: 1,
            source: .scrolling(target: ScrollCaptureTargetSummary(title: nil, ownerName: nil, frame: nil), frameCount: 6), captureNotes: notes)
        let viewModel = ScreenshotPopupViewModel(document: document, settings: AppSettings(defaults: UserDefaults(suiteName: "BelloBoxTests.Notes.\(UUID().uuidString)")!))
        XCTAssertTrue(viewModel.showsCaptureNotes)
        XCTAssertFalse(viewModel.showsAllCaptureNotes)
        func height(showsAll: Bool) -> CGFloat {
            let banner = CaptureNotesBanner(title: "Scrolling capture · 6 frames", notes: notes,
                showsAll: .constant(showsAll), onDismiss: {}).frame(width: 1000)
            return NSHostingView(rootView: banner).fittingSize.height
        }
        let collapsed = height(showsAll: false)
        let expanded = height(showsAll: true)
        XCTAssertLessThan(collapsed, 110, "Two notes plus a Show all link stay compact")
        XCTAssertGreaterThan(expanded, collapsed)
        XCTAssertLessThanOrEqual(expanded, CaptureNotesBanner.expandedMaxHeight + 70, "Everything else scrolls inside the banner")
        XCTAssertEqual(CaptureNotesBanner.inlineCount, 2)
        XCTAssertTrue(notes[0].contains("left out"), "The engine puts the incomplete-capture note first, so it is always inline")
        viewModel.showsAllCaptureNotes = true
        viewModel.dismissCaptureNotes()
        XCTAssertFalse(viewModel.showsCaptureNotes)
    }

    func testTallBaseImageViewDrawsThroughABoundedDisplayCopy() throws {
        let tall = ScreenshotTestHelpers.stripedImage(width: 64, height: 4096)
        XCTAssertEqual(ScreenshotBaseImageNSView.step(for: 1), 0)
        XCTAssertEqual(ScreenshotBaseImageNSView.step(for: 0.6), 0)
        XCTAssertEqual(ScreenshotBaseImageNSView.step(for: 0.3), 1)
        XCTAssertEqual(ScreenshotBaseImageNSView.step(for: 0.1), 3)
        let copy = try XCTUnwrap(ScreenshotBaseImageNSView.displayCopy(of: tall, step: 3))
        XCTAssertEqual(copy.width, 8)
        XCTAssertEqual(copy.height, 512)
        XCTAssertNil(ScreenshotBaseImageNSView.displayCopy(of: tall, step: 0), "Magnified views draw the original")

        let view = ScreenshotBaseImageNSView(frame: CGRect(x: 0, y: 0, width: 16, height: 1024))
        view.update(image: tall, displayScale: 0.25)
        let bitmap = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 16, pixelsHigh: 1024, bitsPerSample: 8,
            samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 32))
        view.cacheDisplay(in: view.bounds, to: bitmap)
        let drawn = try XCTUnwrap(bitmap.cgImage)
        let top = ScreenshotTestHelpers.pixel(drawn, x: 8, y: 2)
        let bottom = ScreenshotTestHelpers.pixel(drawn, x: 8, y: 1020)
        XCTAssertEqual(top[3], 255)
        XCTAssertNotEqual(top, bottom, "The stripes survive the display copy")
    }
}

@MainActor
final class AnnotationToolbarLayoutTests: XCTestCase {
    func testOverlayToolbarFitsItsReservedWidthWithEveryToolSelected() {
        let name = "BelloBoxTests.Toolbar.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.set(false, forKey: "screenshotAutoCopy")
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(baseImage: ScreenshotTestHelpers.image(width: 320, height: 200), scale: 1, source: .importedClipboard),
            settings: AppSettings(defaults: defaults)
        )
        for tool in AnnotationTool.allCases {
            viewModel.activeTool = tool
            let toolbar = AnnotationToolbarView(viewModel: viewModel, showExportActions: true, onClose: {}, onScrollCapture: {})
                .padding(.horizontal, 10).padding(.vertical, 8)
            let view = NSHostingView(rootView: toolbar)
            XCTAssertLessThanOrEqual(view.fittingSize.width, AnnotationToolbarView.overlayToolbarWidth, "\(tool) overflows the overlay toolbar")
            XCTAssertLessThanOrEqual(view.fittingSize.height, 54, "\(tool) makes the toolbar taller than its slot")
        }
        viewModel.activeTool = .blur
        let popup = NSHostingView(rootView: AnnotationToolbarView(viewModel: viewModel))
        XCTAssertLessThanOrEqual(popup.fittingSize.width, ScreenshotPopupView.preferredSize.width - 32, "The popup toolbar fits the popup")
    }
}
