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
