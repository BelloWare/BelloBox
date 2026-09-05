import XCTest
@testable import BelloBox

final class RecordingOutputTransactionTests: XCTestCase {
    private var directory: URL!
    private var destination: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        destination = directory.appendingPathComponent("existing.mov")
        try Data("previous recording".utf8).write(to: destination)
    }

    override func tearDownWithError() throws {
        try FileManager.default.removeItem(at: directory)
    }

    func testDiscardBeforePublicationOnlyRemovesOwnedFiles() throws {
        let transaction = RecordingOutputTransaction(destinationURL: destination)
        try Data("partial capture".utf8).write(to: transaction.captureURL)
        try Data("partial mix".utf8).write(to: transaction.mixedURL)
        transaction.discard()
        XCTAssertEqual(try String(contentsOf: destination, encoding: .utf8), "previous recording")
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path), ["existing.mov"])
    }

    func testLateExportCannotPublishAfterCancellation() throws {
        let transaction = RecordingOutputTransaction(destinationURL: destination)
        transaction.cancelPublication()
        try Data("late export".utf8).write(to: transaction.mixedURL)
        XCTAssertThrowsError(try transaction.publish(transaction.mixedURL)) { XCTAssertTrue($0 is CancellationError) }
        XCTAssertEqual(try String(contentsOf: destination, encoding: .utf8), "previous recording")
        transaction.discard()
        XCTAssertFalse(FileManager.default.fileExists(atPath: transaction.mixedURL.path))
    }

    func testSuccessfulPublicationReplacesDestinationAndCanBeDiscarded() throws {
        let transaction = RecordingOutputTransaction(destinationURL: destination)
        try Data("complete capture".utf8).write(to: transaction.captureURL)
        XCTAssertEqual(try transaction.publish(transaction.captureURL), RecordingFileStore.resolved(destination))
        XCTAssertEqual(try String(contentsOf: destination, encoding: .utf8), "complete capture")
        transaction.discard()
        XCTAssertTrue(try FileManager.default.contentsOfDirectory(atPath: directory.path).isEmpty)
    }

    func testRecordingsSharingDestinationHaveIndependentCaptureFiles() throws {
        let first = RecordingOutputTransaction(destinationURL: destination)
        let second = RecordingOutputTransaction(destinationURL: destination)
        XCTAssertNotEqual(first.captureURL, second.captureURL)
        try Data("second recording".utf8).write(to: second.captureURL)
        first.discard()
        XCTAssertEqual(try String(contentsOf: second.captureURL, encoding: .utf8), "second recording")
    }
}
