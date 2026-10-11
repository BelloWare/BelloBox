import CoreGraphics
import XCTest
@testable import BelloBox

/// Undo history is bounded like the Rust editor's: at most 64 steps and about
/// 64 MiB of retained snapshots; the oldest steps go first.
@MainActor
final class ScreenshotUndoHistoryTests: XCTestCase {
    private let suiteName = "BelloBoxTests.ScreenshotUndoHistory.\(UUID().uuidString)"

    override func tearDown() {
        UserDefaults(suiteName: suiteName)?.removePersistentDomain(forName: suiteName)
        super.tearDown()
    }

    private func viewModel() -> ScreenshotPopupViewModel {
        ScreenshotPopupViewModel(
            document: ScreenshotDocument(baseImage: ScreenshotTestHelpers.image(width: 80, height: 60), scale: 1, source: .importedClipboard),
            settings: AppSettings(defaults: UserDefaults(suiteName: suiteName)!)
        )
    }

    private func undoAll(_ viewModel: ScreenshotPopupViewModel) -> Int {
        var steps = 0
        while viewModel.canUndo { viewModel.undo(); steps += 1 }
        return steps
    }

    func testUndoKeepsTheLatest64Steps() {
        let viewModel = viewModel()
        for index in 0..<100 {
            viewModel.addVisibleAnnotation(.rectangle(CGRect(x: index % 40, y: 2, width: 10, height: 10)))
        }
        XCTAssertEqual(undoAll(viewModel), 64, "0.0.77 kept every step")
        XCTAssertEqual(viewModel.document.annotations.count, 36, "The oldest steps were dropped, the latest kept")
        var redone = 0
        while viewModel.canRedo { viewModel.redo(); redone += 1 }
        XCTAssertEqual(redone, 64)
        XCTAssertEqual(viewModel.document.annotations.count, 100)
    }

    func testUndoHistoryStaysWithinItsByteBudget() {
        // Each snapshot holds every earlier stroke, so long strokes add up fast.
        let viewModel = viewModel()
        let stroke = (0..<100_000).map { CGPoint(x: CGFloat($0 % 80), y: CGFloat($0 % 60)) }
        for _ in 0..<30 { viewModel.addVisibleAnnotation(.freehand(points: stroke)) }
        XCTAssertLessThanOrEqual(viewModel.undoHistoryBytes, ScreenshotPopupViewModel.undoHistoryByteLimit)
        let steps = undoAll(viewModel)
        XCTAssertGreaterThan(steps, 0)
        XCTAssertLessThan(steps, 30, "0.0.77 kept all 30 snapshots (~0.7 GB by this count)")
        XCTAssertEqual(viewModel.document.annotations.count, 30 - steps, "The newest steps are the ones kept")
    }
}
