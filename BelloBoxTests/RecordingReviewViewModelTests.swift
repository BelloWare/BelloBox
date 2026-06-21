import XCTest
@testable import BelloBox

@MainActor
final class RecordingReviewViewModelTests: XCTestCase {
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
}
