import AppKit
import XCTest
@testable import BelloBox

@MainActor
final class UtilityWorkbenchWindowTests: XCTestCase {
    private var defaults: UserDefaults!
    private var suite: String!

    override func setUp() {
        super.setUp()
        suite = "WorkbenchWindows.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)!
    }
    override func tearDown() {
        defaults.removePersistentDomain(forName: suite)
        super.tearDown()
    }
    private func selection(_ text: String) -> TextSelection {
        TextSelection(text: text, anchorRect: nil, appName: "Test", bundleID: nil, pid: nil)
    }
    private func snippets() -> SnippetStore {
        SnippetStore(url: FileManager.default.temporaryDirectory.appendingPathComponent("workbench-tests-\(UUID()).json"))
    }
    private func waitUntil(_ condition: () -> Bool) async throws {
        for _ in 0..<200 {
            if condition() { return }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        XCTFail("Window state did not settle")
    }
    private func event(_ code: UInt16, _ text: String, window: NSWindow, modifiers: NSEvent.ModifierFlags = .command) throws -> NSEvent {
        try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers,
            timestamp: 0, windowNumber: window.windowNumber, context: nil, characters: text,
            charactersIgnoringModifiers: text, isARepeat: false, keyCode: code))
    }

    func testDiffStaysVisibleAcrossFocusChangesAndPreservesNativeEditing() async throws {
        let windows = UtilityWorkbenchWindows(onSearchTools: {})
        let model = UtilityWorkbenchModel(command: .compare, selection: selection("alpha\nbeta"), snippets: snippets())
        model.secondInput = "alpha\ngamma"
        let controller = windows.open(model, settings: AppSettings(defaults: defaults))
        defer { windows.windows.forEach { $0.close() } }
        let window = try XCTUnwrap(controller.window)
        try await waitUntil { window.isKeyWindow && window.firstResponder is LiteralTextView }
        let editor = try XCTUnwrap(window.firstResponder as? LiteralTextView)
        let end = (editor.string as NSString).length
        editor.insertText("\ndelta", replacementRange: NSRange(location: end, length: 0))
        let selection = editor.selectedRange()
        let undo = try XCTUnwrap(editor.undoManager)
        XCTAssertTrue(undo.canUndo)
        let other = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 300, height: 200), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        other.isReleasedWhenClosed = false
        defer { other.close() }
        other.makeKeyAndOrderFront(nil)
        try await waitUntil { other.isKeyWindow && !model.busy }
        XCTAssertTrue(window.isVisible, "A click in another window must not close the diff")
        XCTAssertEqual(model.input, "alpha\nbeta\ndelta")
        XCTAssertEqual(model.secondInput, "alpha\ngamma")
        XCTAssertNotNil(model.result?.comparison)
        XCTAssertEqual(windows.windows.count, 1)
        XCTAssertFalse(window.hidesOnDeactivate)
        XCTAssertEqual(window.level, .normal)
        XCTAssertNotNil(window.standardWindowButton(.closeButton))
        XCTAssertNotNil(window.standardWindowButton(.miniaturizeButton))
        window.makeKeyAndOrderFront(nil)
        try await waitUntil { window.isKeyWindow }
        XCTAssertTrue(window.firstResponder === editor, "Refocusing keeps the same native input")
        XCTAssertEqual(editor.selectedRange(), selection)
        undo.undo()
        XCTAssertEqual(editor.string, "alpha\nbeta")
    }

    func testRepeatedLauncherOpensKeepIndependentWindowsAndCloseOnlyTheirOwnJobs() async throws {
        let launcher = LauncherWindowController()
        launcher.settings = AppSettings(defaults: defaults)
        defer {
            launcher.close()
            launcher.workspaces.windows.forEach { $0.close() }
        }
        launcher.show(selection: selection("first"), initialCommand: .compare)
        let first = try XCTUnwrap(launcher.workspaces.windows.first)
        first.model.secondInput = "first revision"
        launcher.show(selection: selection("second"), initialCommand: .compare)
        let second = try XCTUnwrap(launcher.workspaces.windows.last)
        XCTAssertEqual(launcher.workspaces.windows.count, 2)
        XCTAssertFalse(first.model === second.model)
        XCTAssertNotEqual(first.window?.title, second.window?.title, "Instances are identifiable in the native title bar")
        XCTAssertNil(launcher.panel)
        XCTAssertTrue(first.window?.isVisible == true)
        XCTAssertTrue(second.window?.isVisible == true)
        first.model.ignoreWhitespace = true
        second.model.secondInput = "second revision"
        XCTAssertFalse(second.model.ignoreWhitespace)
        XCTAssertEqual(first.model.secondInput, "first revision")
        launcher.show(selection: selection("searching"))
        launcher.close(reason: "outsideClick")
        XCTAssertEqual(launcher.workspaces.windows.count, 2)
        first.model.input = "cancel this pending comparison"
        first.close()
        XCTAssertNil(first.window)
        XCTAssertFalse(first.model.busy)
        XCTAssertEqual(launcher.workspaces.windows.count, 1)
        try await waitUntil { second.model.result != nil && !second.model.busy }
        XCTAssertEqual(second.model.input, "second")
        XCTAssertTrue(second.window?.isVisible == true)
        XCTAssertNotEqual(second.model.message, "Cancelled.")
    }

    func testNewWindowAndCloseShortcutsAreScopedToTheKeyTool() async throws {
        let windows = UtilityWorkbenchWindows(onSearchTools: {})
        defer { windows.windows.forEach { $0.close() } }
        let first = windows.open(UtilityWorkbenchModel(command: .compare, selection: selection("original"), snippets: snippets()), settings: AppSettings(defaults: defaults))
        let original = try XCTUnwrap(first.window)
        try await waitUntil { original.isKeyWindow }
        XCTAssertTrue(original.performKeyEquivalent(with: try event(45, "n", window: original)))
        let second = try XCTUnwrap(windows.windows.last)
        let new = try XCTUnwrap(second.window)
        try await waitUntil { new.isKeyWindow }
        XCTAssertEqual(windows.windows.count, 2)
        XCTAssertEqual(second.model.command, .compare)
        XCTAssertTrue(second.model.input.isEmpty)
        XCTAssertTrue(second.model.secondInput.isEmpty)
        XCTAssertNil(second.model.selection.pid)
        XCTAssertEqual(first.model.input, "original")
        XCTAssertFalse(original.performKeyEquivalent(with: try event(45, "n", window: original)), "An inactive tool does not handle the active tool's shortcuts")
        XCTAssertEqual(windows.windows.count, 2)
        new.cancelOperation(nil)
        XCTAssertTrue(new.isVisible)
        XCTAssertTrue(new.performKeyEquivalent(with: try event(13, "w", window: new)))
        XCTAssertEqual(windows.windows.count, 1)
        XCTAssertTrue(original.isVisible)
        XCTAssertNil(second.window)
    }

    func testAttachedSheetKeepsWindowShortcutsUntilItCloses() async throws {
        var searches = 0
        let windows = UtilityWorkbenchWindows(onSearchTools: { searches += 1 })
        defer { windows.windows.forEach { $0.close() } }
        let controller = windows.open(UtilityWorkbenchModel(command: .json, selection: selection("{}"), snippets: snippets()), settings: AppSettings(defaults: defaults))
        let window = try XCTUnwrap(controller.window)
        try await waitUntil { window.isKeyWindow }
        let sheet = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        sheet.isReleasedWhenClosed = false
        window.beginSheet(sheet, completionHandler: { _ in })
        XCTAssertFalse(window.performKeyEquivalent(with: try event(45, "n", window: window)))
        XCTAssertFalse(window.performKeyEquivalent(with: try event(40, "k", window: window)))
        XCTAssertEqual(searches, 0)
        XCTAssertEqual(windows.windows.count, 1)
        window.endSheet(sheet)
        sheet.close()
        window.makeKeyAndOrderFront(nil)
        try await waitUntil { window.isKeyWindow && window.attachedSheet == nil }
        XCTAssertTrue(window.performKeyEquivalent(with: try event(40, "k", window: window)))
        XCTAssertEqual(searches, 1)
        XCTAssertTrue(window.isVisible)
    }

    func testTransferOutlivesPaletteCleanupWithoutLosingHTTPDraftOrPinStorage() async throws {
        var palette: LauncherModel? = LauncherModel(selection: selection("curl https://example.invalid"), snippets: snippets(), defaults: defaults)
        var transferred: UtilityWorkbenchModel?
        var pinned: String?
        palette?.pinnedText = { pinned }
        palette?.pinText = { pinned = $0 }
        palette?.onOpenWorkbench = { transferred = $0 }
        palette?.selectedID = LauncherCommand.http.id
        let session = try XCTUnwrap(palette?.session(for: .http)?.workbench)
        try await waitUntil { !session.busy && session.result != nil }
        session.request = HTTPRequestDraft(method: "PATCH", url: "https://example.invalid/draft", headers: "X-Test: value", body: "unsent")
        palette?.open(.http)
        XCTAssertTrue(transferred === session)
        palette?.cancelAll()
        weak var released = palette
        palette = nil
        XCTAssertNil(released, "An open tool does not retain the entire palette")
        XCTAssertEqual(session.request.method, "PATCH")
        XCTAssertEqual(session.request.body, "unsent")
        XCTAssertFalse(session.sending)
        session.pinText("independent pin")
        XCTAssertEqual(session.pinnedText(), "independent pin")
        session.cancel()
    }
}
