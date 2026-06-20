import XCTest
@testable import BelloBox

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
