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
        let asset = AVURLAsset(url: sourceURL)
        let audioTracks = asset.tracks(withMediaType: .audio)
        guard audioTracks.count > 1 || sourceURL != destinationURL else { return sourceURL }

        if audioTracks.count <= 1 {
            try? FileManager.default.removeItem(at: destinationURL)
            try FileManager.default.copyItem(at: sourceURL, to: destinationURL)
            if sourceURL != destinationURL {
                try? FileManager.default.removeItem(at: sourceURL)
            }
            return destinationURL
        }

        let composition = AVMutableComposition()
        let fullRange = CMTimeRange(start: .zero, duration: asset.duration)

        for sourceVideoTrack in asset.tracks(withMediaType: .video) {
            guard let videoTrack = composition.addMutableTrack(
                withMediaType: .video,
                preferredTrackID: kCMPersistentTrackID_Invalid
            ) else { continue }
            try videoTrack.insertTimeRange(fullRange, of: sourceVideoTrack, at: .zero)
            videoTrack.preferredTransform = sourceVideoTrack.preferredTransform
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

        try? FileManager.default.removeItem(at: destinationURL)
        guard let exportSession = AVAssetExportSession(asset: composition, presetName: AVAssetExportPresetHighestQuality) else {
            throw RecordingAudioMixerError.noExportSession
        }
        guard exportSession.supportedFileTypes.contains(.mov) else {
            throw RecordingAudioMixerError.unsupportedOutputType
        }

        exportSession.outputURL = destinationURL
        exportSession.outputFileType = .mov
        exportSession.audioMix = audioMix
        exportSession.shouldOptimizeForNetworkUse = true

        try await withCheckedThrowingContinuation { continuation in
            exportSession.exportAsynchronously {
                switch exportSession.status {
                case .completed:
                    if sourceURL != destinationURL {
                        try? FileManager.default.removeItem(at: sourceURL)
                    }
                    continuation.resume()
                case .failed, .cancelled:
                    try? FileManager.default.removeItem(at: destinationURL)
                    continuation.resume(
                        throwing: RecordingAudioMixerError.exportFailed(
                            exportSession.error?.localizedDescription ?? "Unknown export error."
                        )
                    )
                default:
                    try? FileManager.default.removeItem(at: destinationURL)
                    continuation.resume(
                        throwing: RecordingAudioMixerError.exportFailed("Export ended with status \(exportSession.status.rawValue).")
                    )
                }
            }
        }

        return destinationURL
    }
}
