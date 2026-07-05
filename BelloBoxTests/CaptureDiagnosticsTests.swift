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

    func testWriteTrimsLogToRecentWholeLinesWhenCapIsExceeded() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxTests-\(UUID().uuidString)", isDirectory: true)
        let url = directory.appendingPathComponent("capture.log")
        defer { try? FileManager.default.removeItem(at: directory) }

        let text = (1...8)
            .map { "line-\($0)-xxxxxxxx\n" }
            .joined()

        CaptureDiagnostics.write(text, enabled: true, to: url, maximumLogBytes: 90, retainedLogBytes: 45)

        let contents = try String(contentsOf: url, encoding: .utf8)
        XCTAssertEqual(contents, "line-7-xxxxxxxx\nline-8-xxxxxxxx\n")
        XCTAssertLessThanOrEqual(contents.utf8.count, 45)
    }
}
