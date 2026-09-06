import AppKit
import XCTest
@testable import BelloBox

@MainActor
final class LauncherInteractionTests: XCTestCase {
    private func withModel(_ text: String, _ test: (LauncherModel) throws -> Void) rethrows {
        let suite = "LauncherInteraction.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let selection = TextSelection(text: text, anchorRect: nil, appName: "Editor", bundleID: "example.editor", pid: 123)
        let model = LauncherModel(selection: selection, snippets: SnippetStore(), defaults: defaults)
        defer { model.cancelAll() }
        try test(model)
    }

    func testLargeSelectionKeepsTheEntireDocumentButBoundsItsPreview() {
        let text = "{\"body\":\"" + String(repeating: "long text ", count: 30_000) + "\",\"tail\":true}"
        withModel(text) { model in
            XCTAssertFalse(model.context.exceedsLimit)
            XCTAssertLessThanOrEqual(model.context.preview.unicodeScalars.count, 160)
            XCTAssertEqual(model.context.characterCount, text.count)
            XCTAssertEqual(model.suggestions.first, .json)
            for query in ["regex", "clock", "json", "url"] { model.query = query }
            model.open(.json)
            XCTAssertEqual(model.workbench?.input, text)
            XCTAssertEqual(model.workbench?.selection.pid, 123)
        }
    }
    func testOversizedSelectionNeverBecomesAPartialDocumentOrReplacement() {
        withModel(String(repeating: "x", count: UtilityLimits.inputBytes + 1)) { model in
            XCTAssertTrue(model.context.exceedsLimit)
            XCTAssertTrue(model.context.hasText)
            XCTAssertTrue(model.selection.text.isEmpty)
            XCTAssertNil(model.selection.pid)
            model.open(.json)
            XCTAssertTrue(model.workbench!.input.isEmpty)
            XCTAssertEqual(model.workbench?.inputNotice, LauncherSelectionContext.limitNotice)
            XCTAssertFalse(model.workbench!.canReplace)
            model.back()
            model.open(.generate)
            XCTAssertEqual(model.workbench?.command, .generate)
        }
    }
    func testLimitUsesUTF8BytesAndPreviewBoundsCombiningCharacters() {
        let text = "e" + String(repeating: "\u{301}", count: UtilityLimits.inputBytes / 2)
        let context = LauncherSelectionContext(text: text)
        XCTAssertTrue(context.exceedsLimit)
        XCTAssertLessThanOrEqual(context.preview.unicodeScalars.count, 160)
        let exact = LauncherSelectionContext(text: String(repeating: "a", count: UtilityLimits.inputBytes))
        XCTAssertFalse(exact.exceedsLimit)
    }
    func testClipboardReplacesOversizedContextAndClearingRemovesItsTarget() {
        withModel(String(repeating: "x", count: UtilityLimits.inputBytes + 1)) { model in
            model.useClipboard("{\"clipboard\":true}")
            XCTAssertFalse(model.context.exceedsLimit)
            XCTAssertEqual(model.suggestions.first, .json)
            XCTAssertNil(model.selection.pid)
            model.clearSelection()
            XCTAssertFalse(model.context.hasText)
            XCTAssertTrue(model.selection.text.isEmpty)
            XCTAssertNil(model.selection.pid)
        }
    }
    func testPaletteAdaptsToResultsAndArrowNavigationWraps() {
        withModel("{}") { model in
            let fullHeight = model.paletteSize.height
            model.query = "  regex  "
            XCTAssertEqual(model.commands, [.regex])
            XCTAssertLessThan(model.paletteSize.height, fullHeight)
            model.move(1)
            XCTAssertEqual(model.selectedID, "regex")
            model.query = "no-such-tool"
            XCTAssertTrue(model.commands.isEmpty)
            XCTAssertGreaterThan(model.paletteSize.height, 200)
            model.move(1)
            model.openSelected()
            XCTAssertNil(model.workbench)
        }
    }
    func testNativeSearchDelegateNavigatesAndOpensWithoutMouseFocus() {
        withModel("{}") { model in
            var query = ""
            let search = LauncherSearchField(text: .init(get: { query }, set: { query = $0 }),
                onMove: model.move, onSubmit: model.openSelected, onEscape: model.onClose, onReady: { _ in })
            let coordinator = search.makeCoordinator()
            let field = LauncherSearchTextField()
            let editor = NSTextView()
            XCTAssertTrue(field.needsPanelToBecomeKey)
            XCTAssertTrue(coordinator.control(field, textView: editor, doCommandBy: #selector(NSResponder.moveDown(_:))))
            XCTAssertEqual(model.selectedID, "compare")
            XCTAssertTrue(coordinator.control(field, textView: editor, doCommandBy: #selector(NSResponder.insertNewline(_:))))
            XCTAssertEqual(model.workbench?.command, .compare)
            XCTAssertFalse(coordinator.control(field, textView: editor, doCommandBy: #selector(NSResponder.moveLeft(_:))))
        }
    }
    func testPanelAcceptsKeyboardFocusAndRecognizesItsChildDialogs() {
        let panel = LauncherPanel()
        defer { panel.close() }
        XCTAssertTrue(panel.canBecomeKey)
        XCTAssertFalse(panel.canBecomeMain)
        XCTAssertTrue(panel.styleMask.contains(.nonactivatingPanel))
        XCTAssertFalse(panel.becomesKeyOnlyIfNeeded)
        let child = NSWindow()
        child.isReleasedWhenClosed = false
        panel.addChildWindow(child, ordered: .above)
        XCTAssertTrue(LauncherWindowController.belongsToPanel(child, panel: panel))
        XCTAssertTrue(LauncherWindowController.belongsToPanel(panel, panel: panel))
        XCTAssertFalse(LauncherWindowController.belongsToPanel(nil, panel: panel))
        panel.removeChildWindow(child)
        child.close()
    }
    /// The test host is the real app, which opens Home or onboarding about
    /// 0.3 s after launch and takes key focus. Palettes opened before that
    /// would be dismissed as "focus lost" mid-test, and a palette that never
    /// became key cannot exercise first-responder handling at all.
    private func settleHostStartup() async throws {
        let deadline = Date().addingTimeInterval(2)
        while Date() < deadline, !NSApp.windows.contains(where: { $0.isVisible && !($0 is LauncherPanel) }) {
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        // Activation lands asynchronously and hands key focus to the Home
        // window; a palette shown before that settles would be dismissed.
        NSApp.activate(ignoringOtherApps: true)
        let activation = Date().addingTimeInterval(2)
        while Date() < activation, !(NSApp.isActive && NSApp.keyWindow != nil) {
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        try await Task.sleep(nanoseconds: 100_000_000)
    }

    private func showKeyPalette(_ controller: LauncherWindowController, text: String) async throws -> LauncherPanel {
        controller.show(selection: TextSelection(text: text, anchorRect: nil, appName: text.isEmpty ? nil : "Editor", bundleID: nil, pid: nil))
        let panel = try XCTUnwrap(controller.panel)
        for _ in 0..<100 where !panel.isKeyWindow { try await Task.sleep(nanoseconds: 20_000_000) }
        if !panel.isKeyWindow { throw XCTSkip("The test host could not take key focus; first-responder behavior cannot be verified here") }
        return panel
    }

    private func keyEvent(_ keyCode: UInt16, _ characters: String, modifiers: NSEvent.ModifierFlags = [], in window: NSWindow) throws -> NSEvent {
        try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers, timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: characters, charactersIgnoringModifiers: characters,
            isARepeat: false, keyCode: keyCode))
    }

    func testCopilotFieldKeepsEnterAndArrowsWhileEscapeReturnsToSearch() async throws {
        try await settleHostStartup()
        let controller = LauncherWindowController()
        defer { controller.close() }
        let panel = try await showKeyPalette(controller, text: "")
        for _ in 0..<100 where !controller.isSearchFieldFocused { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertTrue(controller.isSearchFieldFocused, "The palette focuses search when it opens")
        XCTAssertFalse(controller.isSecondaryTextInputFocused(in: panel))
        XCTAssertTrue(controller.handleKeyEvent(try keyEvent(125, "", in: panel)), "Arrows navigate while search is focused")

        let question = NSTextField(frame: NSRect(x: 10, y: 10, width: 200, height: 22))
        panel.contentView?.addSubview(question)
        XCTAssertTrue(panel.makeFirstResponder(question))
        try await Task.sleep(nanoseconds: 50_000_000)
        XCTAssertTrue(controller.isSecondaryTextInputFocused(in: panel), "Nothing steals focus from a field the user is typing in")
        controller.model?.onPreviewResize()
        controller.model?.onPresentationChange()
        try await Task.sleep(nanoseconds: 60_000_000)
        XCTAssertTrue(controller.isSecondaryTextInputFocused(in: panel), "Resizing for the transcript must not refocus search")
        XCTAssertTrue(controller.isVisible)
        XCTAssertFalse(controller.handleKeyEvent(try keyEvent(36, "\r", in: panel)), "Enter sends the question instead of opening a tool")
        XCTAssertFalse(controller.handleKeyEvent(try keyEvent(125, "", in: panel)))
        XCTAssertFalse(controller.handleKeyEvent(try keyEvent(124, "", in: panel)))
        XCTAssertTrue(controller.isVisible)
        XCTAssertTrue(controller.handleKeyEvent(try keyEvent(53, "\u{1B}", in: panel)), "Escape leaves the copilot field")
        for _ in 0..<100 where !controller.isSearchFieldFocused { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertTrue(controller.isVisible, "The first Escape only returns to search")
        XCTAssertTrue(controller.isSearchFieldFocused)
        XCTAssertTrue(controller.handleKeyEvent(try keyEvent(53, "\u{1B}", in: panel)))
        XCTAssertFalse(controller.isVisible, "The second Escape closes the palette")
    }

    func testArrowKeysNudgeTheClockPreviewOnlyWhileSearchIsEmpty() async throws {
        try await settleHostStartup()
        let controller = LauncherWindowController()
        defer { controller.close() }
        let panel = try await showKeyPalette(controller, text: "2026-09-07T12:00:00Z")
        let model = try XCTUnwrap(controller.model)
        for _ in 0..<200 where model.clockPreview == nil { try await Task.sleep(nanoseconds: 10_000_000) }
        let clock = try XCTUnwrap(model.clockPreview)
        let start = clock.selectedInstant
        let height = panel.frame.height
        XCTAssertTrue(controller.handleKeyEvent(try keyEvent(124, "", in: panel)))
        XCTAssertEqual(clock.selectedInstant, start.addingTimeInterval(15 * 60))
        XCTAssertTrue(controller.handleKeyEvent(try keyEvent(123, "", modifiers: .option, in: panel)))
        XCTAssertEqual(clock.selectedInstant, start.addingTimeInterval(-45 * 60))
        XCTAssertTrue(controller.handleKeyEvent(try keyEvent(124, "", modifiers: .shift, in: panel)))
        XCTAssertEqual(clock.selectedInstant, start.addingTimeInterval(24 * 3_600 - 45 * 60), "Shift moves a whole day")
        XCTAssertEqual(model.selectedCommand, .worldClock, "Time keys never move the command selection")
        XCTAssertEqual(panel.frame.height, height, "Scrubbing never resizes the palette")
        model.query = "j"
        XCTAssertFalse(controller.handleKeyEvent(try keyEvent(124, "", in: panel)), "With a query the arrows edit the search text")
    }

    func testPaletteHeightIsClampedToTheVisibleScreen() {
        let tall = NSRect(x: 0, y: 0, width: 1_920, height: 1_055)
        XCTAssertEqual(LauncherWindowController.fittedSize(NSSize(width: 680, height: 754), visibleFrame: tall),
                       NSSize(width: 680, height: 754))
        let small = NSRect(x: 0, y: 0, width: 1_280, height: 700)
        let fitted = LauncherWindowController.fittedSize(NSSize(width: 680, height: 754), visibleFrame: small)
        XCTAssertEqual(fitted.width, 680)
        XCTAssertEqual(fitted.height, 700 - LauncherWindowController.screenMargin * 2, "A 720/768-row display keeps the footer on screen")
        XCTAssertEqual(LauncherWindowController.fittedSize(NSSize(width: 680, height: 754), visibleFrame: NSRect(x: 0, y: 0, width: 800, height: 200)).height, 320,
                       "A tiny frame still leaves a usable palette")
    }

    func testDismissalTearsDownPanelAndCanReopen() async throws {
        let controller = LauncherWindowController()
        let selection = TextSelection(text: "", anchorRect: nil, appName: nil, bundleID: nil, pid: nil)
        controller.show(selection: selection)
        let original = try XCTUnwrap(controller.panel)
        controller.close(reason: "outsideClick")
        XCTAssertFalse(controller.isVisible)
        XCTAssertNil(controller.panel)
        controller.show(selection: selection)
        XCTAssertFalse(controller.panel === original)
        controller.close()
    }
    func testLosingKeyFocusDismissesButOwnedChildWindowsDoNot() async throws {
        let controller = LauncherWindowController()
        defer { controller.close() }
        controller.show(selection: TextSelection(text: "", anchorRect: nil, appName: nil, bundleID: nil, pid: nil))
        let panel = try XCTUnwrap(controller.panel)
        let child = PopupPanel(contentRect: NSRect(x: 100, y: 100, width: 150, height: 100))
        panel.addChildWindow(child, ordered: .above)
        child.makeKeyAndOrderFront(nil)
        try await Task.sleep(nanoseconds: 30_000_000)
        XCTAssertTrue(controller.isVisible, "A tool's own dialog must not dismiss the palette")
        panel.removeChildWindow(child)
        child.close()
        panel.makeKeyAndOrderFront(nil)
        let other = NSWindow(contentRect: NSRect(x: 50, y: 50, width: 200, height: 100),
            styleMask: [.titled, .closable], backing: .buffered, defer: false)
        other.isReleasedWhenClosed = false
        defer { other.close() }
        other.makeKeyAndOrderFront(nil)
        try await Task.sleep(nanoseconds: 30_000_000)
        XCTAssertFalse(controller.isVisible, "Moving keyboard focus outside must dismiss the palette")
    }
}
