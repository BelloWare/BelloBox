import AVFoundation
import XCTest
@testable import BelloBox

final class RecordingAudioMixerTests: XCTestCase {
    func testTwoAudioTracksAreMixedIntoPlayableMovieBeforeReplacingDestination() async throws {
        guard ProcessInfo.processInfo.environment["BELLOBOX_RUN_AUDIO_MIX_E2E"] == "1" else {
            throw XCTSkip("Set BELLOBOX_RUN_AUDIO_MIX_E2E=1 on a Mac with a working Core Audio device to run the audio mixing E2E test.")
        }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let audioURL = directory.appendingPathComponent("tone.caf")
        let format = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: 44_100, channels: 1))
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 22_050))
        buffer.frameLength = buffer.frameCapacity
        let channel = try XCTUnwrap(buffer.floatChannelData?[0])
        for index in 0..<Int(buffer.frameLength) {
            channel[index] = Float(sin(Double(index) * 2 * .pi * 440 / 44_100) * 0.1)
        }
        // Closing the audio file finalizes its header before the asset reads it.
        do {
            let audioFile = try AVAudioFile(forWriting: audioURL, settings: format.settings)
            try audioFile.write(from: buffer)
        }
        let audioAsset = AVURLAsset(url: audioURL)
        let audioTracks = try await audioAsset.loadTracks(withMediaType: .audio)
        let audioTrack = try XCTUnwrap(audioTracks.first)
        let composition = AVMutableComposition()
        let range = CMTimeRange(start: .zero, duration: CMTime(seconds: 0.5, preferredTimescale: 44_100))
        for _ in 0..<2 {
            let track = try XCTUnwrap(composition.addMutableTrack(withMediaType: .audio,
                preferredTrackID: kCMPersistentTrackID_Invalid))
            try track.insertTimeRange(range, of: audioTrack, at: .zero)
        }
        let videoURL = directory.appendingPathComponent("video.mov")
        var options = RecordingOptions.default
        options.audioSource = .none
        options.keystrokeMode = .off
        options.clickOverlayMode = .off
        let engine = RecordingEngine(target: .display(displayID: 1), options: options, outputURL: videoURL)
        var pixelBuffer: CVPixelBuffer?
        XCTAssertEqual(CVPixelBufferCreate(kCFAllocatorDefault, 320, 200, kCVPixelFormatType_32BGRA,
            [kCVPixelBufferIOSurfacePropertiesKey as String: [:]] as CFDictionary, &pixelBuffer), kCVReturnSuccess)
        let frame = try XCTUnwrap(pixelBuffer)
        CVPixelBufferLockBaseAddress(frame, [])
        memset(CVPixelBufferGetBaseAddress(frame), 255, CVPixelBufferGetBytesPerRow(frame) * 200)
        CVPixelBufferUnlockBaseAddress(frame, [])
        try engine.debugStartWithFrame(frame, output: RecordingOutputSettings.make(
            for: CGSize(width: 320, height: 200), quality: .compact))
        defer { engine.cancel() }
        try await Task.sleep(nanoseconds: 550_000_000)
        _ = try await engine.stop()
        let videoAsset = AVURLAsset(url: videoURL)
        let videoTracks = try await videoAsset.loadTracks(withMediaType: .video)
        let videoTrack = try XCTUnwrap(videoTracks.first)
        let compositionVideo = try XCTUnwrap(composition.addMutableTrack(withMediaType: .video,
            preferredTrackID: kCMPersistentTrackID_Invalid))
        try withExtendedLifetime(videoAsset) {
            try compositionVideo.insertTimeRange(range, of: videoTrack, at: .zero)
        }
        let source = directory.appendingPathComponent("tracks.mov")
        let exporter = try XCTUnwrap(AVAssetExportSession(asset: composition, presetName: AVAssetExportPresetPassthrough))
        exporter.outputURL = source
        exporter.outputFileType = .mov
        await withCheckedContinuation { continuation in
            exporter.exportAsynchronously { continuation.resume() }
        }
        XCTAssertEqual(exporter.status, .completed, exporter.error?.localizedDescription ?? "")
        let inputTracks = try await AVURLAsset(url: source).loadTracks(withMediaType: .audio)
        XCTAssertEqual(inputTracks.count, 2, "The fixture must exercise the audio mixing path")
        let destination = directory.appendingPathComponent("saved.mov")
        try Data("previous recording".utf8).write(to: destination)
        let result = try await RecordingAudioMixer.mixIfNeeded(sourceURL: source, destinationURL: destination)
        XCTAssertEqual(result, RecordingFileStore.resolved(destination))
        let output = AVURLAsset(url: result)
        let outputTracks = try await output.loadTracks(withMediaType: .audio)
        let outputVideo = try await output.loadTracks(withMediaType: .video)
        XCTAssertEqual(outputVideo.count, 1)
        let duration = try await output.load(.duration)
        XCTAssertEqual(outputTracks.count, 1)
        XCTAssertGreaterThan(duration.seconds, 0.4)
        XCTAssertFalse(FileManager.default.fileExists(atPath: source.path))
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path).sorted(), ["saved.mov", "tone.caf", "video.mov"])
    }

    func testInvalidMovieLeavesSourceAndExistingDestinationIntact() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = directory.appendingPathComponent("source.mov")
        let destination = directory.appendingPathComponent("destination.mov")
        try Data("incomplete movie".utf8).write(to: source)
        try Data("previous recording".utf8).write(to: destination)
        do {
            _ = try await RecordingAudioMixer.mixIfNeeded(sourceURL: source, destinationURL: destination)
            XCTFail("An invalid movie must fail to export")
        } catch {
            XCTAssertEqual(try String(contentsOf: source, encoding: .utf8), "incomplete movie")
            XCTAssertEqual(try String(contentsOf: destination, encoding: .utf8), "previous recording")
            XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: directory.path).count, 2)
        }
    }
}
