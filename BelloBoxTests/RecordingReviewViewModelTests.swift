import XCTest
import SwiftUI
@testable import BelloBox

@MainActor
final class RecordingReviewViewModelTests: XCTestCase {
    func testRecordingReviewCanLoadItsNativePlayerControls() {
        let viewModel = RecordingReviewViewModel(fileURL: URL(fileURLWithPath: "/unused-recording.mov"))
        let view = NSHostingView(rootView: RecordingReviewView(viewModel: viewModel))
        view.frame = CGRect(origin: .zero, size: RecordingReviewView.preferredSize)
        view.layoutSubtreeIfNeeded()
        XCTAssertGreaterThan(view.fittingSize.width, 0)
    }

    func testCopyRecordingReplacesExistingDestination() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxTests.\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let source = directory.appendingPathComponent("source.mov")
        let destination = directory.appendingPathComponent("destination.mov")
        try Data("new recording".utf8).write(to: source)
        try Data("stale recording".utf8).write(to: destination)

        let viewModel = RecordingReviewViewModel(fileURL: source)
        try viewModel.copyRecording(to: destination)

        let copied = try String(contentsOf: destination, encoding: .utf8)
        XCTAssertEqual(copied, "new recording")
    }

    func testCopyRecordingToSameFileDoesNotDeleteSource() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxTests.\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let source = directory.appendingPathComponent("source.mov")
        try Data("keep me".utf8).write(to: source)

        let viewModel = RecordingReviewViewModel(fileURL: source)
        try viewModel.copyRecording(to: source)

        let existing = try String(contentsOf: source, encoding: .utf8)
        XCTAssertEqual(existing, "keep me")
    }

    func testAsyncSaveReportsSuccessAndKeepsOriginalRecording() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = directory.appendingPathComponent("source.mov")
        let destination = directory.appendingPathComponent("saved.mov")
        try Data("recording".utf8).write(to: source)
        let viewModel = RecordingReviewViewModel(fileURL: source)
        await viewModel.saveRecording(to: destination)
        XCTAssertFalse(viewModel.isSaving)
        XCTAssertNil(viewModel.errorMessage)
        XCTAssertEqual(viewModel.statusMessage, "Saved to saved.mov.")
        XCTAssertEqual(try Data(contentsOf: source), try Data(contentsOf: destination))
    }

    func testAsyncSaveFailureRetainsPreviousFileAndAllowsRetry() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = directory.appendingPathComponent("missing.mov")
        let destination = directory.appendingPathComponent("saved.mov")
        try Data("previous".utf8).write(to: destination)
        let viewModel = RecordingReviewViewModel(fileURL: source)
        await viewModel.saveRecording(to: destination)
        XCTAssertFalse(viewModel.isSaving)
        XCTAssertNotNil(viewModel.errorMessage)
        XCTAssertNil(viewModel.statusMessage)
        XCTAssertEqual(try String(contentsOf: destination, encoding: .utf8), "previous")
        try Data("retry".utf8).write(to: source)
        await viewModel.saveRecording(to: destination)
        XCTAssertNil(viewModel.errorMessage)
        XCTAssertEqual(try String(contentsOf: destination, encoding: .utf8), "retry")
    }

    func testDiscardFailureKeepsReviewOpenAndShowsError() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxTests.\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let source = directory.appendingPathComponent("source.mov")
        try Data("recording".utf8).write(to: source)
        let viewModel = RecordingReviewViewModel(
            fileURL: source,
            removeRecording: { _ in throw CocoaError(.fileWriteNoPermission) }
        )
        var closeCount = 0
        viewModel.onClose = { closeCount += 1 }

        viewModel.requestDiscard()
        viewModel.discard()

        XCTAssertEqual(closeCount, 0)
        XCTAssertTrue(FileManager.default.fileExists(atPath: source.path))
        XCTAssertTrue(viewModel.errorMessage?.hasPrefix("Could not move recording to Trash:") == true)
    }

    func testDiscardSuccessClosesReview() throws {
        let source = FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxTests-\(UUID().uuidString).mov")
        let viewModel = RecordingReviewViewModel(fileURL: source, removeRecording: { _ in })
        var closeCount = 0
        viewModel.onClose = { closeCount += 1 }

        viewModel.requestDiscard()
        // SwiftUI may dismiss the alert binding before invoking its action.
        viewModel.showDiscardConfirmation = false
        viewModel.discard()

        XCTAssertNil(viewModel.errorMessage)
        XCTAssertEqual(closeCount, 1)
    }

    func testDiscardRequiresConfirmationAndCancelKeepsTheFile() {
        var removalCount = 0
        let viewModel = RecordingReviewViewModel(
            fileURL: URL(fileURLWithPath: "/unused-recording.mov"),
            removeRecording: { _ in removalCount += 1 }
        )
        viewModel.discard()
        XCTAssertEqual(removalCount, 0)
        viewModel.requestDiscard()
        XCTAssertTrue(viewModel.showDiscardConfirmation)
        XCTAssertEqual(removalCount, 0)
        viewModel.cancelDiscard()
        viewModel.discard()
        XCTAssertFalse(viewModel.showDiscardConfirmation)
        XCTAssertEqual(removalCount, 0)
    }
}
