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
}
