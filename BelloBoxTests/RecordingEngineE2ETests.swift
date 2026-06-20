import AppKit
@testable import BelloBox
import XCTest

final class RecordingEngineE2ETests: XCTestCase {
    func testAreaRecordingWritesMovieWhenEnabled() async throws {
        let sentinelPath = "/tmp/BELLOBOX_RUN_RECORDING_E2E"
        guard ProcessInfo.processInfo.environment["BELLOBOX_RUN_RECORDING_E2E"] == "1"
            || FileManager.default.fileExists(atPath: sentinelPath)
        else {
            throw XCTSkip("Set BELLOBOX_RUN_RECORDING_E2E=1 or create \(sentinelPath) to run the ScreenCaptureKit recording E2E test.")
        }
        guard ScreenCapturePermission.isTrusted else {
            throw XCTSkip("Screen Recording permission is required for the recording E2E test.")
        }
        guard let screen = NSScreen.main,
              let displayID = ScreenCoordinateSpace.displayID(for: screen)
        else {
            throw XCTSkip("No display is available for the recording E2E test.")
        }

        let rect = CGRect(
            x: screen.frame.midX - 160,
            y: screen.frame.midY - 100,
            width: 320,
            height: 200
        )
        let outputURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxRecordingE2E-\(UUID().uuidString).mov")
        let options = RecordingOptions(
            audioSource: .none,
            microphoneDeviceID: nil,
            includeCursor: false,
            clickOverlayMode: .off,
            keystrokeMode: .off,
            secureFieldRedactionMode: .strict,
            quality: .compact,
            countdownSeconds: 0,
            excludeBelloBoxWindows: true,
            excludesCurrentProcessAudio: true
        )
        let engine = RecordingEngine(
            target: .area(displayID: displayID, rectInScreenPoints: rect),
            options: options,
            outputURL: outputURL
        )

        _ = try await engine.start()
        try await Task.sleep(nanoseconds: 1_200_000_000)
        let movieURL = try await engine.stop()
        defer { try? FileManager.default.removeItem(at: movieURL) }

        let size = (try FileManager.default.attributesOfItem(atPath: movieURL.path)[.size] as? NSNumber)?.intValue ?? 0
        XCTAssertGreaterThan(size, 2_048)
    }
}
