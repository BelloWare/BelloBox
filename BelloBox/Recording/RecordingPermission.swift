import AVFoundation
import Foundation

struct RecordingPermissionState: Equatable {
    var screenRecording: PermissionStatus
    var microphone: PermissionStatus
    var inputMonitoring: PermissionStatus
    var accessibility: PermissionStatus
    var systemAudio: PermissionStatus

    var canRecordVideo: Bool { screenRecording == .granted }

    static func current(options: RecordingOptions = .default) -> RecordingPermissionState {
        RecordingPermissionState(
            screenRecording: ScreenCapturePermission.isTrusted ? .granted : .notDetermined,
            microphone: options.audioSource.includesMicrophone ? MicrophonePermission.status() : .granted,
            inputMonitoring: (options.clickOverlayMode.isEnabled || options.keystrokeMode != .off)
                ? InputMonitoringPermission.status()
                : .granted,
            accessibility: AccessibilityService.isTrusted ? .granted : .notDetermined,
            systemAudio: options.audioSource.includesSystemAudio
                ? (ScreenCapturePermission.isTrusted ? .granted : .notDetermined)
                : .granted
        )
    }
}

enum PermissionStatus: Equatable {
    case granted
    case denied
    case notDetermined
    case unavailable(String)

    var isGranted: Bool { self == .granted }
}

enum MicrophonePermission {
    static func status() -> PermissionStatus {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            return .granted
        case .denied, .restricted:
            return .denied
        case .notDetermined:
            return .notDetermined
        @unknown default:
            return .unavailable("Unknown microphone permission status")
        }
    }

    static func request() async -> PermissionStatus {
        let granted = await AVCaptureDevice.requestAccess(for: .audio)
        return granted ? .granted : .denied
    }
}
