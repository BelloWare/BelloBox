import AppKit
import Combine
import CoreImage
import XCTest
@testable import BelloBox

/// QR codes for edited text are drawn off the main thread (up to ~30 ms each
/// for 2,000 bytes in the popup, and as much again in the palette row), and a
/// result for text that has since changed is never shown.
@MainActor
final class QRRenderingTests: XCTestCase {
    private static func decode(_ image: NSImage?) -> String? {
        guard let image, let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
        let detector = CIDetector(ofType: CIDetectorTypeQRCode, context: nil, options: [CIDetectorAccuracy: CIDetectorAccuracyHigh])!
        return detector.features(in: CIImage(cgImage: cg)).compactMap { ($0 as? CIQRCodeFeature)?.messageString }.first
    }

    private func waitUntil(_ predicate: () -> Bool) async throws {
        for _ in 0..<300 {
            if predicate() { return }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        XCTFail("Condition did not settle")
    }

    func testEditingDrawsTheNewCodeOffTheMainThread() async throws {
        let viewModel = QRCodePopupViewModel(text: "first")
        let first = try XCTUnwrap(viewModel.image, "The opening code is drawn at once")
        viewModel.text = String(repeating: "b", count: 1_900)
        XCTAssertTrue(viewModel.image === first, "The edit returns before encoding; the old code stays meanwhile")
        try await waitUntil { viewModel.image !== first }
        XCTAssertEqual(Self.decode(viewModel.image), viewModel.text)
    }

    func testOnlyTheLatestEditIsShown() async throws {
        let viewModel = QRCodePopupViewModel(text: "start")
        var shown: [String?] = []
        let observer = viewModel.$image.dropFirst().sink { shown.append(Self.decode($0)) }
        defer { observer.cancel() }
        for index in 1...20 { viewModel.text = "edit \(index) " + String(repeating: "c", count: 1_500) }
        let latest = viewModel.text
        try await waitUntil { Self.decode(viewModel.image) == latest }
        try await Task.sleep(nanoseconds: 200_000_000)
        XCTAssertEqual(shown.last, latest, "Nothing older lands after the latest")
        XCTAssertLessThan(shown.count, 20, "Superseded edits are skipped, not all drawn")
    }

    func testTextThatCannotBeEncodedClearsTheCodeAtOnceAndStaysClear() async throws {
        let viewModel = QRCodePopupViewModel(text: "start")
        viewModel.text = String(repeating: "d", count: 1_800)
        viewModel.text = ""
        XCTAssertNil(viewModel.image)
        try await Task.sleep(nanoseconds: 300_000_000)
        XCTAssertNil(viewModel.image, "The earlier edit's code does not land afterwards")
        viewModel.text = "back"
        XCTAssertEqual(Self.decode(viewModel.image), "back", "With no code shown, the next one is drawn at once")
    }

    func testASpellingOnlyEditRedrawsTheCode() async throws {
        let viewModel = QRCodePopupViewModel(text: "caf\u{E9}")
        let first = try XCTUnwrap(viewModel.image)
        viewModel.text = "cafe\u{301}"
        try await waitUntil { viewModel.image !== first }
    }

    func testThePaletteCardKeepsItsCodeWhileTheEditedOneIsDrawn() async throws {
        let qr = LauncherQRPreview(text: "first")
        let first = try XCTUnwrap(qr.cardImage)
        qr.code.text = String(repeating: "e", count: 300)
        XCTAssertEqual(qr.moduleCount, QRCodeGenerator.moduleCount(for: qr.text), "Metrics still follow the edit at once")
        XCTAssertTrue(qr.cardImage === first, "The edit returns before encoding the card")
        try await waitUntil { qr.cachedRenderCount == 1 }
        let card = try XCTUnwrap(qr.cardImage)
        XCTAssertFalse(card === first)
        XCTAssertEqual(Self.decode(card), qr.text)
        qr.isEnlarged = true
        XCTAssertEqual(Self.decode(qr.cardImage), qr.text, "A new size for the same text is drawn at once")
        XCTAssertEqual(qr.cachedRenderCount, 2)
    }
}
