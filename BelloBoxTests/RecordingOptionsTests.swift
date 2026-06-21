import AppKit
@testable import BelloBox
import XCTest

final class RecordingOptionsTests: XCTestCase {
    func testDefaultsArePrivacySafe() {
        let options = RecordingOptions.default

        XCTAssertEqual(options.audioSource, .none)
        XCTAssertTrue(options.includeCursor)
        XCTAssertEqual(options.clickOverlayMode, .ringsAndLabels)
        XCTAssertEqual(options.keystrokeMode, .shortcutsOnly)
        XCTAssertEqual(options.secureFieldRedactionMode, .strict)
        XCTAssertEqual(options.quality, .balanced)
        XCTAssertEqual(options.countdownSeconds, 3)
        XCTAssertTrue(options.excludeBelloBoxWindows)
        XCTAssertTrue(options.excludesCurrentProcessAudio)
    }

    func testSecureFieldWarningRequiresAccessibility() {
        XCTAssertNil(RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: true))
        XCTAssertEqual(
            RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: false),
            RecordingPrivacyNotice.secureFieldRedactionUnavailableMessage
        )
    }

    func testAudioSourceHelpers() {
        XCTAssertFalse(RecordingAudioSource.none.includesMicrophone)
        XCTAssertFalse(RecordingAudioSource.none.includesSystemAudio)
        XCTAssertTrue(RecordingAudioSource.microphone.includesMicrophone)
        XCTAssertFalse(RecordingAudioSource.microphone.includesSystemAudio)
        XCTAssertFalse(RecordingAudioSource.systemAudio.includesMicrophone)
        XCTAssertTrue(RecordingAudioSource.systemAudio.includesSystemAudio)
        XCTAssertTrue(RecordingAudioSource.microphoneAndSystemAudio.includesMicrophone)
        XCTAssertTrue(RecordingAudioSource.microphoneAndSystemAudio.includesSystemAudio)
    }

    func testSettingsMapInvalidRecordingRawValuesToDefaults() {
        let defaults = temporaryDefaults()
        defaults.set("bad-audio", forKey: "recordingAudioSource")
        defaults.set("bad-clicks", forKey: "recordingClickOverlayMode")
        defaults.set("bad-keys", forKey: "recordingKeystrokeMode")
        defaults.set("bad-redaction", forKey: "recordingSecureFieldRedactionMode")
        defaults.set("bad-quality", forKey: "recordingQualityPreset")
        defaults.set(99, forKey: "recordingCountdownSeconds")

        let settings = AppSettings(defaults: defaults)

        XCTAssertEqual(settings.recordingAudioSource, RecordingOptions.default.audioSource)
        XCTAssertEqual(settings.recordingClickOverlayMode, RecordingOptions.default.clickOverlayMode)
        XCTAssertEqual(settings.recordingKeystrokeMode, RecordingOptions.default.keystrokeMode)
        XCTAssertEqual(settings.recordingSecureFieldRedactionMode, RecordingOptions.default.secureFieldRedactionMode)
        XCTAssertEqual(settings.recordingQualityPreset, RecordingOptions.default.quality)
        XCTAssertEqual(settings.recordingCountdownSeconds, 10)
    }

    func testSettingsClampInvalidScrollingFrameCount() {
        let lowDefaults = temporaryDefaults("low-scroll")
        lowDefaults.set(0, forKey: "scrollingScreenshotMaxFrames")
        XCTAssertEqual(AppSettings(defaults: lowDefaults).scrollingScreenshotMaxFrames, 2)

        let highDefaults = temporaryDefaults("high-scroll")
        highDefaults.set(999, forKey: "scrollingScreenshotMaxFrames")
        XCTAssertEqual(AppSettings(defaults: highDefaults).scrollingScreenshotMaxFrames, 60)
    }

    func testSettingsClampInvalidLLMOCRUploadLongEdge() {
        let lowDefaults = temporaryDefaults("low-llm-ocr")
        lowDefaults.set(0, forKey: "llmOCRMaxUploadLongEdge")
        XCTAssertEqual(AppSettings(defaults: lowDefaults).llmOCRMaxUploadLongEdge, 800)

        let highDefaults = temporaryDefaults("high-llm-ocr")
        highDefaults.set(99999, forKey: "llmOCRMaxUploadLongEdge")
        XCTAssertEqual(AppSettings(defaults: highDefaults).llmOCRMaxUploadLongEdge, 5000)
    }

    func testSettingsPersistNormalizedValuesLoadedFromDefaults() {
        let defaults = temporaryDefaults("persisted-normalization")
        defaults.set("bad-provider", forKey: "provider")
        defaults.set("bad-api-kind", forKey: "openAIAPIKind")
        defaults.set("bad-temperature-mode", forKey: "temperatureMode")
        defaults.set(9.997, forKey: "temperature")
        defaults.set(999_999, forKey: "globalHotkeyKeyCode")
        defaults.set(0, forKey: "globalHotkeyModifiers")
        defaults.set(60, forKey: "screenshotHotkeyKeyCode")
        defaults.set(Int(NSEvent.ModifierFlags.command.rawValue), forKey: "screenshotHotkeyModifiers")
        defaults.set(15, forKey: "recordingHotkeyKeyCode")
        defaults.set(0, forKey: "recordingHotkeyModifiers")
        defaults.set("", forKey: "codexModel")
        defaults.set("maximum", forKey: "codexReasoningEffort")
        defaults.set("ask-every-time", forKey: "codexApprovalPolicy")
        defaults.set("write-everywhere", forKey: "codexSandboxMode")
        defaults.set("bad-screenshot-mode", forKey: "screenshotDefaultMode")
        defaults.set(0, forKey: "scrollingScreenshotMaxFrames")
        defaults.set(99999, forKey: "llmOCRMaxUploadLongEdge")
        defaults.set("bad-quality", forKey: "recordingQualityPreset")
        defaults.set(99, forKey: "recordingCountdownSeconds")

        let settings = AppSettings(defaults: defaults)

        XCTAssertEqual(settings.providerKind, .openAI)
        XCTAssertEqual(defaults.string(forKey: "provider"), ProviderKind.openAI.rawValue)
        XCTAssertEqual(defaults.string(forKey: "openAIAPIKind"), OpenAIAPIKind.chatCompletions.rawValue)
        XCTAssertEqual(defaults.string(forKey: "temperatureMode"), TemperatureMode.providerDefault.rawValue)
        XCTAssertEqual(defaults.double(forKey: "temperature"), 2.0)
        XCTAssertEqual(defaults.integer(forKey: "globalHotkeyKeyCode"), Int(GlobalHotkey.default.keyCode))
        XCTAssertEqual(defaults.integer(forKey: "globalHotkeyModifiers"), Int(GlobalHotkey.default.modifiers.rawValue))
        XCTAssertEqual(defaults.integer(forKey: "screenshotHotkeyKeyCode"), Int(GlobalHotkey.defaultScreenshot.keyCode))
        XCTAssertEqual(defaults.integer(forKey: "screenshotHotkeyModifiers"), Int(GlobalHotkey.defaultScreenshot.modifiers.rawValue))
        XCTAssertEqual(defaults.integer(forKey: "recordingHotkeyKeyCode"), Int(GlobalHotkey.defaultRecording.keyCode))
        XCTAssertEqual(defaults.integer(forKey: "recordingHotkeyModifiers"), Int(GlobalHotkey.defaultRecording.modifiers.rawValue))
        XCTAssertEqual(defaults.string(forKey: "codexModel"), CodexCLI.defaultModel)
        XCTAssertEqual(defaults.string(forKey: "codexReasoningEffort"), CodexCLI.defaultReasoningEffort)
        XCTAssertEqual(defaults.string(forKey: "codexApprovalPolicy"), CodexCLI.defaultApprovalPolicy.rawValue)
        XCTAssertEqual(defaults.string(forKey: "codexSandboxMode"), CodexCLI.defaultSandboxMode.rawValue)
        XCTAssertEqual(defaults.string(forKey: "screenshotDefaultMode"), ScreenshotDefaultMode.area.rawValue)
        XCTAssertEqual(defaults.integer(forKey: "scrollingScreenshotMaxFrames"), 2)
        XCTAssertEqual(defaults.integer(forKey: "llmOCRMaxUploadLongEdge"), 5000)
        XCTAssertEqual(defaults.string(forKey: "recordingQualityPreset"), RecordingOptions.default.quality.rawValue)
        XCTAssertEqual(defaults.integer(forKey: "recordingCountdownSeconds"), 10)
    }

    func testSettingsDoesNotMaterializeAbsentDefaultsDuringInit() {
        let defaults = temporaryDefaults("absent-defaults")

        _ = AppSettings(defaults: defaults)

        XCTAssertNil(defaults.object(forKey: "temperature"))
        XCTAssertNil(defaults.object(forKey: "codexModel"))
        XCTAssertNil(defaults.object(forKey: "codexApprovalPolicy"))
        XCTAssertNil(defaults.object(forKey: "codexSandboxMode"))
        XCTAssertNil(defaults.object(forKey: "scrollingScreenshotMaxFrames"))
        XCTAssertNil(defaults.object(forKey: "llmOCRMaxUploadLongEdge"))
        XCTAssertNil(defaults.object(forKey: "recordingCountdownSeconds"))
    }

    func testProgrammaticNumericSettingsStayClampedInMemory() {
        let defaults = temporaryDefaults("programmatic-clamps")
        let settings = AppSettings(defaults: defaults)

        settings.temperature = .infinity
        settings.recordingCountdownSeconds = 99
        settings.scrollingScreenshotMaxFrames = 0
        settings.llmOCRMaxUploadLongEdge = 99999

        XCTAssertEqual(settings.temperature, 1.0)
        XCTAssertEqual(settings.recordingCountdownSeconds, 10)
        XCTAssertEqual(settings.scrollingScreenshotMaxFrames, 2)
        XCTAssertEqual(settings.llmOCRMaxUploadLongEdge, 5000)
        XCTAssertEqual(defaults.double(forKey: "temperature"), 1.0)
        XCTAssertEqual(defaults.integer(forKey: "recordingCountdownSeconds"), 10)
        XCTAssertEqual(defaults.integer(forKey: "scrollingScreenshotMaxFrames"), 2)
        XCTAssertEqual(defaults.integer(forKey: "llmOCRMaxUploadLongEdge"), 5000)
    }

    func testOutputSettingsPreserveEvenDimensionsAndBoundLongEdge() {
        let compact = RecordingOutputSettings.make(for: CGSize(width: 3841, height: 2161), quality: .compact)
        let balanced = RecordingOutputSettings.make(for: CGSize(width: 3841, height: 2161), quality: .balanced)
        let high = RecordingOutputSettings.make(for: CGSize(width: 3841, height: 2161), quality: .high)

        XCTAssertLessThanOrEqual(max(compact.width, compact.height), 1600)
        XCTAssertLessThanOrEqual(max(balanced.width, balanced.height), 2560)
        XCTAssertLessThanOrEqual(max(high.width, high.height), 3840)
        XCTAssertTrue(compact.width.isMultiple(of: 2))
        XCTAssertTrue(compact.height.isMultiple(of: 2))
        XCTAssertEqual(compact.framesPerSecond, 24)
        XCTAssertEqual(balanced.framesPerSecond, 30)
        XCTAssertEqual(high.framesPerSecond, 30)
    }

    private func temporaryDefaults(_ name: String = UUID().uuidString) -> UserDefaults {
        let suiteName = "BelloBoxTests.\(name)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
