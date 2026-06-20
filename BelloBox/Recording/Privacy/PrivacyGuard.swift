import AppKit
import AVFoundation
import Foundation

enum SensitiveInputState: Equatable {
    case notSensitive
    case sensitiveKnownFrame(SensitiveFieldInfo)
    case sensitiveUnknownFrame(reason: SensitiveInputReason)
    case detectorUnavailable(reason: String)

    var isSensitive: Bool {
        switch self {
        case .notSensitive, .detectorUnavailable:
            return false
        case .sensitiveKnownFrame, .sensitiveUnknownFrame:
            return true
        }
    }
}

enum SensitiveInputReason: String, Equatable {
    case secureTextField
    case passwordLikeLabel
    case tokenLikeLabel
    case accessibilityUnavailable
    case unknownFocusedTextField
}

struct SensitiveFieldInfo: Equatable {
    let reason: SensitiveInputReason
    let frameInScreenPoints: CGRect?
    let owningAppBundleID: String?
    let confidence: Double
}

final class PrivacyGuard {
    private let detector: PasswordFieldDetecting
    private let options: RecordingOptions
    private var lastSensitiveState: SensitiveInputState = .notSensitive
    private var lastSensitiveTime: CMTime?
    private let hysteresisDuration = CMTime(seconds: 0.5, preferredTimescale: 600)

    init(detector: PasswordFieldDetecting, options: RecordingOptions) {
        self.detector = detector
        self.options = options
    }

    func update(now: CMTime) -> SensitiveInputState {
        let current = detector.currentSensitiveInputState()
        if current.isSensitive {
            lastSensitiveState = current
            lastSensitiveTime = now
            return current
        }

        if let lastSensitiveTime,
           CMTimeCompare(CMTimeSubtract(now, lastSensitiveTime), hysteresisDuration) <= 0 {
            return lastSensitiveState
        }

        lastSensitiveState = current
        return current
    }

    func shouldAllowPrintableKeyOverlay(now: CMTime) -> Bool {
        guard options.keystrokeMode == .allKeys || options.keystrokeMode == .maskedPrintable else { return false }
        guard AccessibilityService.isTrusted else { return false }
        return !update(now: now).isSensitive
    }

    func redactionState(now: CMTime) -> SensitiveInputState {
        update(now: now)
    }
}
