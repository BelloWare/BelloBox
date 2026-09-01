import CoreGraphics
import XCTest
@testable import BelloBox

@MainActor
final class ScreenshotSelectionAdjustmentTests: XCTestCase {
    func testDisplayDocumentKeepsWholeDisplayAndSetsCrop() throws {
        let snapshot = DisplaySnapshot(
            displayID: 7,
            screenFrame: CGRect(x: 0, y: 0, width: 200, height: 100),
            scale: 2,
            image: ScreenshotTestHelpers.stripedImage(width: 400, height: 200)
        )

        let document = try ScreenCaptureService().displayDocument(
            fromSnapshot: snapshot,
            selectionCocoaRect: CGRect(x: 20, y: 30, width: 50, height: 40),
            source: .area(rect: .zero, displayID: 7)
        )

        XCTAssertEqual(document.baseImage.width, 400)
        XCTAssertEqual(document.baseImage.height, 200)
        XCTAssertEqual(document.scale, 2)
        // Cocoa y=30..70 from the bottom of a 100pt display is 30..70 from the top too.
        XCTAssertEqual(document.cropRect, CGRect(x: 40, y: 60, width: 100, height: 80))
        let rendered = try AnnotationRenderer.render(document)
        XCTAssertEqual(rendered.width, 100)
        XCTAssertEqual(rendered.height, 80)
    }

    func testDisplayDocumentLeavesCropUnsetForFullDisplaySelection() throws {
        let snapshot = DisplaySnapshot(
            displayID: 7,
            screenFrame: CGRect(x: 0, y: 0, width: 200, height: 100),
            scale: 1,
            image: ScreenshotTestHelpers.image(width: 200, height: 100)
        )
        let document = try ScreenCaptureService().displayDocument(
            fromSnapshot: snapshot,
            selectionCocoaRect: snapshot.screenFrame,
            source: .display(displayID: 7)
        )
        XCTAssertNil(document.cropRect)
    }

    func testSelectionAdjustmentIsUndoableAsOneStep() {
        let viewModel = makeViewModel()
        XCTAssertEqual(viewModel.selectionCropRect, CGRect(x: 40, y: 60, width: 100, height: 80))

        viewModel.beginSelectionAdjustment()
        viewModel.setSelectionCropRect(CGRect(x: 40, y: 60, width: 120, height: 80))
        viewModel.setSelectionCropRect(CGRect(x: 40, y: 60, width: 160, height: 90))
        viewModel.endSelectionAdjustment()

        XCTAssertEqual(viewModel.document.cropRect, CGRect(x: 40, y: 60, width: 160, height: 90))
        XCTAssertEqual(viewModel.visibleImageSize, CGSize(width: 160, height: 90))
        XCTAssertTrue(viewModel.canUndo)

        viewModel.undo()
        XCTAssertEqual(viewModel.document.cropRect, CGRect(x: 40, y: 60, width: 100, height: 80))
        XCTAssertFalse(viewModel.canUndo)
    }

    func testUnchangedAdjustmentLeavesNoUndoEntry() {
        let viewModel = makeViewModel()
        viewModel.beginSelectionAdjustment()
        viewModel.setSelectionCropRect(viewModel.selectionCropRect)
        viewModel.endSelectionAdjustment()
        XCTAssertFalse(viewModel.canUndo)
        XCTAssertFalse(viewModel.hasImageEdits)
    }

    func testSelectionIsClampedToImageAndMinimumSize() {
        let viewModel = makeViewModel()
        viewModel.setSelectionCropRect(CGRect(x: 300, y: 150, width: 500, height: 500))
        XCTAssertEqual(viewModel.document.cropRect, CGRect(x: 300, y: 150, width: 100, height: 50))

        viewModel.setSelectionCropRect(CGRect(x: 10, y: 10, width: 1, height: 1))
        let minimum = viewModel.minimumSelectionPixelSize
        XCTAssertEqual(viewModel.document.cropRect?.size, minimum)
    }

    func testGrowingToTheFullDisplayClearsTheCrop() {
        let viewModel = makeViewModel()
        viewModel.setSelectionCropRect(CGRect(x: 0, y: 0, width: 400, height: 200))
        XCTAssertNil(viewModel.document.cropRect)
        XCTAssertEqual(viewModel.visibleImageSize, CGSize(width: 400, height: 200))
    }

