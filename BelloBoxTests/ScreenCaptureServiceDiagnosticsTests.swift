import AppKit
import XCTest
@testable import BelloBox

@MainActor
final class ScreenCaptureServiceDiagnosticsTests: XCTestCase {
    func testDisplayCaptureDiagnosticsUsesSettingsFlag() {
        var writes: [(event: String, enabled: Bool, details: [String])] = []
        let service = ScreenCaptureService(
            diagnosticsEnabledProvider: { false },
            diagnosticsWriter: { event, enabled, details in
                writes.append((event, enabled, details))
            }
        )

        service.debugLogDisplayCaptureForTesting("displayCapture.test", details: ["key=value"])

        XCTAssertEqual(writes.count, 1)
        XCTAssertEqual(writes[0].event, "displayCapture.test")
        XCTAssertFalse(writes[0].enabled)
        XCTAssertEqual(writes[0].details, ["key=value"])
    }

    func testScreenParameterInvalidationDiagnosticsUsesSettingsFlag() async {
        var writes: [(event: String, enabled: Bool, details: [String])] = []
        var diagnosticsEnabled = false
        let service = ScreenCaptureService(
            diagnosticsEnabledProvider: { diagnosticsEnabled },
            diagnosticsWriter: { event, enabled, details in
                writes.append((event, enabled, details))
            }
        )
        _ = service

        NotificationCenter.default.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
        await waitUntil(writes.count == 1)

        XCTAssertEqual(writes.first?.event, "displayCapture.verify.cacheInvalidated")
        XCTAssertFalse(writes.first?.enabled ?? true)

        diagnosticsEnabled = true
        NotificationCenter.default.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
        await waitUntil(writes.count == 2)

        XCTAssertEqual(writes.last?.event, "displayCapture.verify.cacheInvalidated")
        XCTAssertTrue(writes.last?.enabled ?? false)
    }

    private func waitUntil(
        _ predicate: @autoclosure @escaping @MainActor () -> Bool,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        for _ in 0..<100 {
            if predicate() { return }
            try? await Task.sleep(nanoseconds: 1_000_000)
        }
        XCTFail("Timed out waiting for condition", file: file, line: line)
    }
}
