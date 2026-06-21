import AppKit
import XCTest
@testable import BelloBox

final class GlobalHotkeyTests: XCTestCase {
    func testDefaultDisplayString() {
        XCTAssertEqual(GlobalHotkey.default.displayString, "⌃⌥⌘B")
    }

    func testDefaultScreenshotDisplayString() {
        XCTAssertEqual(GlobalHotkey.defaultScreenshot.displayString, "⌃⌥⌘S")
    }

    func testDefaultRecordingDisplayString() {
        XCTAssertEqual(GlobalHotkey.defaultRecording.displayString, "⌃⌥⌘R")
    }

    func testMatchesConfiguredShortcut() {
        let event = keyEvent(keyCode: 11, modifiers: [.control, .option, .command])
        XCTAssertTrue(GlobalHotkey.default.matches(event))
    }

    func testMatchesConfiguredScreenshotShortcut() {
        let event = keyEvent(keyCode: 1, modifiers: [.control, .option, .command])
        XCTAssertTrue(GlobalHotkey.defaultScreenshot.matches(event))
    }

    func testMatchesConfiguredRecordingShortcut() {
        let event = keyEvent(keyCode: 15, modifiers: [.control, .option, .command])
        XCTAssertTrue(GlobalHotkey.defaultRecording.matches(event))
    }

    func testRejectsDifferentModifiers() {
        let event = keyEvent(keyCode: 11, modifiers: [.control, .option])
        XCTAssertFalse(GlobalHotkey.default.matches(event))
    }

    func testRejectsUnmodifiedShortcut() {
        let event = keyEvent(keyCode: 11, modifiers: [])
        XCTAssertNil(GlobalHotkey.from(event: event))
    }

    func testRejectsEscapeShortcut() {
        let event = keyEvent(keyCode: 53, modifiers: [.control, .option, .command])
        XCTAssertNil(GlobalHotkey.from(event: event))
        XCTAssertFalse(GlobalHotkey(keyCode: 53, modifiers: [.control, .option, .command]).isValid)
    }

    func testAllowsCustomShortcut() {
        let event = keyEvent(keyCode: 40, modifiers: [.command, .shift])
        let hotkey = GlobalHotkey.from(event: event)

        XCTAssertEqual(hotkey?.displayString, "⇧⌘K")
        XCTAssertTrue(hotkey?.matches(event) == true)
    }

    func testIgnoresIrrelevantModifierFlagsWhenComparingAndMatching() {
        let stored = GlobalHotkey(keyCode: 11, modifiers: [.control, .option, .command, .capsLock])
        let normalized = GlobalHotkey.default
        let event = keyEvent(keyCode: 11, modifiers: [.control, .option, .command])

        XCTAssertEqual(stored, normalized)
        XCTAssertEqual(stored.displayString, "⌃⌥⌘B")
        XCTAssertTrue(stored.matches(event))
    }

    func testHotkeyConflictMessagesOnlyConsiderEnabledShortcuts() {
        let messages = AppSettings.hotkeyConflictMessages(
            boardEnabled: true,
            board: .default,
            screenshotEnabled: false,
            screenshot: .default,
            recordingEnabled: true,
            recording: .defaultRecording
        )

        XCTAssertTrue(messages.isEmpty)
    }

    func testHotkeyConflictMessagesCatchDuplicateEnabledShortcuts() {
        let messages = AppSettings.hotkeyConflictMessages(
            boardEnabled: true,
            board: .default,
            screenshotEnabled: true,
            screenshot: .default,
            recordingEnabled: true,
            recording: .default
        )

        XCTAssertEqual(messages.count, 2)
        XCTAssertTrue(messages[0].contains("Screenshot"))
        XCTAssertTrue(messages[1].contains("Recording"))
    }

    func testActiveShortcutRecorderIDIsNotPersisted() {
        let defaults = temporaryDefaults("active-recorder")
        let settings = AppSettings(defaults: defaults)
        settings.activeShortcutRecorderID = UUID()

        let reloaded = AppSettings(defaults: defaults)

        XCTAssertNil(reloaded.activeShortcutRecorderID)
    }

    private func keyEvent(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> NSEvent {
        NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: modifiers,
            timestamp: 0,
            windowNumber: 0,
            context: nil,
            characters: "",
            charactersIgnoringModifiers: "",
            isARepeat: false,
            keyCode: keyCode
        )!
    }

    private func temporaryDefaults(_ name: String) -> UserDefaults {
        let suiteName = "BelloBoxTests.GlobalHotkey.\(name).\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
