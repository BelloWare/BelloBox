import AppKit
import XCTest
@testable import BelloBox

@MainActor
final class UIUXBehaviorTests: XCTestCase {
    func testTextSizeAndColorChangesApplyToTheActiveAnnotationAndUndoTogether() throws {
        let viewModel = screenshotViewModel()
        viewModel.beginTextAnnotation(atVisiblePoint: CGPoint(x: 10, y: 10))
        viewModel.updateEditingText("Larger label")
        viewModel.style.fontSize = 36
        viewModel.style.strokeColor = CodableColor(.blue)
        viewModel.endTextEditing()
        let annotation = try XCTUnwrap(viewModel.document.annotations.first)
        XCTAssertEqual(annotation.style.fontSize, 36)
        XCTAssertEqual(annotation.style.strokeColor, CodableColor(.blue))
        XCTAssertEqual(viewModel.visibleAnnotations.first?.style, annotation.style)
        viewModel.undo()
        XCTAssertTrue(viewModel.document.annotations.isEmpty)
        viewModel.redo()
        XCTAssertEqual(viewModel.document.annotations.first?.style, annotation.style)
    }

    func testScreenshotCopyFeedbackClearsWhenTheImageChanges() {
        let viewModel = screenshotViewModel()
        viewModel.copyRenderedImage()
        XCTAssertNil(viewModel.errorMessage)
        XCTAssertEqual(viewModel.statusMessage, "Copied image.")
        viewModel.addVisibleAnnotation(.rectangle(CGRect(x: 10, y: 10, width: 20, height: 20)))
        XCTAssertNil(viewModel.statusMessage)
    }

    func testQRCapacityUsesUTF8BytesAndExplainsTheOverage() {
        let viewModel = QRCodePopupViewModel(text: String(repeating: "a", count: 1997) + "😀")
        XCTAssertEqual(viewModel.byteCount, 2001)
        XCTAssertTrue(viewModel.isTooLong)
        XCTAssertEqual(viewModel.capacityMessage, "Remove at least 1 byte to create a QR code.")
        XCTAssertNil(viewModel.image)
        viewModel.text = String(repeating: "a", count: 2000)
        XCTAssertFalse(viewModel.isTooLong)
        XCTAssertEqual(viewModel.capacityMessage, "0 bytes available")
        XCTAssertNotNil(viewModel.image)
    }

    func testEditingQRTextClearsStaleExportFeedback() {
        let viewModel = QRCodePopupViewModel(text: "Old text")
        viewModel.statusMessage = "Copied QR image."
        viewModel.errorMessage = "Previous error"
        viewModel.text = "New text"
        XCTAssertNil(viewModel.statusMessage)
        XCTAssertNil(viewModel.errorMessage)
    }

    func testOCRHasNoCopyablePlaceholderAndEmptyResultsDisableCopy() {
        let panel = OCRPanelViewModel()
        XCTAssertTrue(panel.plainText.isEmpty)
        XCTAssertFalse(panel.canCopyText)
        XCTAssertFalse(panel.canCopyMarkdown)
        panel.result = OCRResult(
            id: UUID(), engine: .appleVision(revision: 3, recognitionLevel: .accurate),
            target: .fullImage, plainText: " \n ", markdownText: nil, regions: [],
            languageHints: [], imageDigest: "test", warnings: [], createdAt: Date()
        )
        XCTAssertFalse(panel.canCopyText)
        XCTAssertFalse(panel.canCopyMarkdown)
        panel.result?.plainText = "Recognized text"
        XCTAssertTrue(panel.canCopyText)
        XCTAssertTrue(panel.canCopyMarkdown)
        panel.statusMessage = "Copied text."
        panel.result = nil
        XCTAssertNil(panel.statusMessage)
    }

    private func screenshotViewModel() -> ScreenshotPopupViewModel {
        let name = "BelloBoxTests.UIUX.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.set(false, forKey: "screenshotAutoCopy")
        return ScreenshotPopupViewModel(
            document: ScreenshotDocument(baseImage: ScreenshotTestHelpers.image(width: 320, height: 200), scale: 1, source: .importedClipboard),
            settings: AppSettings(defaults: defaults)
        )
    }
}
