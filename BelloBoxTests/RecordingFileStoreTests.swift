import XCTest
@testable import BelloBox

final class RecordingFileStoreTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try FileManager.default.removeItem(at: directory)
    }

    func testFailedCopyPreservesExistingDestination() throws {
        let destination = directory.appendingPathComponent("saved.mov")
        try Data("original".utf8).write(to: destination)
        XCTAssertThrowsError(try RecordingFileStore.copy(
            from: directory.appendingPathComponent("missing.mov"), to: destination))
        XCTAssertEqual(try String(contentsOf: destination, encoding: .utf8), "original")
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path), ["saved.mov"])
    }

    func testFailedPublishPreservesDestinationDirectoryAndCleansStagingFile() throws {
        let destination = directory.appendingPathComponent("saved.mov", isDirectory: true)
        try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
        let source = directory.appendingPathComponent("source.mov")
        try Data("movie".utf8).write(to: source)
        let child = destination.appendingPathComponent("keep.txt")
        try Data("keep".utf8).write(to: child)
        XCTAssertThrowsError(try RecordingFileStore.copy(from: source, to: destination))
        XCTAssertEqual(try String(contentsOf: child, encoding: .utf8), "keep")
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path).sorted(), ["saved.mov", "source.mov"])
    }

    func testSymlinkToSourceDoesNotDeleteRecording() throws {
        let source = directory.appendingPathComponent("source.mov")
        let alias = directory.appendingPathComponent("alias.mov")
        try Data("recording".utf8).write(to: source)
        try FileManager.default.createSymbolicLink(at: alias, withDestinationURL: source)
        try RecordingFileStore.copy(from: source, to: alias)
        XCTAssertEqual(try String(contentsOf: source, encoding: .utf8), "recording")
        XCTAssertEqual(try FileManager.default.destinationOfSymbolicLink(atPath: alias.path), source.path)
    }

    func testCancelledPublishPreservesOriginalAndDoesNotMoveStagingFile() async throws {
        let destination = directory.appendingPathComponent("saved.mov")
        let staged = RecordingFileStore.stagedURL(for: destination)
        try Data("original".utf8).write(to: destination)
        try Data("replacement".utf8).write(to: staged)
        let task = Task {
            withUnsafeCurrentTask { $0?.cancel() }
            try RecordingFileStore.publish(staged, to: destination)
        }
        do {
            try await task.value
            XCTFail("Cancelled exports must not publish")
        } catch is CancellationError {
            XCTAssertEqual(try String(contentsOf: destination, encoding: .utf8), "original")
            XCTAssertEqual(try String(contentsOf: staged, encoding: .utf8), "replacement")
        }
    }
}
