import AVFoundation

struct RecordingMicrophoneDevice: Identifiable, Equatable {
    let id: String
    let name: String
}

enum RecordingMicrophoneDevices {
    static func available() -> [RecordingMicrophoneDevice] {
        availableAudioDevices()
            .map { RecordingMicrophoneDevice(id: $0.uniqueID, name: $0.localizedName) }
            .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    static func device(matching deviceID: String?) -> AVCaptureDevice? {
        let devices = availableAudioDevices()
        if let deviceID, let selected = devices.first(where: { $0.uniqueID == deviceID }) {
            return selected
        }
        return AVCaptureDevice.default(for: .audio)
    }

    private static func availableAudioDevices() -> [AVCaptureDevice] {
        if #available(macOS 14.0, *) {
            return AVCaptureDevice.DiscoverySession(
                deviceTypes: [.microphone],
                mediaType: .audio,
                position: .unspecified
            ).devices
        }

        // On macOS 13, DiscoverySession only exposes built-in microphones for
        // audio; this deprecated API is the available path that still includes
        // USB and other external input devices.
        return AVCaptureDevice.devices(for: .audio)
    }
}
