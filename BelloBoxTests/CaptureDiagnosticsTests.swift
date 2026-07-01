import XCTest
@testable import BelloBox

final class CaptureDiagnosticsTests: XCTestCase {
    func testWriteHonorsEnabledFlag() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxTests-\(UUID().uuidString)", isDirectory: true)
        let url = directory.appendingPathComponent("capture.log")
        defer { try? FileManager.default.removeItem(at: directory) }

        CaptureDiagnostics.write("disabled\n", enabled: false, to: url)
        XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))

        CaptureDiagnostics.write("enabled\n", enabled: true, to: url)
        let contents = try String(contentsOf: url, encoding: .utf8)
        XCTAssertEqual(contents, "enabled\n")
    }

    func testReadLogTailReturnsOnlyRecentBytesWhenLogIsLarge() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxTests-\(UUID().uuidString)", isDirectory: true)
        let url = directory.appendingPathComponent("capture.log")
        defer { try? FileManager.default.removeItem(at: directory) }

        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try "first-line\nsecond-line\nthird-line\n".write(to: url, atomically: true, encoding: .utf8)

        let tail = try XCTUnwrap(CaptureDiagnostics.readLogTail(maxBytes: 12, from: url))

        XCTAssertTrue(tail.contains("[last 12 bytes"))
        XCTAssertTrue(tail.contains("third-line"))
        XCTAssertFalse(tail.contains("first-line"))
    }
}
