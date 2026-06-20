import AppKit
import AVFoundation
import CoreGraphics
import Foundation
import ScreenCaptureKit

struct RecordingSessionID: Hashable, Codable {
    let rawValue: UUID

    init(rawValue: UUID = UUID()) {
        self.rawValue = rawValue
    }
}

enum RecordingCaptureMode: String, Codable, CaseIterable, Identifiable {
    case area
    case window
    case display

    var id: String { rawValue }

    var label: String {
        switch self {
        case .area: return "Area"
        case .window: return "Window"
        case .display: return "Screen"
        }
    }

    var symbol: String {
        switch self {
        case .area: return "selection.pin.in.out"
        case .window: return "macwindow"
        case .display: return "display"
        }
    }
}

enum RecordingTarget: Equatable {
    case area(displayID: CGDirectDisplayID, rectInScreenPoints: CGRect)
    case window(windowID: CGWindowID, displayID: CGDirectDisplayID?, frameInScreenPoints: CGRect?)
    case display(displayID: CGDirectDisplayID)
}

enum RecordingAudioSource: String, Codable, CaseIterable, Identifiable {
    case none
    case microphone
    case systemAudio
    case microphoneAndSystemAudio

    var id: String { rawValue }

    var label: String {
        switch self {
        case .none: return "None"
        case .microphone: return "Microphone"
        case .systemAudio: return "Mac Audio"
        case .microphoneAndSystemAudio: return "Mic + Mac Audio"
        }
    }

    var includesMicrophone: Bool {
        self == .microphone || self == .microphoneAndSystemAudio
    }

    var includesSystemAudio: Bool {
        self == .systemAudio || self == .microphoneAndSystemAudio
    }
}

enum KeystrokeCaptureMode: String, Codable, CaseIterable, Identifiable {
    case off
    case shortcutsOnly
    case maskedPrintable
    case allKeys

    var id: String { rawValue }

    var label: String {
        switch self {
        case .off: return "Off"
        case .shortcutsOnly: return "Shortcuts only"
        case .maskedPrintable: return "Masked printable keys"
        case .allKeys: return "All keys"
        }
    }
}

enum ClickOverlayMode: String, Codable, CaseIterable, Identifiable {
    case off
    case ringsOnly
    case ringsAndLabels

    var id: String { rawValue }

    var label: String {
        switch self {
        case .off: return "Off"
        case .ringsOnly: return "Click rings"
        case .ringsAndLabels: return "Click rings + labels"
        }
    }

    var isEnabled: Bool { self != .off }
}

enum SecureFieldRedactionMode: String, Codable, CaseIterable, Identifiable {
    case strict
    case balanced
    case visualFieldOnly

    var id: String { rawValue }

    var label: String {
        switch self {
        case .strict: return "Strict"
        case .balanced: return "Balanced"
        case .visualFieldOnly: return "Visual field only"
        }
    }
}

enum RecordingQualityPreset: String, Codable, CaseIterable, Identifiable {
    case compact
    case balanced
    case high

    var id: String { rawValue }

    var label: String {
        switch self {
        case .compact: return "Compact"
        case .balanced: return "Balanced"
        case .high: return "High"
        }
    }
}

struct RecordingOptions: Equatable, Codable {
    var audioSource: RecordingAudioSource
    var microphoneDeviceID: String?
    var includeCursor: Bool
    var clickOverlayMode: ClickOverlayMode
    var keystrokeMode: KeystrokeCaptureMode
    var secureFieldRedactionMode: SecureFieldRedactionMode
    var quality: RecordingQualityPreset
    var countdownSeconds: Int
    var excludeBelloBoxWindows: Bool
    var excludesCurrentProcessAudio: Bool

    static let `default` = RecordingOptions(
        audioSource: .none,
        microphoneDeviceID: nil,
        includeCursor: true,
        clickOverlayMode: .ringsAndLabels,
        keystrokeMode: .shortcutsOnly,
        secureFieldRedactionMode: .strict,
        quality: .balanced,
        countdownSeconds: 3,
        excludeBelloBoxWindows: true,
        excludesCurrentProcessAudio: true
    )
}

struct RecordingOutputSettings: Equatable, Codable {
    var width: Int
    var height: Int
    var framesPerSecond: Int
    var videoBitrate: Int
    var audioSampleRate: Double
    var audioChannelCount: Int

    static func make(for targetPixelSize: CGSize, quality: RecordingQualityPreset) -> RecordingOutputSettings {
        let sourceWidth = max(2, Int(targetPixelSize.width.rounded()))
        let sourceHeight = max(2, Int(targetPixelSize.height.rounded()))
        let sourceLongEdge = max(sourceWidth, sourceHeight)
        let maxLongEdge: Int
        let framesPerSecond: Int
        let bitsPerPixelPerFrame: Double

        switch quality {
        case .compact:
            maxLongEdge = 1600
            framesPerSecond = 24
            bitsPerPixelPerFrame = 0.10
        case .balanced:
            maxLongEdge = 2560
            framesPerSecond = 30
            bitsPerPixelPerFrame = 0.12
        case .high:
            maxLongEdge = 3840
            framesPerSecond = 30
            bitsPerPixelPerFrame = 0.16
        }

        let scale = sourceLongEdge > maxLongEdge ? Double(maxLongEdge) / Double(sourceLongEdge) : 1
        let width = Self.even(max(2, Int((Double(sourceWidth) * scale).rounded())))
        let height = Self.even(max(2, Int((Double(sourceHeight) * scale).rounded())))
        let bitrate = max(1_000_000, Int(Double(width * height * framesPerSecond) * bitsPerPixelPerFrame))

        return RecordingOutputSettings(
            width: width,
            height: height,
            framesPerSecond: framesPerSecond,
            videoBitrate: bitrate,
            audioSampleRate: 48_000,
            audioChannelCount: 2
        )
    }

    private static func even(_ value: Int) -> Int {
        value.isMultiple(of: 2) ? value : value + 1
    }
}

enum RecordingState: Equatable {
    case idle
    case requestingPermissions
    case choosingTarget
    case countingDown(Int)
    case recording(RecordingRuntimeState)
    case paused(RecordingRuntimeState)
    case finishing
    case reviewing(URL)
    case failed(String)
}

struct RecordingRuntimeState: Equatable {
    let sessionID: RecordingSessionID
    var startedAt: Date
    let targetDescription: String
    var elapsed: TimeInterval
    var isMicEnabled: Bool
    var isSystemAudioEnabled: Bool
    var isInputOverlayEnabled: Bool
    var isSecureFieldHidden: Bool
}
