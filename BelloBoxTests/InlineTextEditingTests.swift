import CoreGraphics
import XCTest
@testable import BelloBox

@MainActor
final class InlineTextEditingTests: XCTestCase {
    func testTextEditingLifecycleUpdatesAnnotationInPlace() {
        let settings = AppSettings(defaults: temporaryDefaults())
        let document = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 200, height: 120),
            scale: 1,
            source: .importedClipboard
        )
        let viewModel = ScreenshotPopupViewModel(document: document, settings: settings)

        viewModel.beginTextAnnotation(atVisiblePoint: CGPoint(x: 20, y: 30))
        let id = viewModel.editingTextAnnotationID
        XCTAssertNotNil(id)
        XCTAssertEqual(viewModel.document.annotations.count, 1)

        viewModel.updateEditingText("Hello")
        XCTAssertEqual(viewModel.textForEditingAnnotation(), "Hello")

        viewModel.endTextEditing()
        XCTAssertNil(viewModel.editingTextAnnotationID)
        XCTAssertEqual(viewModel.document.annotations.count, 1)
    }

    func testEmptyTextAnnotationIsRemovedOnCommit() {
        let settings = AppSettings(defaults: temporaryDefaults())
        let document = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 200, height: 120),
            scale: 1,
            source: .importedClipboard
        )
        let viewModel = ScreenshotPopupViewModel(document: document, settings: settings)

        viewModel.beginTextAnnotation(atVisiblePoint: CGPoint(x: 20, y: 30))
        viewModel.endTextEditing()

        XCTAssertNil(viewModel.editingTextAnnotationID)
        XCTAssertTrue(viewModel.document.annotations.isEmpty)
    }

    func testTextEditingInCroppedImageMapsVisiblePointToDocumentPoint() {
        let settings = AppSettings(defaults: temporaryDefaults())
        let document = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 200, height: 120),
            scale: 1,
            source: .importedClipboard,
            cropRect: CGRect(x: 40, y: 25, width: 80, height: 60)
        )
        let viewModel = ScreenshotPopupViewModel(document: document, settings: settings)

        viewModel.beginTextAnnotation(atVisiblePoint: CGPoint(x: 12, y: 15))

        guard let annotation = viewModel.document.annotations.first,
              case let .text(_, origin, _) = annotation.kind
        else {
            return XCTFail("Expected a text annotation")
        }
        XCTAssertEqual(origin, CGPoint(x: 52, y: 40))
        XCTAssertEqual(viewModel.visibleTextFrameForEditingAnnotation()?.origin, CGPoint(x: 12, y: 15))
    }

    func testRedactionInCroppedImageMapsVisibleRectToDocumentRect() {
        let settings = AppSettings(defaults: temporaryDefaults())
        let document = ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 200, height: 120),
            scale: 1,
            source: .importedClipboard,
            cropRect: CGRect(x: 40, y: 25, width: 80, height: 60)
        )
        let viewModel = ScreenshotPopupViewModel(document: document, settings: settings)

        viewModel.addVisibleAnnotation(.blur(CGRect(x: 5, y: 7, width: 20, height: 11)))

        guard let annotation = viewModel.document.annotations.first,
              case let .blur(rect) = annotation.kind
        else {
            return XCTFail("Expected a redaction annotation")
        }
        XCTAssertEqual(rect, CGRect(x: 45, y: 32, width: 20, height: 11))
        XCTAssertEqual(annotation.style, .redaction)
    }

    func testImageViewportMapsLetterboxedCanvasPoints() {
        let viewport = ImageViewport(
            imageSize: CGSize(width: 400, height: 200),
            viewSize: CGSize(width: 500, height: 500)
        )

        XCTAssertEqual(viewport.fittedImageRect, CGRect(x: 0, y: 125, width: 500, height: 250))
        XCTAssertEqual(viewport.viewPointToImagePoint(CGPoint(x: 250, y: 250)), CGPoint(x: 200, y: 100))
        XCTAssertEqual(viewport.viewPointToImagePoint(CGPoint(x: 250, y: 50)), CGPoint(x: 200, y: 0))
        XCTAssertEqual(viewport.viewPointToImagePoint(CGPoint(x: 250, y: 470)), CGPoint(x: 200, y: 200))
        XCTAssertEqual(viewport.imagePointToViewPoint(CGPoint(x: 400, y: 200)), CGPoint(x: 500, y: 375))
    }

    private func temporaryDefaults(_ name: String = UUID().uuidString) -> UserDefaults {
        let defaults = UserDefaults(suiteName: "BelloBoxTests.InlineTextEditing.\(name)")!
        defaults.removePersistentDomain(forName: "BelloBoxTests.InlineTextEditing.\(name)")
        return defaults
    }
}
