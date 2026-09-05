import AppKit
import AVFoundation
@testable import BelloBox
import XCTest

final class RecordingEngineE2ETests: XCTestCase {
    func testStaticScreenMovieKeepsDurationAndLiveKeyboardToggle() async throws {
        let temporaryDirectory = ProcessInfo.processInfo.environment["BELLOBOX_TEST_TMP_DIR"]
            .map { URL(fileURLWithPath: $0, isDirectory: true) } ?? FileManager.default.temporaryDirectory
        let outputURL = temporaryDirectory.appendingPathComponent("BelloBoxStaticRecording-\(UUID().uuidString).mov")
        defer { try? FileManager.default.removeItem(at: outputURL) }
        var options = RecordingOptions.default
        options.audioSource = .none
        options.keystrokeMode = .allKeys
        options.clickOverlayMode = .off
        let engine = RecordingEngine(target: .display(displayID: 1), options: options, outputURL: outputURL)
        var buffer: CVPixelBuffer?
        XCTAssertEqual(CVPixelBufferCreate(kCFAllocatorDefault, 320, 200, kCVPixelFormatType_32BGRA,
            [kCVPixelBufferIOSurfacePropertiesKey as String: [:]] as CFDictionary, &buffer), kCVReturnSuccess)
        let source = try XCTUnwrap(buffer)
        CVPixelBufferLockBaseAddress(source, [])
        memset(CVPixelBufferGetBaseAddress(source), 255, CVPixelBufferGetBytesPerRow(source) * 200)
        CVPixelBufferUnlockBaseAddress(source, [])
        try engine.debugStartWithFrame(source, output: RecordingOutputSettings.make(
            for: CGSize(width: 320, height: 200), quality: .compact))
        defer { engine.cancel() }
        let time = CMClockGetTime(CMClockGetHostTimeClock())
        engine.debugAddOverlayEvent(TimedOverlayEvent(id: UUID(), time: time,
            kind: .keystroke(KeystrokeOverlayEvent(displayLabel: "Test", isShortcut: false, isPrintable: true, modifiers: [])),
            expiresAt: CMTimeAdd(time, CMTime(seconds: 5, preferredTimescale: 600))))
        try await Task.sleep(nanoseconds: 450_000_000)
        XCTAssertTrue(engine.updateInputOverlays(clicks: .off, keys: .off))
        try await Task.sleep(nanoseconds: 450_000_000)
        let movie = try await engine.stop()
        let asset = AVURLAsset(url: movie)
        let duration = try await asset.load(.duration)
        XCTAssertGreaterThan(CMTimeGetSeconds(duration), 0.7, "A static screen must not collapse to a single-frame movie")
        let generator = AVAssetImageGenerator(asset: asset)
        generator.requestedTimeToleranceBefore = .zero
        generator.requestedTimeToleranceAfter = CMTime(seconds: 0.1, preferredTimescale: 600)
        let enabled = try generator.copyCGImage(at: CMTime(seconds: 0.3, preferredTimescale: 600), actualTime: nil)
        let disabled = try generator.copyCGImage(at: CMTime(seconds: 0.7, preferredTimescale: 600), actualTime: nil)
        func darkPixels(_ image: CGImage) -> Int {
            (130..<170).reduce(0) { count, y in
                count + (115..<205).filter { ScreenshotTestHelpers.pixel(image, x: $0, y: y)[0] < 180 }.count
            }
        }
        XCTAssertGreaterThan(darkPixels(enabled), 300, "The key bubble must be encoded even without another screen image")
        XCTAssertEqual(darkPixels(disabled), 0, "Turning tracking off must remove the pending bubble from the movie")
    }

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
