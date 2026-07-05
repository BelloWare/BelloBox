import XCTest
@testable import BelloBox

@MainActor
final class DebugInfoCollectorTests: XCTestCase {
    func testReportIncludesCoreTroubleshootingSections() {
        let settings = AppSettings(defaults: temporaryDefaults())
        settings.captureDiagnosticsEnabled = true
        settings.screenshotHotkeyEnabled = true

        let report = DebugInfoCollector.report(
            settings: settings,
            diagnosticsLogTail: "2026-07-01T00:00:00Z | overlay.ready | windowCount=2\n"
        )

        XCTAssertTrue(report.contains("== App =="))
        XCTAssertTrue(report.contains("== Permissions =="))
        XCTAssertTrue(report.contains("== Settings =="))
        XCTAssertTrue(report.contains("== Screens =="))
        XCTAssertTrue(report.contains("== Capture Self-Test =="))
        XCTAssertTrue(report.contains("captureDiagnosticsEnabled=true"))
        XCTAssertTrue(report.contains("screenshotHotkeyEnabled=true"))
        XCTAssertTrue(report.contains("screenshotCaptureEngine=auto"))
        XCTAssertTrue(report.contains("nsscreenCount="))
        XCTAssertTrue(report.contains("cgOnlineDisplayCount="))
        XCTAssertTrue(report.contains("sckAvailable:"))
        XCTAssertTrue(report.contains("verification:"))
        XCTAssertTrue(report.contains("chosenEngine:"))
        XCTAssertTrue(report.contains("overlay.ready"))
    }

    func testReportDoesNotIncludeAPIKey() {
        let settings = AppSettings(defaults: temporaryDefaults())

        let report = DebugInfoCollector.report(settings: settings, diagnosticsLogTail: nil)

        XCTAssertFalse(report.localizedCaseInsensitiveContains("apikey"))
    }

    private func temporaryDefaults() -> UserDefaults {
        let suiteName = "BelloBoxTests.DebugInfoCollector.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
