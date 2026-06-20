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

    func testAllowsCustomShortcut() {
        let event = keyEvent(keyCode: 40, modifiers: [.command, .shift])
        let hotkey = GlobalHotkey.from(event: event)

        XCTAssertEqual(hotkey?.displayString, "⇧⌘K")
        XCTAssertTrue(hotkey?.matches(event) == true)
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
}
