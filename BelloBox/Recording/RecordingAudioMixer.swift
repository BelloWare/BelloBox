import AVFoundation
import Foundation

enum RecordingAudioMixerError: LocalizedError, Equatable {
    case noExportSession
    case unsupportedOutputType
    case exportFailed(String)

    var errorDescription: String? {
        switch self {
        case .noExportSession:
            return "Could not prepare the recording audio mix."
        case .unsupportedOutputType:
            return "The recording audio mix could not be exported as a movie."
        case let .exportFailed(message):
            return "The recording audio mix failed. \(message)"
        }
    }
}

enum RecordingAudioMixer {
    static func mixIfNeeded(sourceURL: URL, destinationURL: URL) async throws -> URL {
        try Task.checkCancellation()
        let sourceURL = RecordingFileStore.resolved(sourceURL)
        let destinationURL = RecordingFileStore.resolved(destinationURL)
        let asset = AVURLAsset(url: sourceURL)
        let audioTracks = try await asset.loadTracks(withMediaType: .audio)
        guard audioTracks.count > 1 || sourceURL != destinationURL else { return sourceURL }

        if audioTracks.count <= 1 {
            try RecordingFileStore.copy(from: sourceURL, to: destinationURL)
            if sourceURL != destinationURL {
                try? FileManager.default.removeItem(at: sourceURL)
            }
            return destinationURL
        }

        let composition = AVMutableComposition()
        let duration = try await asset.load(.duration)
        let fullRange = CMTimeRange(start: .zero, duration: duration)

        for sourceVideoTrack in try await asset.loadTracks(withMediaType: .video) {
            guard let videoTrack = composition.addMutableTrack(
                withMediaType: .video,
                preferredTrackID: kCMPersistentTrackID_Invalid
            ) else { continue }
            try videoTrack.insertTimeRange(fullRange, of: sourceVideoTrack, at: .zero)
            videoTrack.preferredTransform = try await sourceVideoTrack.load(.preferredTransform)
        }

        var audioMixParameters: [AVAudioMixInputParameters] = []
        for sourceAudioTrack in audioTracks {
            guard let audioTrack = composition.addMutableTrack(
                withMediaType: .audio,
                preferredTrackID: kCMPersistentTrackID_Invalid
            ) else { continue }
            try audioTrack.insertTimeRange(fullRange, of: sourceAudioTrack, at: .zero)
            let parameters = AVMutableAudioMixInputParameters(track: audioTrack)
            parameters.setVolume(1.0, at: .zero)
            audioMixParameters.append(parameters)
        }

        let audioMix = AVMutableAudioMix()
        audioMix.inputParameters = audioMixParameters

        let exportURL = RecordingFileStore.stagedURL(for: destinationURL)
        defer { try? FileManager.default.removeItem(at: exportURL) }
        guard let exportSession = AVAssetExportSession(asset: composition, presetName: AVAssetExportPresetHighestQuality) else {
            throw RecordingAudioMixerError.noExportSession
        }
        guard exportSession.supportedFileTypes.contains(.mov) else {
            throw RecordingAudioMixerError.unsupportedOutputType
        }

        exportSession.outputURL = exportURL
        exportSession.outputFileType = .mov
        exportSession.audioMix = audioMix
        exportSession.shouldOptimizeForNetworkUse = true

        let exportBox = ExportSessionBox(exportSession)
        try Task.checkCancellation()
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                exportBox.session.exportAsynchronously {
                    switch exportBox.session.status {
                    case .completed:
                        continuation.resume()
                    case .cancelled:
                        continuation.resume(throwing: CancellationError())
                    case .failed:
                        continuation.resume(
                            throwing: RecordingAudioMixerError.exportFailed(
                                exportBox.session.error?.localizedDescription ?? "Unknown export error."
                            )
                        )
                    default:
                        continuation.resume(
                            throwing: RecordingAudioMixerError.exportFailed("Export ended with status \(exportBox.session.status.rawValue).")
                        )
                    }
                }
            }
        } onCancel: {
            exportBox.cancel()
        }

        // Commit in the caller's task so cancellation is observed even if the
        // exporter reports completion just as the recording is cancelled.
        try RecordingFileStore.publish(exportURL, to: destinationURL)
        if sourceURL != destinationURL {
            try? FileManager.default.removeItem(at: sourceURL)
        }
        return destinationURL
    }
}

private final class ExportSessionBox: @unchecked Sendable {
    let session: AVAssetExportSession

    init(_ session: AVAssetExportSession) {
        self.session = session
    }

    func cancel() {
        session.cancelExport()
    }
}