    func testAnnotationsStayAttachedToPixelsWhenSelectionMoves() {
        let viewModel = makeViewModel()
        viewModel.addVisibleAnnotation(.rectangle(CGRect(x: 10, y: 10, width: 20, height: 20)))
        guard case let .rectangle(documentRect)? = viewModel.document.annotations.first?.kind else {
            return XCTFail("Expected a rectangle annotation.")
        }
        XCTAssertEqual(documentRect, CGRect(x: 50, y: 70, width: 20, height: 20))

        viewModel.setSelectionCropRect(CGRect(x: 30, y: 50, width: 100, height: 80))

        guard case let .rectangle(visibleRect)? = viewModel.visibleAnnotations.first?.kind else {
            return XCTFail("Expected a visible rectangle annotation.")
        }
        XCTAssertEqual(visibleRect, CGRect(x: 20, y: 20, width: 20, height: 20))
    }

    func testOCRRegionsFollowTheSelectionWhenItMoves() {
        let viewModel = makeViewModel()
        let region = OCRTextRegion(
            kind: .line,
            text: "hello",
            boundingBox: CGRectCodable(CGRect(x: 10, y: 20, width: 30, height: 8)),
            children: [OCRTextRegion(kind: .word, text: "hello", boundingBox: CGRectCodable(CGRect(x: 10, y: 20, width: 12, height: 8)))]
        )
        viewModel.document.ocrResults = [
            OCRResult(
                id: UUID(),
                engine: .appleVision(revision: nil, recognitionLevel: .fast),
                target: .visibleAfterRedactions(crop: viewModel.document.cropRect.map(CGRectCodable.init)),
                plainText: "hello",
                markdownText: nil,
                regions: [region],
                languageHints: [],
                imageDigest: "digest",
                warnings: [],
                createdAt: Date()
            ),
        ]
        viewModel.document.activeOCRResultID = viewModel.document.ocrResults[0].id

        // Move the selection 30 px right and 10 px down: regions shift the other way so
        // they stay over the same pixels.
        viewModel.setSelectionCropRect(CGRect(x: 70, y: 70, width: 100, height: 80))

        let shifted = viewModel.document.activeOCRResult?.regions.first
        XCTAssertEqual(shifted?.boundingBox?.rect, CGRect(x: -20, y: 10, width: 30, height: 8))
        XCTAssertEqual(shifted?.children.first?.boundingBox?.rect, CGRect(x: -20, y: 10, width: 12, height: 8))
        XCTAssertTrue(viewModel.document.activeOCRResult?.warnings.contains("OCR may be out of date after crop or redaction changed.") == true)

        viewModel.undo()
        XCTAssertEqual(viewModel.document.activeOCRResult?.regions.first?.boundingBox?.rect, CGRect(x: 10, y: 20, width: 30, height: 8))
    }

    func testAdjustmentIsIgnoredForWindowStyleDocuments() {
        let viewModel = ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.image(width: 120, height: 80),
                scale: 1,
                source: .window(title: "Win", ownerName: "App", windowID: 1)
            ),
            settings: AppSettings(defaults: temporaryDefaults())
        )
        XCTAssertFalse(viewModel.supportsSelectionAdjustment)
        viewModel.setSelectionCropRect(CGRect(x: 0, y: 0, width: 10, height: 10))
        XCTAssertNil(viewModel.document.cropRect)
        XCTAssertFalse(viewModel.canUndo)
    }

    private func makeViewModel() -> ScreenshotPopupViewModel {
        ScreenshotPopupViewModel(
            document: ScreenshotDocument(
                baseImage: ScreenshotTestHelpers.stripedImage(width: 400, height: 200),
                scale: 2,
                source: .area(rect: .zero, displayID: 7),
                cropRect: CGRect(x: 40, y: 60, width: 100, height: 80)
            ),
            settings: AppSettings(defaults: temporaryDefaults()),
            allowsSelectionAdjustment: true
        )
    }

    private func temporaryDefaults() -> UserDefaults {
        let suiteName = "BelloBoxTests.SelectionAdjustment.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
