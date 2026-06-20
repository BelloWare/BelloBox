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

    private func temporaryDefaults(_ name: String = UUID().uuidString) -> UserDefaults {
        let defaults = UserDefaults(suiteName: "BelloBoxTests.InlineTextEditing.\(name)")!
        defaults.removePersistentDomain(forName: "BelloBoxTests.InlineTextEditing.\(name)")
        return defaults
    }
}
