import AppKit
import AVFoundation
import XCTest
@testable import BelloBox

final class RecordingRecoveryTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try FileManager.default.removeItem(at: directory)
    }

    func testCombinedAudioRecordingStillPublishesWhenNoAudioSamplesArrive() async throws {
        let destination = directory.appendingPathComponent("recording.mov")
        try Data("older recording".utf8).write(to: destination)
        var options = RecordingOptions.default
        options.audioSource = .microphoneAndSystemAudio
        let engine = RecordingEngine(target: .display(displayID: 1), options: options, outputURL: destination)
        try startSyntheticRecording(engine)
        try await Task.sleep(nanoseconds: 300_000_000)
        let saved = try await engine.stop()
        XCTAssertEqual(saved, RecordingFileStore.resolved(destination))
        try await assertPlayable(saved)
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path), ["recording.mov"])
    }

    func testFailedAudioMixPreservesPlayableCaptureAndOlderDestination() async throws {
        let destination = directory.appendingPathComponent("recording.mov")
        try Data("older recording".utf8).write(to: destination)
        var options = RecordingOptions.default
        options.audioSource = .microphoneAndSystemAudio
        let engine = RecordingEngine(target: .display(displayID: 1), options: options, outputURL: destination,
            mixAudio: { _, _ in throw RecordingAudioMixerError.exportFailed("Injected mixer failure") })
        try startSyntheticRecording(engine)
        try await Task.sleep(nanoseconds: 300_000_000)
        do {
            _ = try await engine.stop()
            XCTFail("Expected a recoverable export failure")
        } catch let RecordingEngineError.recoverableExportFailure(recovered, message) {
            XCTAssertTrue(message.contains("Injected mixer failure"))
            try await assertPlayable(recovered)
            XCTAssertEqual(try String(contentsOf: destination, encoding: .utf8), "older recording")
        }
    }

    func testFailedFinalRenamePreservesPlayableCaptureAndDestinationDirectory() async throws {
        let destination = directory.appendingPathComponent("recording.mov", isDirectory: true)
        try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
        let child = destination.appendingPathComponent("keep.txt")
        try Data("keep".utf8).write(to: child)
        var options = RecordingOptions.default
        options.audioSource = .none
        let engine = RecordingEngine(target: .display(displayID: 1), options: options, outputURL: destination)
        try startSyntheticRecording(engine)
        try await Task.sleep(nanoseconds: 300_000_000)
        do {
            _ = try await engine.stop()
            XCTFail("Expected a recoverable rename failure")
        } catch let RecordingEngineError.recoverableExportFailure(recovered, _) {
            try await assertPlayable(recovered)
            XCTAssertEqual(try String(contentsOf: child, encoding: .utf8), "keep")
        }
    }

    func testCancelDuringMixRejectsLateMovieAndPreservesOlderDestination() async throws {
        let destination = directory.appendingPathComponent("recording.mov")
        try Data("older recording".utf8).write(to: destination)
        var options = RecordingOptions.default
        options.audioSource = .microphoneAndSystemAudio
        let mixer = SuspendedTestMixer()
        let engine = RecordingEngine(target: .display(displayID: 1), options: options, outputURL: destination,
            mixAudio: { _, destination in try await mixer.mix(to: destination) })
        try startSyntheticRecording(engine)
        try await Task.sleep(nanoseconds: 300_000_000)
        let stopTask = Task { try await engine.stop() }
        await fulfillment(of: [mixer.started], timeout: 3)
        engine.cancel()
        await mixer.complete()
        do {
            _ = try await stopTask.value
            XCTFail("A cancelled recording must not publish a late export")
        } catch is CancellationError {}
        XCTAssertEqual(try String(contentsOf: destination, encoding: .utf8), "older recording")
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path), ["recording.mov"])
    }

    private func startSyntheticRecording(_ engine: RecordingEngine) throws {
        var buffer: CVPixelBuffer?
        XCTAssertEqual(CVPixelBufferCreate(kCFAllocatorDefault, 320, 200, kCVPixelFormatType_32BGRA,
            [kCVPixelBufferIOSurfacePropertiesKey as String: [:]] as CFDictionary, &buffer), kCVReturnSuccess)
        let frame = try XCTUnwrap(buffer)
        CVPixelBufferLockBaseAddress(frame, [])
        memset(CVPixelBufferGetBaseAddress(frame), 255, CVPixelBufferGetBytesPerRow(frame) * 200)
        CVPixelBufferUnlockBaseAddress(frame, [])
        try engine.debugStartWithFrame(frame, output: RecordingOutputSettings.make(for: CGSize(width: 320, height: 200), quality: .compact))
    }

    private func assertPlayable(_ url: URL) async throws {
        let asset = AVURLAsset(url: url)
        let video = try await asset.loadTracks(withMediaType: .video)
        let duration = try await asset.load(.duration)
        XCTAssertEqual(video.count, 1)
        XCTAssertGreaterThan(duration.seconds, 0.1)
        let generator = AVAssetImageGenerator(asset: asset)
        let frame = try generator.copyCGImage(at: .zero, actualTime: nil)
        XCTAssertEqual(frame.width, 320)
        XCTAssertEqual(frame.height, 200)
    }
}

private actor SuspendedTestMixer {
    nonisolated let started = XCTestExpectation(description: "Mixer started")
    private var continuation: CheckedContinuation<Void, Never>?

    func mix(to destination: URL) async throws -> URL {
        await withCheckedContinuation { continuation in
            self.continuation = continuation
            started.fulfill()
        }
        // Deliberately ignore cancellation, like an encoder whose completion was
        // already queued when the user discarded the recording.
        try Data("late completed export".utf8).write(to: destination)
        return destination
    }

    func complete() {
        continuation?.resume()
        continuation = nil
    }
}
