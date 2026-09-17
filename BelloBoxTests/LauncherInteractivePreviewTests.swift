import AppKit
import CoreImage
import SwiftUI
import XCTest
@testable import BelloBox

/// The interactive row previews: one session per focused command that keeps
/// its draft, never has side effects on focus, hands its draft to the full
/// tool, and fits the height the palette reserves.
@MainActor
final class LauncherInteractivePreviewTests: XCTestCase {
    private var defaults: UserDefaults!
    private var suite: String!
    override func setUp() {
        super.setUp()
        suite = "LauncherInteractive.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)!
    }
    override func tearDown() {
        defaults.removePersistentDomain(forName: suite)
        super.tearDown()
    }
    private func model(_ text: String, pid: pid_t? = 123, settings: AppSettings? = nil) -> LauncherModel {
        let selection = BelloBox.TextSelection(text: text, anchorRect: nil, appName: "Editor", bundleID: nil, pid: pid)
        return LauncherModel(selection: selection, snippets: SnippetStore(), defaults: defaults, settings: settings ?? AppSettings(defaults: defaults))
    }
    /// Settings that cannot send anything: no model for the OpenAI-compatible endpoint.
    private func unconfiguredSettings() -> AppSettings {
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = ""
        XCTAssertFalse(settings.isConfigured)
        return settings
    }
    /// "Configured" for the preview's purposes; nothing in these tests sends a request.
    private func configuredSettings() -> AppSettings {
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        return settings
    }
    private func waitUntil(_ predicate: () -> Bool, timeout: Int = 300) async throws {
        for _ in 0..<timeout {
            if predicate() { return }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        XCTFail("Condition did not settle")
    }
    private func focus(_ model: LauncherModel, _ command: LauncherCommand) { model.selectedID = command.id }

    // MARK: Sessions and limits

    func testEveryCommandGetsAnInteractiveSessionOnFocusAndKeepsItWhileNavigating() async throws {
        let model = model("{\"id\":1}")
        defer { model.cancelAll() }
        for command in LauncherCommand.allCases {
            focus(model, command)
            if command == .worldClock { try await waitUntil { model.clockPreview != nil } }
            XCTAssertNotNil(model.expandedSession, "\(command) has an interactive session")
            XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.previewHeight(for: command), "\(command)")
        }
        XCTAssertEqual(model.sessions.count, LauncherCommand.allCases.count, "Sessions survive focusing other rows")
        model.query = "regex"
        XCTAssertEqual(model.expandedCommand, .regex)
        XCTAssertEqual(model.sessions.count, LauncherCommand.allCases.count, "Searching keeps every session")
        model.useClipboard("{\"replaced\":true}")
        XCTAssertLessThanOrEqual(model.sessions.count, 1, "A new selection drops the old sessions")
    }

    func testSelectionsOverThePreviewLimitShowANoticeInsteadOfParsingOrEditing() async throws {
        let text = "{\"body\":\"" + String(repeating: "x", count: LauncherPreview.parsingByteLimit) + "\"}"
        let model = model(text)
        var opened: [UtilityWorkbenchModel] = []
        model.onOpenWorkbench = { opened.append($0) }
        defer { opened.forEach { $0.cancel() } }
        defer { model.cancelAll() }
        XCTAssertFalse(model.context.exceedsLimit)
        XCTAssertFalse(model.fitsPreviewLimit)
        for command in [LauncherCommand.json, .compare, .jwt, .regex, .url, .time, .cron, .convert, .snippets, .http, .qr, .textTools] {
            focus(model, command)
            XCTAssertNil(model.expandedSession, "\(command) never parses or edits a selection over 64 KB in the row")
            XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.previewHeight, "\(command) keeps the notice height")
        }
        for command in [LauncherCommand.generate, .ai, .screenshot, .recording, .videoToGIF, .settings, .home] {
            focus(model, command)
            XCTAssertNotNil(model.expandedSession, "\(command) does not depend on the selection")
        }
        focus(model, .json)
        try await waitUntil { model.expandedPreview != nil }
        XCTAssertEqual(model.expandedPreview?.title, "Full selection ready")
        model.openSelected()
        XCTAssertEqual(opened.last?.input, text, "Enter opens the complete selection, never a truncation")
        XCTAssertEqual(opened.last?.input.count, text.count)
    }

    func testDraftsThatGrowPastThePreviewLimitShowTheNoticeAndKeepTheCompleteDraft() async throws {
        let model = model("alpha\nbeta")
        var opened: [UtilityWorkbenchModel] = []
        model.onOpenWorkbench = { opened.append($0) }
        defer { opened.forEach { $0.cancel() } }
        defer { model.cancelAll() }
        // Pasting a large second text into Compare.
        focus(model, .compare)
        let compare = try XCTUnwrap(model.expandedSession?.workbench)
        XCTAssertFalse(compare.draftExceedsPreviewLimit)
        let large = String(repeating: "line\n", count: 20_000) // 100 KB
        compare.secondInput = large
        XCTAssertTrue(compare.draftExceedsPreviewLimit, "The row shows the notice instead of editors and a stale result")
        XCTAssertEqual(compare.secondInput.count, large.count, "The complete draft stays with the tool")
        compare.secondInput = "beta\ngamma"
        XCTAssertFalse(compare.draftExceedsPreviewLimit, "Replacing the input previews again")
        // Loading a large saved snippet, then transferring the complete draft.
        focus(model, .snippets)
        let snippets = try XCTUnwrap(model.expandedSession?.workbench)
        snippets.loadSnippet(DeveloperSnippet(name: "Big", template: String(repeating: "{{name}} ", count: 12_000)))
        XCTAssertTrue(snippets.draftExceedsPreviewLimit)
        model.open(.snippets)
        XCTAssertTrue(opened.last === snippets)
        opened.last?.input += " tail"
        XCTAssertTrue(snippets.draftExceedsPreviewLimit)
        XCTAssertTrue(snippets.input.hasSuffix(" tail"))
        XCTAssertNil(model.expandedSession, "The palette no longer owns the open window's draft")
        // "Use as Input" expanding a result past the limit.
        focus(model, .generate)
        let generate = try XCTUnwrap(model.expandedSession?.workbench)
        generate.generatorKind = .records
        generate.generatorCount = 1_000
        generate.generatorFormat = .json
        try await waitUntil { generate.result != nil && !generate.busy }
        XCTAssertGreaterThan(generate.output.utf8.count, LauncherPreview.parsingByteLimit, "A thousand records exceed the row limit")
        XCTAssertFalse(generate.draftExceedsPreviewLimit, "Output alone is shown natively; only drafts gate the row")
        focus(model, .json)
        let json = try XCTUnwrap(model.expandedSession?.workbench)
        json.input = generate.output
        XCTAssertTrue(json.draftExceedsPreviewLimit)
        // A draft over the tool limit is never parsed in the row and never partially copied.
        json.input = String(repeating: "x", count: UtilityLimits.inputBytes + 1)
        XCTAssertFalse(json.busy)
        XCTAssertTrue(json.output.isEmpty)
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        json.copyOutput()
        XCTAssertNil(pasteboard.string(forType: .string), "Nothing is copied for an oversized draft")
        model.open(.json)
        try await waitUntil { json.error != nil }
        XCTAssertTrue(json.error?.contains("500 KB") == true, "The full tool rejects it with the existing limit")
        XCTAssertTrue(json.output.isEmpty)
    }

    func testQRDraftPastThePreviewLimitShowsTheNoticeWithoutMountingEditors() {
        let model = model("https://example.com")
        defer { model.cancelAll() }
        focus(model, .qr)
        let qr = try! XCTUnwrap(model.expandedSession?.qr)
        XCTAssertTrue(qr.fitsPreviewLimit)
        qr.code.text = String(repeating: "q", count: LauncherPreview.parsingByteLimit + 1)
        XCTAssertFalse(qr.fitsPreviewLimit)
        XCTAssertNil(qr.code.image, "Far over capacity: no code")
        let view = LauncherInteractivePreviewView(command: .qr, session: model.expandedSession, preview: nil, height: nil,
            hostWindow: { nil }, onOpen: {}, onLaunch: { _ in }, onOpenSettings: {}, onFocusSearch: {}, onCopilotFieldReady: { _ in })
        let hosting = NSHostingView(rootView: view.frame(width: 644))
        hosting.layoutSubtreeIfNeeded()
        XCTAssertTrue(Self.descendants(of: hosting).compactMap { $0 as? LiteralTextView }.isEmpty, "The notice mounts no editor for the huge draft")
        var launched: BelloBox.TextSelection?
        model.onCommand = { _, selection, _ in launched = selection }
        model.openSelected()
        XCTAssertEqual(launched?.text.count, LauncherPreview.parsingByteLimit + 1, "Enter still carries the complete draft")
    }

    func testLargeSelectionTransfersCompletelyAndLaterOpensHaveIndependentDrafts() async throws {
        let large = "{\"body\":\"" + String(repeating: "x", count: LauncherPreview.parsingByteLimit) + "\"}"
        let model = model(large)
        var opened: [UtilityWorkbenchModel] = []
        model.onOpenWorkbench = { opened.append($0) }
        defer { opened.forEach { $0.cancel() } }
        defer { model.cancelAll() }
        XCTAssertNil(model.expandedSession, "The original selection is too large for the row")
        XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.previewHeight)
        model.open(.json)
        let tool = try XCTUnwrap(opened.last)
        XCTAssertEqual(tool.input, large)
        tool.input = "{\"small\":true}"
        XCTAssertNil(model.expandedSession, "The palette has transferred ownership")
        XCTAssertFalse(tool.previewsOnly)
        try await waitUntil { tool.result != nil }
        XCTAssertTrue(tool.output.contains("\"small\": true"))
        XCTAssertFalse(tool.draftExceedsPreviewLimit)
        // Full windows keep calculating even after their original palette closes.
        tool.input = large
        XCTAssertTrue(tool.draftExceedsPreviewLimit)
        model.openSelected()
        XCTAssertFalse(opened.last === tool, "Another open owns another model")
        XCTAssertEqual(opened.last?.input, large, "The complete draft, never a truncation")
        model.cancelAll()
        tool.input = "[1,2,3]"
        try await waitUntil { tool.result?.text.contains("1") == true }
        XCTAssertEqual(opened.last?.input, large, "Editing one window cannot change another")
    }

    func testURLsWithThousandsOfParametersKeepEveryParameterButMountOnlyABoundedEditor() async throws {
        let query = (0..<3_000).map { "p\($0)=v\($0)" }.joined(separator: "&")
        let model = model("https://example.com/path?\(query)&flag#frag")
        defer { model.cancelAll() }
        focus(model, .url)
        let url = try XCTUnwrap(model.expandedSession?.workbench)
        try await waitUntil { url.urlDraft != nil }
        let draft = try XCTUnwrap(url.urlDraft)
        XCTAssertEqual(draft.parameters.count, 3_001, "The model keeps every parameter")
        XCTAssertEqual(try draft.rebuilt().components(separatedBy: "&").count, 3_001, "The rebuilt URL includes all of them")
        let view = LauncherInteractivePreviewView(command: .url, session: model.expandedSession, preview: model.expandedPreview,
            height: LauncherModel.previewHeight(for: .url), hostWindow: { nil }, onOpen: {}, onLaunch: { _ in }, onOpenSettings: {},
            onFocusSearch: {}, onCopilotFieldReady: { _ in })
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 660, height: 260), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let hosting = NSHostingView(rootView: view.frame(width: 644))
        window.contentView = hosting
        hosting.layoutSubtreeIfNeeded()
        let fields = Self.descendants(of: hosting).compactMap { $0 as? LauncherSearchTextField }
        XCTAssertLessThan(fields.count, 2 * LauncherURLParameterList.compactLimit + 10, "Rows are lazy and bounded; \(fields.count) native fields were mounted")
        XCTAssertGreaterThan(fields.count, 4, "The host fields and the first visible rows are editable")
        url.buildURL()
        XCTAssertEqual(url.output.components(separatedBy: "&").count, 3_001, "Copy builds the complete URL")
        // The full workbench mounts its parameter rows lazily as well.
        let workbench = NSHostingView(rootView: UtilityWorkbenchView(model: url, onSearchTools: {}, onNewWindow: {}).frame(width: 820, height: 660))
        let toolWindow = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 820, height: 660), styleMask: [.titled], backing: .buffered, defer: false)
        toolWindow.isReleasedWhenClosed = false
        defer { toolWindow.close() }
        toolWindow.contentView = workbench
        workbench.layoutSubtreeIfNeeded()
        let toolFields = Self.descendants(of: workbench).compactMap { $0 as? NSTextField }.filter { $0.isEditable }
        XCTAssertLessThan(toolFields.count, 400, "The full tool does not mount thousands of fields at once (\(toolFields.count))")
    }

    func testPreviewGroupKeepsItsControlsOwnAccessibilityIdentifiers() async throws {
        let controller = LauncherWindowController()
        defer { controller.close() }
        controller.show(selection: BelloBox.TextSelection(text: "https://example.com", anchorRect: nil, appName: "Editor", bundleID: nil, pid: nil), focus: .qr)
        let panel = try XCTUnwrap(controller.panel)
        let root = try XCTUnwrap(panel.contentView)
        // SwiftUI builds its accessibility tree on demand for an on-screen window.
        var identifiers: [String] = []
        func collect(_ element: Any, depth: Int) {
            guard depth < 40, let node = element as? NSAccessibilityProtocol else { return }
            if let id = node.accessibilityIdentifier(), !id.isEmpty { identifiers.append(id) }
            for child in node.accessibilityChildren() ?? [] { collect(child, depth: depth + 1) }
        }
        for _ in 0..<25 {
            identifiers = []
            root.layoutSubtreeIfNeeded()
            collect(root, depth: 0)
            if identifiers.contains("launcherPreviewQRSave") { break }
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        if identifiers.isEmpty {
            // SwiftUI materializes its accessibility elements only for an
            // assistive client; without one the tree cannot be inspected here.
            throw XCTSkip("SwiftUI exposed no accessibility elements in this process; verify the grouped identifiers with a live accessibility inspector")
        }
        XCTAssertTrue(identifiers.contains("launcherPreview_qr"), "The preview is a group: \(identifiers)")
        XCTAssertTrue(identifiers.contains("launcherPreviewQRSave"), "Save keeps its own identifier: \(identifiers)")
        XCTAssertTrue(identifiers.contains("launcherPreviewQRCopy"), "Copy keeps its own identifier")
        XCTAssertTrue(identifiers.contains("launcherPreviewQREnlarge"))
        XCTAssertEqual(identifiers.filter { $0 == "launcherPreview_qr" }.count, 1, "The group identifier is not repeated on every child")
    }

    func testRejectedSelectionsHaveNoTextSessionsAndOpenEmptyTools() {
        let model = model(String(repeating: "x", count: UtilityLimits.inputBytes + 1))
        defer { model.cancelAll() }
        XCTAssertTrue(model.context.exceedsLimit)
        for command in [LauncherCommand.json, .qr, .textTools] {
            focus(model, command)
            XCTAssertNil(model.expandedSession, "\(command)")
        }
        focus(model, .qr)
        var launched: BelloBox.TextSelection?
        model.onCommand = { _, selection, _ in launched = selection }
        model.openSelected()
        XCTAssertEqual(launched?.text, "")
    }

    func testDeveloperToolPreviewIsTheWorkbenchThatEnterOpensWithEveryEditIntact() async throws {
        let model = model("{\"b\":1,\"a\":[1,2]}")
        var opened: [UtilityWorkbenchModel] = []
        model.onOpenWorkbench = { opened.append($0) }
        defer { opened.forEach { $0.cancel() } }
        defer { model.cancelAll() }
        let session = try XCTUnwrap(model.expandedSession?.workbench)
        XCTAssertEqual(session.command, .json)
        XCTAssertEqual(session.input, "{\"b\":1,\"a\":[1,2]}")
        try await waitUntil { session.result != nil }
        XCTAssertTrue(session.output.contains("\"a\": [\n"), "The full formatted output is available, not an excerpt")
        session.jsonMode = "Minify"
        try await waitUntil { session.result?.text == "{\"a\":[1,2],\"b\":1}" }
        XCTAssertNil(opened.last, "Editing the preview never opens the tool")
        XCTAssertNil(defaults.stringArray(forKey: "launcherRecents"), "Focus and edits are not uses")
        model.openSelected()
        XCTAssertTrue(opened.last === session, "Enter opens the same model, so the draft survives")
        XCTAssertEqual(opened.last?.jsonMode, "Minify")
        XCTAssertNil(model.expandedSession, "The exact session has left the palette")
        focus(model, .regex)
        let regex = try XCTUnwrap(model.expandedSession?.workbench)
        regex.regexPattern = "\\d+"
        try await waitUntil { regex.result?.regex != nil }
        XCTAssertEqual(regex.result?.regex?.ranges.count, 3)
        focus(model, .json)
        focus(model, .regex)
        XCTAssertTrue(model.expandedSession?.workbench === regex)
        XCTAssertEqual(regex.regexPattern, "\\d+", "Navigating away and back keeps the draft")
        model.open(.regex)
        XCTAssertEqual(opened.last?.regexPattern, "\\d+")
    }

    func testPinStorageInstalledAfterTheFirstSessionStillReachesAnInitiallyFocusedCompare() throws {
        // Compare is the only favorite and nothing is selected, so it is the first row.
        defaults.set(["compare"], forKey: "launcherFavorites")
        let model = model("")
        var opened: [UtilityWorkbenchModel] = []
        model.onOpenWorkbench = { opened.append($0) }
        defer { opened.forEach { $0.cancel() } }
        defer { model.cancelAll() }
        XCTAssertEqual(model.selectedCommand, .compare, "Compare is the first row before the host installs anything")
        let compare = try XCTUnwrap(model.expandedSession?.workbench)
        var pinned: String?
        // The window controller installs these after the model (and its first session) exist.
        model.pinnedText = { pinned }
        model.pinText = { pinned = $0 }
        compare.input = "first text"
        compare.pin()
        XCTAssertEqual(pinned, "first text", "Pin reaches the late-installed storage")
        compare.usePinned()
        XCTAssertEqual(compare.secondInput, "first text", "Use Pinned reads the late-installed storage")
        model.open(.compare)
        XCTAssertTrue(opened.last === compare)
        model.cancelAll()
        compare.input = "still pinned"
        compare.pin()
        XCTAssertEqual(pinned, "still pinned", "The transferred model keeps the pin store after palette cleanup")
    }

    func testFocusingRowsHasNoSideEffectsBeyondBoundedPreviewWork() async throws {
        let pasteboard = NSPasteboard.general
        let marker = "launcher-side-effect-\(UUID().uuidString)"
        pasteboard.clearContents()
        pasteboard.setString(marker, forType: .string)
        defer { pasteboard.clearContents() }
        let settings = configuredSettings()
        let model = model("curl -X POST https://example.invalid/api -H 'X-A: 1' -d 'a=1'", settings: settings)
        defer { model.cancelAll() }
        for command in LauncherCommand.allCases {
            focus(model, command)
            if command == .worldClock { try await waitUntil { model.clockPreview != nil } }
        }
        let http = try XCTUnwrap(model.session(for: .http)?.workbench)
        try await waitUntil { http.request.url == "https://example.invalid/api" }
        XCTAssertFalse(http.sending, "cURL is imported, never sent, on focus")
        XCTAssertEqual(http.request.method, "POST")
        XCTAssertTrue(http.result?.text.isEmpty ?? true, "No response exists without Send")
        XCTAssertEqual(pasteboard.string(forType: .string), marker, "Nothing is copied on focus")
        XCTAssertNil(defaults.stringArray(forKey: "launcherRecents"))
        XCTAssertNil(defaults.data(forKey: LauncherUsageStore.defaultsKey), "Focus does not teach preferences")
        let recording = try XCTUnwrap(model.session(for: .recording)?.recording)
        recording.options.outputFormat = .gif
        recording.options.countdownSeconds = 0
        XCTAssertEqual(settings.recordingOptions.outputFormat, .movie, "Recording edits stay in the session")
        XCTAssertEqual(settings.recordingOptions.countdownSeconds, 3)
        let gif = try XCTUnwrap(model.session(for: .videoToGIF)?.videoToGIF)
        gif.options.framesPerSecond = 10
        XCTAssertEqual(settings.gifExportOptions.framesPerSecond, 15, "GIF edits stay in the session")
        let ai = try XCTUnwrap(model.session(for: .ai)?.ai)
        XCTAssertTrue(ai.isConfigured)
        XCTAssertNil(ai.openHandoff, "An empty prompt carries nothing")
        XCTAssertNil(ai.sendHandoff)
        XCTAssertEqual(model.context(for: .ai).ai, nil)
    }

    // MARK: Handoffs

    func testQRPreviewShowsAScannableCodeAndHandsTheEditedTextToThePopup() throws {
        let model = model("https://example.com")
        defer { model.cancelAll() }
        focus(model, .qr)
        let qr = try XCTUnwrap(model.expandedSession?.qr)
        XCTAssertNotNil(qr.cardImage, "The row shows the real QR image")
        XCTAssertEqual(model.context(for: .qr).qrText, nil, "Unchanged text is not a separate draft")
        qr.code.text = "https://example.com/edited"
        XCTAssertNotNil(qr.cardImage)
        var launched: (BelloBox.TextSelection, LauncherCommandContext)?
        model.onCommand = { _, selection, context in launched = (selection, context) }
        model.openSelected()
        XCTAssertEqual(launched?.0.text, "https://example.com/edited", "The popup opens with the edited text")
        XCTAssertEqual(launched?.0.pid, 123, "The original app stays the replacement target")
        XCTAssertEqual(launched?.1.qrText, "https://example.com/edited")
    }

    func testTextToolsPreviewHandsCategoryAndOptionToThePopup() throws {
        let model = model("hello world")
        defer { model.cancelAll() }
        focus(model, .textTools)
        let tools = try XCTUnwrap(model.expandedSession?.textTools)
        XCTAssertEqual(tools.caseOutput, "HELLO WORLD")
        tools.category = .caseConvert
        tools.caseStyle = .snake
        XCTAssertEqual(tools.primaryOutput, "hello_world")
        var handoff: TextToolsHandoff?
        model.onCommand = { _, _, context in handoff = context.textTools }
        model.openSelected()
        XCTAssertEqual(handoff?.category, .caseConvert)
        XCTAssertEqual(handoff?.caseStyle, .snake)
        let popup = TextToolsPopupViewModel(selection: model.selection, settings: AppSettings(defaults: defaults), accessibility: AccessibilityService())
        popup.apply(try XCTUnwrap(handoff))
        XCTAssertEqual(popup.category, .caseConvert)
        XCTAssertEqual(popup.primaryOutput, "hello_world")
    }

    func testAIOpenCarriesTheDraftWithoutSendingAndOnlySendOrActionsRun() throws {
        let model = model("Some text to improve", settings: configuredSettings())
        defer { model.cancelAll() }
        focus(model, .ai)
        let ai = try XCTUnwrap(model.expandedSession?.ai)
        var handoff: AIHandoff??
        model.onCommand = { _, _, context in handoff = .some(context.ai) }
        model.openSelected()
        XCTAssertEqual(handoff, .some(nil), "Enter with an empty prompt opens Ask AI with nothing to send")
        ai.draft = "  Translate to French  "
        XCTAssertTrue(ai.canSend)
        model.openSelected()
        XCTAssertEqual(handoff??.instruction, "Translate to French")
        XCTAssertEqual(handoff??.run, false, "Enter is Open: the draft travels, nothing is sent")
        let send = try XCTUnwrap(ai.sendHandoff)
        XCTAssertEqual(send.run, true, "Only the explicit Send (button or Return in the prompt field) runs")
        XCTAssertEqual(send.instruction, "Translate to French")
        model.open(.ai) { $0.ai = ai.sendHandoff }
        XCTAssertEqual(handoff??.run, true)
        let action = try XCTUnwrap(ai.quickActions.first { $0.id == "summarize" })
        let quick = try XCTUnwrap(ai.handoff(for: action))
        XCTAssertEqual(quick.instruction, action.instruction)
        XCTAssertEqual(quick.replacesSelection, false)
        XCTAssertEqual(quick.run, true)
    }

    func testAIDraftSurvivesWithoutAProviderOrASelectionAndQuickActionsNeedText() throws {
        let unconfigured = model("Selected words", settings: unconfiguredSettings())
        defer { unconfigured.cancelAll() }
        focus(unconfigured, .ai)
        let noProvider = try XCTUnwrap(unconfigured.expandedSession?.ai)
        noProvider.draft = "Explain this"
        XCTAssertFalse(noProvider.isConfigured)
        XCTAssertFalse(noProvider.canSend, "Nothing can be sent without a provider")
        XCTAssertNil(noProvider.sendHandoff)
        XCTAssertNil(noProvider.handoff(for: noProvider.quickActions[0]))
        XCTAssertEqual(noProvider.openHandoff?.instruction, "Explain this", "The draft still opens Ask AI, unsent")
        XCTAssertEqual(noProvider.openHandoff?.run, false)
        XCTAssertEqual(unconfigured.context(for: .ai).ai?.run, false)

        let empty = model("", settings: configuredSettings())
        defer { empty.cancelAll() }
        focus(empty, .ai)
        let noSelection = try XCTUnwrap(empty.expandedSession?.ai)
        XCTAssertFalse(noSelection.hasSelection)
        noSelection.draft = "What is the capital of France?"
        XCTAssertTrue(noSelection.canSend, "A custom question needs no selection")
        XCTAssertEqual(noSelection.sendHandoff?.run, true)
        XCTAssertFalse(noSelection.canRunQuickActions, "Transformations need selected text")
        XCTAssertNil(noSelection.handoff(for: noSelection.quickActions[0]))
    }

    func testAskAIPopupAppliesOpenHandoffsWithoutRunningAndSendHandoffsRun() {
        let settings = unconfiguredSettings()
        let selection = BelloBox.TextSelection(text: "Some words", anchorRect: nil, appName: "Editor", bundleID: nil, pid: nil)
        let popup = ActionPopupViewModel(selection: selection, settings: settings, client: AIClient(), accessibility: AccessibilityService())
        popup.apply(AIHandoff(instruction: "Translate to French", replacesSelection: true, run: false))
        XCTAssertEqual(popup.instruction, "Translate to French", "Opening shows the prompt")
        XCTAssertFalse(popup.didRun, "Opening never starts a request")
        XCTAssertNil(popup.errorMessage)
        popup.apply(AIHandoff(instruction: "Translate to French", replacesSelection: true, run: true))
        XCTAssertTrue(popup.didRun, "Send starts the request")
        XCTAssertNotNil(popup.errorMessage, "Without a provider the popup explains instead of sending")
        XCTAssertFalse(popup.isStreaming)
        popup.apply(AIHandoff(instruction: QuickAction.library[0].instruction, replacesSelection: true, run: true))
        XCTAssertEqual(popup.instruction, "Translate to French", "A quick action keeps the custom prompt field as it was")
        let draftPopup = ActionPopupViewModel(selection: selection, settings: settings, client: AIClient(), accessibility: AccessibilityService())
        draftPopup.apply(AIHandoff(instruction: QuickAction.library[0].instruction, replacesSelection: true, run: false))
        XCTAssertEqual(draftPopup.instruction, QuickAction.library[0].instruction, "A typed draft matching a preset still appears when opened")
        XCTAssertFalse(draftPopup.didRun, "Opening a preset-shaped draft must not send it")
    }

    func testCaptureRecordingGIFSettingsAndHomeHandoffsCarryTheChosenOptions() throws {
        let model = model("")
        defer { model.cancelAll() }
        var launched: (LauncherCommand, LauncherCommandContext)?
        model.onCommand = { command, _, context in launched = (command, context) }
        focus(model, .screenshot)
        let capture = try XCTUnwrap(model.expandedSession?.capture)
        XCTAssertEqual(capture.modes, ScreenshotCaptureMode.allCases)
        model.openSelected()
        XCTAssertNil(launched?.1.capture, "Enter uses the default flow")
        model.open(.screenshot) { $0.capture = CaptureHandoff(mode: .window) }
        XCTAssertEqual(launched?.1.capture?.mode, .window)
        focus(model, .scrollCapture)
        XCTAssertEqual(model.expandedSession?.capture?.modes, [.scrolling])
        model.open(.scrollCapture) { $0.capture = CaptureHandoff(mode: .scrolling) }
        XCTAssertEqual(launched?.0, .scrollCapture)
        XCTAssertEqual(launched?.1.capture?.mode, .scrolling)

        focus(model, .recording)
        let recording = try XCTUnwrap(model.expandedSession?.recording)
        XCTAssertEqual(recording.deliverable, "Silent movie (.mov)", "No audio source means no audio track")
        recording.options.audioSource = .microphone
        XCTAssertTrue(recording.deliverable.contains("microphone"))
        recording.options.outputFormat = .gif
        recording.options.quality = .high
        XCTAssertTrue(recording.deliverable.hasPrefix("Silent GIF"))
        model.openSelected()
        XCTAssertEqual(launched?.1.recording?.outputFormat, .gif)
        XCTAssertEqual(launched?.1.recording?.quality, .high)

        focus(model, .videoToGIF)
        let gif = try XCTUnwrap(model.expandedSession?.videoToGIF)
        gif.options.maxWidth = 320
        gif.options.loops = false
        model.openSelected()
        XCTAssertEqual(launched?.1.videoToGIF?.options.maxWidth, 320)
        XCTAssertEqual(launched?.1.videoToGIF?.options.loops, false)
        XCTAssertEqual(launched?.1.videoToGIF?.chooseFile, false, "Enter opens the converter; the file chooser is a separate explicit action")
        model.open(.videoToGIF) { $0.videoToGIF = VideoToGIFHandoff(options: gif.options, chooseFile: true) }
        XCTAssertEqual(launched?.1.videoToGIF?.chooseFile, true)

        focus(model, .settings)
        XCTAssertNotNil(model.expandedSession?.appStatus)
        model.openSelected()
        XCTAssertNil(launched?.1.settings)
        model.open(.settings) { $0.settings = .permissions }
        XCTAssertEqual(launched?.1.settings, .permissions)
        focus(model, .home)
        model.open(.home) { $0.home = .category(.capture) }
        XCTAssertEqual(launched?.1.home, .category(.capture))
        XCTAssertEqual(defaults.stringArray(forKey: "launcherRecents")?.first, "home", "Explicit opens are recorded as uses")
    }

    func testRecordingAndCapturePreviewsReportPermissionsWithoutPrompting() {
        var asked = 0
        let denied = LauncherRecordingPreview(options: .default) { options in
            asked += 1
            return RecordingPermissionState(screenRecording: .notDetermined, microphone: .granted, inputMonitoring: .granted, accessibility: .granted, systemAudio: .granted)
        }
        XCTAssertFalse(denied.canRecordVideo)
        XCTAssertEqual(asked, 1)
        denied.refreshPermissions()
        XCTAssertEqual(asked, 2, "Refresh re-reads the status; nothing prompts")
        XCTAssertTrue(denied.summary.contains("Movie"))
        let settings = AppSettings(defaults: defaults)
        let capture = LauncherCapturePreview(command: .screenshot, settings: settings, permission: { false })
        XCTAssertFalse(capture.hasScreenRecordingPermission)
        XCTAssertEqual(capture.defaultMode, .area)
        let scrolling = LauncherCapturePreview(command: .scrollCapture, settings: settings, permission: { true })
        XCTAssertEqual(scrolling.defaultMode, .scrolling)
        let status = LauncherAppStatusPreview(settings: settings, accessibility: { false }, screenRecording: { true })
        XCTAssertFalse(status.accessibilityTrusted)
        XCTAssertTrue(status.screenRecordingTrusted)
        XCTAssertTrue(status.aiSummary.hasPrefix("AI"))
        settings.openAIModel = ""
        XCTAssertTrue(status.aiSummary.contains("optional"), "Without a usable provider the status says AI is optional")
    }

    // MARK: Accessibility

    func testAccessibilitySummariesFollowTheLiveSessionNotTheOriginalSelection() async throws {
        let model = model("{\"id\":1}")
        defer { model.cancelAll() }
        let json = try XCTUnwrap(model.expandedSession)
        try await waitUntil { json.workbench?.result != nil }
        XCTAssertTrue(json.accessibilitySummary.contains("Valid JSON"))
        XCTAssertFalse(json.accessibilitySummary.contains("excerpt"))
        json.workbench?.input = "{broken"
        try await waitUntil { json.workbench?.error != nil }
        XCTAssertTrue(json.accessibilitySummary.contains("Not recognized"), "The row announces the current error, not the old validity")
        focus(model, .qr)
        let qr = try XCTUnwrap(model.expandedSession)
        qr.qr?.code.text = "short"
        XCTAssertTrue(qr.accessibilitySummary.contains("5 bytes"), "Byte count follows the edited text")
        XCTAssertFalse(qr.accessibilitySummary.contains("{\"id\":1}"))
        focus(model, .ai)
        model.expandedSession?.ai?.draft = "Explain"
        XCTAssertTrue(model.expandedSession?.accessibilitySummary.contains("Prompt: Explain") == true)
    }

    // MARK: QR density

    private static func decode(_ image: CGImage) -> String? {
        let detector = CIDetector(ofType: CIDetectorTypeQRCode, context: nil, options: [CIDetectorAccuracy: CIDetectorAccuracyHigh])!
        return detector.features(in: CIImage(cgImage: image)).compactMap { ($0 as? CIQRCodeFeature)?.messageString }.first
    }
    /// The card as a standard-resolution display shows it: white, inset, nearest-neighbour.
    private static func card(_ image: NSImage, points: CGFloat, inset: CGFloat) -> CGImage {
        let size = Int(points)
        let context = CGContext(data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
                                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.setFillColor(CGColor(gray: 1, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: size, height: size))
        context.interpolationQuality = .none
        var rect = CGRect(x: 0, y: 0, width: image.size.width, height: image.size.height)
        let cg = image.cgImage(forProposedRect: &rect, context: nil, hints: nil)!
        context.draw(cg, in: CGRect(x: inset, y: inset, width: points - 2 * inset, height: points - 2 * inset))
        return context.makeImage()!
    }

    func testDenseCodesAreFlaggedAndTheEnlargedCardDecodesOnAStandardDisplay() throws {
        let sparse = LauncherQRPreview(text: String(repeating: "b", count: 50))
        XCTAssertFalse(sparse.isDense)
        XCTAssertEqual(Self.decode(Self.card(try XCTUnwrap(sparse.cardImage), points: sparse.cardPoints, inset: LauncherQRPreview.cardInset)),
                       sparse.text, "A sparse code scans from the compact card")
        for length in [500, 1_000, 1_900] {
            let dense = LauncherQRPreview(text: String(repeating: "b", count: length))
            XCTAssertTrue(dense.isDense, "\(length) bytes is dense for the compact card")
            XCTAssertNotNil(dense.moduleCount)
            XCTAssertEqual(dense.extraHeight, 0)
            dense.isEnlarged = true
            XCTAssertGreaterThanOrEqual(dense.largeCardPoints, CGFloat(dense.moduleCount!) * 2 + 2 * LauncherQRPreview.cardInset)
            XCTAssertLessThanOrEqual(dense.largeCardPoints, LauncherQRPreview.largeCardRange.upperBound)
            XCTAssertEqual(dense.extraHeight, dense.largeCardPoints - LauncherQRPreview.compactCardPoints)
            let shown = Self.card(try XCTUnwrap(dense.cardImage), points: dense.cardPoints, inset: LauncherQRPreview.cardInset)
            XCTAssertEqual(Self.decode(shown), dense.text, "\(length) bytes decodes from the enlarged card at 1×")
            let export = try XCTUnwrap(QRCodeGenerator.pngData(for: dense.text, pixelSize: QRCodeGenerator.exportPixelSize(for: dense.text)))
            let source = CGImageSourceCreateWithData(export as CFData, nil)!
            XCTAssertEqual(Self.decode(CGImageSourceCreateImageAtIndex(source, 0, nil)!), dense.text, "\(length) bytes decodes from the saved PNG")
            XCTAssertGreaterThanOrEqual(QRCodeGenerator.exportPixelSize(for: dense.text), CGFloat(dense.moduleCount! * 4))
        }
        XCTAssertNil(LauncherQRPreview(text: "").moduleCount)
        XCTAssertNil(LauncherQRPreview(text: String(repeating: "x", count: QRCodeGenerator.maxByteCount + 1)).moduleCount)
    }

    func testEnlargingTheQRCodeGrowsThePaletteOnceAndShrinksItBack() async throws {
        let model = model(String(repeating: "b", count: 1_000))
        defer { model.cancelAll() }
        focus(model, .qr)
        let qr = try XCTUnwrap(model.expandedSession?.qr)
        let base = model.paletteSize.height
        var resizes: [CGFloat] = []
        var presentations = 0
        model.onPreviewResize = { resizes.append(model.paletteSize.height) }
        model.onPresentationChange = { presentations += 1 }
        qr.isEnlarged = true
        XCTAssertEqual(resizes, [base + qr.extraHeight], "The palette grows once; the callback sees the grown height")
        XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.previewHeight(for: .qr) + qr.extraHeight)
        qr.isEnlarged = true
        XCTAssertEqual(resizes.count, 1, "Setting the same state again does not resize")
        qr.isEnlarged = false
        XCTAssertEqual(resizes.last, base)
        XCTAssertEqual(presentations, 0, "Enlarging uses the no-refocus resize path")
        focus(model, .json)
        qr.isEnlarged = true
        XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.previewHeight(for: .json), "Another row is unaffected")
        focus(model, .qr)
        XCTAssertTrue(qr.isEnlarged, "The enlarged state survives navigating away and back")
    }

    func testChangingTheTextOfAnEnlargedCodeResizesOnceForTheNewDensityAndDropsOldRasters() throws {
        let model = model(String(repeating: "b", count: 1_900))
        defer { model.cancelAll() }
        focus(model, .qr)
        let qr = try XCTUnwrap(model.expandedSession?.qr)
        qr.isEnlarged = true
        _ = qr.cardImage
        let large = qr.largeCardPoints
        XCTAssertGreaterThan(large, 320, "A 1,900-byte code needs a card near the top of the range")
        XCTAssertLessThanOrEqual(large, LauncherQRPreview.largeCardRange.upperBound)
        var resizes: [CGFloat] = []
        var presentations = 0
        model.onPreviewResize = { resizes.append(model.paletteSize.height) }
        model.onPresentationChange = { presentations += 1 }
        qr.code.text = String(repeating: "b", count: 300)
        XCTAssertLessThan(qr.largeCardPoints, large, "A sparser code needs a smaller card")
        XCTAssertEqual(resizes.count, 1, "The real height change is reported once")
        XCTAssertEqual(resizes.first, model.paletteSize.height)
        XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.previewHeight(for: .qr) + qr.extraHeight)
        XCTAssertEqual(presentations, 0, "…without refocusing")
        qr.code.text = String(repeating: "b", count: 301)
        XCTAssertEqual(resizes.count, 1, "The same density does not resize again")
        XCTAssertEqual(qr.cachedRenderCount, 0, "Old rasters are dropped when the text changes")
        _ = qr.cardImage
        XCTAssertEqual(qr.cachedRenderCount, 1)
        qr.isEnlarged = false
        _ = qr.cardImage
        XCTAssertEqual(qr.cachedRenderCount, 2, "At most one raster per card size for the current text")
        qr.code.text = "changed"
        XCTAssertEqual(qr.cachedRenderCount, 0)
        XCTAssertEqual(qr.moduleCount, QRCodeGenerator.moduleCount(for: "changed"), "Metrics follow the new text before it lands")
    }

    func testOversizedDraftsAreNotCalculatedInTheRowUntilTheToolOpens() async throws {
        let model = model("alpha\nbeta")
        defer { model.cancelAll() }
        focus(model, .compare)
        let compare = try XCTUnwrap(model.expandedSession?.workbench)
        XCTAssertTrue(compare.previewsOnly)
        compare.secondInput = "alpha\n" + String(repeating: "x", count: 100_000) // 100 KB on two lines
        XCTAssertTrue(compare.draftExceedsPreviewLimit)
        XCTAssertFalse(compare.busy, "The row schedules no work for an oversized draft")
        try await Task.sleep(nanoseconds: 300_000_000)
        XCTAssertNil(compare.result)
        XCTAssertNil(compare.error)
        model.open(.compare)
        XCTAssertFalse(compare.previewsOnly)
        try await waitUntil { compare.result != nil || compare.error != nil }
        XCTAssertNotNil(compare.result, "The full tool calculates the complete draft")
        model.cancelAll()
        XCTAssertFalse(compare.previewsOnly, "Closing search never returns an open window to preview limits")
        compare.cancel()
    }

    func testAIPromptSummaryIsBoundedAndRecordingPrivacyCopyFollowsAccessibility() {
        let ai = LauncherAIPreview(text: "x", settings: AppSettings(defaults: defaults))
        ai.draft = String(repeating: "word ", count: 100)
        XCTAssertLessThanOrEqual(ai.promptSummary.count, 61)
        XCTAssertTrue(ai.promptSummary.hasSuffix("…"))
        ai.draft = "short"
        XCTAssertEqual(ai.promptSummary, "short")
        let granted = RecordingPermissionState(screenRecording: .granted, microphone: .granted, inputMonitoring: .granted, accessibility: .granted, systemAudio: .granted)
        XCTAssertTrue(LauncherRecordingPreview(options: .default) { _ in granted }.privacyNote.contains("hidden while you type"))
        var denied = granted
        denied.accessibility = .notDetermined
        XCTAssertTrue(LauncherRecordingPreview(options: .default) { _ in denied }.privacyNote.contains("needs Accessibility"))
    }

    func testLiveClockPreviewHandoffTakesOverAnExistingPlannedWindowAndClearsItsConversation() async throws {
        let store = WorldClockPreferencesStore(defaults: defaults)
        store.save(zoneIDs: ["Asia/Tokyo", "Europe/Berlin"], anchorZoneID: "Europe/Berlin")
        let settings = AppSettings(defaults: defaults)
        // The dedicated window is already open, planning a historical time with a conversation.
        let window = WorldClockViewModel(settings: settings, seedDate: Date(timeIntervalSince1970: 1_700_000_000), preferences: store, mode: .planner,
                                         askCopilot: { _, _ in WorldClockCopilotReply(answer: "old", suggestion: nil, suggestionIssue: nil) })
        window.copilot.restore(WorldClockCopilotSnapshot(messages: [
            WorldClockCopilotSession.Message(id: UUID(), role: .user, text: "earlier question", suggestion: nil, issue: nil),
            WorldClockCopilotSession.Message(id: UUID(), role: .assistant, text: "old", suggestion: nil, issue: nil)
        ], draft: "", appliedParts: [:], pendingQuestion: nil, outcome: .answered))
        XCTAssertFalse(window.isFollowingNow)
        XCTAssertEqual(window.copilot.messages.count, 2)
        // A live palette preview without a timestamp, reference switched to Tokyo.
        let model = model("plain words", settings: settings)
        defer { model.cancelAll() }
        model.query = "world"
        try await waitUntil { model.clockPreview != nil }
        let preview = try XCTUnwrap(model.clockPreview)
        XCTAssertTrue(preview.isFollowingNow)
        preview.setAnchorZone("Asia/Tokyo")
        let handoff = try XCTUnwrap(model.worldClockHandoff, "A live preview always hands over explicit live intent")
        XCTAssertTrue(handoff.followsNow)
        XCTAssertNil(handoff.instant)
        XCTAssertEqual(handoff.anchorZoneID, "Asia/Tokyo")
        XCTAssertEqual(handoff.copilot?.isEmpty, true)
        window.adopt(handoff)
        XCTAssertTrue(window.isFollowingNow, "The window follows now again")
        XCTAssertLessThan(abs(window.selectedInstant.timeIntervalSinceNow), 5)
        XCTAssertNil(window.seedInstant)
        XCTAssertEqual(window.anchorZoneID, "Asia/Tokyo")
        XCTAssertTrue(window.copilot.messages.isEmpty, "The old conversation is dropped for the new context")
        XCTAssertEqual(store.loadAnchorZoneID(validZoneIDs: ["Asia/Tokyo", "Europe/Berlin"]), "Europe/Berlin", "Nothing is saved")
        // Scrubbing the preview hands over a planned instant instead.
        preview.nudge(by: 4)
        let planned = try XCTUnwrap(model.worldClockHandoff)
        XCTAssertFalse(planned.followsNow)
        XCTAssertEqual(planned.instant, preview.selectedInstant)
        window.adopt(planned)
        XCTAssertFalse(window.isFollowingNow)
        XCTAssertEqual(window.selectedInstant, preview.selectedInstant)
        // A plain reopen (no handoff) keeps whatever the window shows.
        XCTAssertEqual(WorldClockHandoff().followsNow, false)
    }

    // MARK: Stale work

    func testStaleWorkbenchResultsAreDroppedAndCancelledSessionsStayQuiet() async throws {
        let model = model("{\"first\":1}")
        defer { model.cancelAll() }
        let json = try XCTUnwrap(model.expandedSession?.workbench)
        try await waitUntil { json.result != nil }
        json.jsonMode = "Validate"
        json.jsonMode = "Minify"
        try await waitUntil { json.result?.text == "{\"first\":1}" }
        XCTAssertFalse(json.busy)
        model.useClipboard("{\"second\":2}")
        XCTAssertFalse(model.sessions[.json]?.workbench === json, "The old session is detached")
        let replacement = try XCTUnwrap(model.expandedSession?.workbench)
        XCTAssertFalse(replacement === json)
        try await waitUntil { replacement.result != nil }
        XCTAssertTrue(replacement.output.contains("\"second\": 2"))
        XCTAssertFalse(json.output.contains("second"), "The replaced session never receives the new input")
    }

    // MARK: Keyboard focus

    func testPreviewFieldsNeverClaimFocusWhenTheyAppearAndKeepArrowsNativelyWhileEscapeReturnsToSearch() {
        var text = ""
        var escapes = 0
        var submits = 0
        let field = LauncherSearchField(text: .init(get: { text }, set: { text = $0 }), onMove: { _ in XCTFail("Preview fields keep Up/Down") },
                                        onSubmit: { submits += 1 }, onEscape: { escapes += 1 }, onReady: { _ in },
                                        focusesWhenAttached: false, monospaced: true, consumesVerticalArrows: false)
        let coordinator = field.makeCoordinator()
        let native = LauncherSearchTextField()
        let editor = NSTextView()
        XCTAssertFalse(coordinator.control(native, textView: editor, doCommandBy: #selector(NSResponder.moveDown(_:))), "Up/Down stay with the field")
        XCTAssertFalse(coordinator.control(native, textView: editor, doCommandBy: #selector(NSResponder.moveUp(_:))))
        XCTAssertTrue(coordinator.control(native, textView: editor, doCommandBy: #selector(NSResponder.insertNewline(_:))))
        XCTAssertEqual(submits, 1)
        XCTAssertTrue(coordinator.control(native, textView: editor, doCommandBy: #selector(NSResponder.cancelOperation(_:))))
        XCTAssertEqual(escapes, 1)

        // Mounted for real: neither the single-line field nor the literal editor asks for focus.
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let hosting = NSHostingView(rootView: VStack {
            LauncherPreviewField(text: .init(get: { text }, set: { text = $0 }), placeholder: "Pattern", label: "Pattern", onEscape: {})
            LauncherPreviewEditor(text: .init(get: { text }, set: { text = $0 }), label: "Draft", height: 60)
        }.frame(width: 300))
        window.contentView = hosting
        hosting.layoutSubtreeIfNeeded()
        let fields = Self.descendants(of: hosting).compactMap { $0 as? LauncherSearchTextField }
        let editors = Self.descendants(of: hosting).compactMap { $0 as? LiteralTextView }
        XCTAssertEqual(fields.count, 1)
        XCTAssertEqual(editors.count, 1)
        XCTAssertFalse(fields[0].focusesWhenAttached, "Mounting a preview never steals the search field's focus")
        XCTAssertFalse(editors[0].focusesWhenAttached)
        XCTAssertTrue(fields[0].needsPanelToBecomeKey)
        XCTAssertFalse(editors[0].isAutomaticQuoteSubstitutionEnabled, "Drafts stay literal")
        XCTAssertTrue(editors[0].allowsUndo)
    }
    private static func descendants(of view: NSView) -> [NSView] {
        view.subviews.flatMap { [$0] + descendants(of: $0) }
    }

    func testReadOnlyOutputDoesNotCountAsATextInputButEditorsDo() throws {
        let controller = LauncherWindowController()
        defer { controller.close() }
        controller.show(selection: BelloBox.TextSelection(text: "", anchorRect: nil, appName: nil, bundleID: nil, pid: nil))
        let panel = try XCTUnwrap(controller.panel)
        let output = LauncherOutputTextView(frame: NSRect(x: 0, y: 0, width: 100, height: 40))
        output.isEditable = false
        panel.contentView?.addSubview(output)
        XCTAssertTrue(panel.makeFirstResponder(output))
        XCTAssertFalse(controller.isSecondaryTextInputFocused(in: panel), "A clicked result keeps the palette's navigation keys")
        XCTAssertTrue(controller.isReadOnlyOutputFocused(in: panel))
        let editor = LiteralTextView(frame: NSRect(x: 0, y: 0, width: 100, height: 40))
        editor.isEditable = true
        panel.contentView?.addSubview(editor)
        XCTAssertTrue(panel.makeFirstResponder(editor))
        XCTAssertTrue(controller.isSecondaryTextInputFocused(in: panel), "An editor owns Enter and the arrows")
        XCTAssertFalse(controller.isReadOnlyOutputFocused(in: panel))
        XCTAssertTrue(LauncherWindowController.isNavigationKey(125))
        XCTAssertTrue(LauncherWindowController.isNavigationKey(36))
        XCTAssertFalse(LauncherWindowController.isNavigationKey(0), "Typing a letter over a clicked result returns to search")
    }

    func testFocusingARowHighlightsItsPreviewWithoutOpening() {
        let controller = LauncherWindowController()
        defer { controller.close() }
        controller.show(selection: BelloBox.TextSelection(text: "hello", anchorRect: nil, appName: "Editor", bundleID: nil, pid: nil), focus: .qr)
        XCTAssertEqual(controller.model?.selectedCommand, .qr)
        XCTAssertNotNil(controller.model?.expandedSession?.qr)
        XCTAssertTrue(controller.workspaces.windows.isEmpty)
        XCTAssertNotNil(controller.model?.hostWindow(), "Sheets from the preview attach to the palette")
    }

    private func keyEvent(_ keyCode: UInt16, _ characters: String, modifiers: NSEvent.ModifierFlags = [], in window: NSWindow) throws -> NSEvent {
        try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers, timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: characters, charactersIgnoringModifiers: characters,
            isARepeat: false, keyCode: keyCode))
    }

    /// The test host is the real app; the palette must be key to exercise first responders.
    private func showKeyPalette(_ controller: LauncherWindowController, text: String) async throws -> LauncherPanel {
        let deadline = Date().addingTimeInterval(2)
        while Date() < deadline, !NSApp.windows.contains(where: { $0.isVisible && !($0 is LauncherPanel) }) {
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        NSApp.activate(ignoringOtherApps: true)
        let activation = Date().addingTimeInterval(2)
        while Date() < activation, !(NSApp.isActive && NSApp.keyWindow != nil) { try await Task.sleep(nanoseconds: 20_000_000) }
        try await Task.sleep(nanoseconds: 100_000_000)
        controller.show(selection: BelloBox.TextSelection(text: text, anchorRect: nil, appName: "Editor", bundleID: nil, pid: nil))
        let panel = try XCTUnwrap(controller.panel)
        for _ in 0..<100 where !panel.isKeyWindow { try await Task.sleep(nanoseconds: 20_000_000) }
        if !panel.isKeyWindow { throw XCTSkip("The test host could not take key focus; first-responder behavior cannot be verified here") }
        return panel
    }

    func testSearchFromAnOpenToolKeepsTheEditorAndRoutesTypingToSearch() async throws {
        let controller = LauncherWindowController()
        defer {
            controller.close()
            controller.workspaces.windows.forEach { $0.close() }
        }
        let original = "{\"id\":1,\"name\":\"Bello\"}"
        _ = try await showKeyPalette(controller, text: original)
        controller.model?.open(.json)
        let workspace = try XCTUnwrap(controller.workspaces.windows.first)
        let window = try XCTUnwrap(workspace.window)
        try await waitUntil { window.isKeyWindow && window.firstResponder is LiteralTextView }
        let editor = try XCTUnwrap(window.firstResponder as? LiteralTextView)
        window.cancelOperation(nil)
        XCTAssertTrue(window.isVisible, "Escape never closes a persistent tool")
        XCTAssertTrue(window.performKeyEquivalent(with: try keyEvent(40, "k", modifiers: .command, in: window)))
        try await waitUntil { controller.isSearchFieldFocused }
        let panel = try XCTUnwrap(controller.panel)
        let search = try XCTUnwrap(Self.descendants(of: try XCTUnwrap(panel.contentView)).compactMap { $0 as? LauncherSearchTextField }
            .first { $0.identifier?.rawValue == "launcherSearch" })
        XCTAssertFalse(panel === window)
        XCTAssertTrue(window.isVisible)
        panel.sendEvent(try keyEvent(12, "q", in: panel))
        panel.sendEvent(try keyEvent(15, "r", in: panel))
        XCTAssertEqual(search.stringValue, "qr")
        XCTAssertEqual(workspace.model.input, original)
        XCTAssertEqual(editor.string, original)
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString("-paste", forType: .string)
        defer { pasteboard.clearContents() }
        (panel.firstResponder as? NSTextView)?.paste(nil)
        XCTAssertEqual(search.stringValue, "qr-paste")
        XCTAssertEqual((panel.firstResponder as? NSTextView)?.undoManager?.canUndo, true)
        controller.close()
        window.makeKeyAndOrderFront(nil)
        XCTAssertTrue(window.isVisible)
        XCTAssertTrue(editor.window === window, "The old editor stays attached with its native editing state")
        XCTAssertEqual(workspace.model.input, original)
    }

    func testTrackedMenusAndAttachedSheetsOwnTheKeyboardAndNeverDismissOrOpen() async throws {
        let controller = LauncherWindowController()
        defer { controller.close() }
        let panel = try await showKeyPalette(controller, text: "")
        let model = try XCTUnwrap(controller.model)
        for _ in 0..<100 where !controller.isSearchFieldFocused { try await Task.sleep(nanoseconds: 10_000_000) }
        let selected = model.selectedID
        controller.setMenuTracking(true)
        XCTAssertFalse(controller.handleKeyEvent(try keyEvent(125, "", in: panel)), "A tracked menu keeps the arrows")
        XCTAssertFalse(controller.handleKeyEvent(try keyEvent(36, "\r", in: panel)), "Return selects the menu item, not a tool")
        XCTAssertFalse(controller.handleKeyEvent(try keyEvent(53, "\u{1B}", in: panel)), "Escape closes the menu, not the palette")
        XCTAssertEqual(model.selectedID, selected)
        XCTAssertTrue(controller.workspaces.windows.isEmpty)
        XCTAssertTrue(controller.isVisible)
        controller.setMenuTracking(false)
        XCTAssertTrue(controller.handleKeyEvent(try keyEvent(125, "", in: panel)))
        XCTAssertNotEqual(model.selectedID, selected)

        let sheet = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        sheet.isReleasedWhenClosed = false
        panel.beginSheet(sheet, completionHandler: { _ in })
        for _ in 0..<100 where panel.attachedSheet == nil { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertNotNil(panel.attachedSheet)
        let before = model.selectedID
        XCTAssertFalse(controller.handleKeyEvent(try keyEvent(125, "", in: panel)), "An attached sheet keeps the keyboard")
        XCTAssertFalse(controller.handleKeyEvent(try keyEvent(36, "\r", in: panel)))
        XCTAssertFalse(controller.handleKeyEvent(try keyEvent(53, "\u{1B}", in: panel)))
        XCTAssertEqual(model.selectedID, before)
        XCTAssertTrue(controller.workspaces.windows.isEmpty)
        try await Task.sleep(nanoseconds: 60_000_000)
        XCTAssertTrue(controller.isVisible, "The sheet taking key focus does not dismiss the palette")
        panel.endSheet(sheet)
        sheet.close()
        for _ in 0..<100 where panel.attachedSheet != nil { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertTrue(controller.handleKeyEvent(try keyEvent(126, "", in: panel)), "Keys return to the palette after the sheet")
    }

    // MARK: Layout

    func testEveryInteractivePreviewFitsItsReservedHeight() async throws {
        let settings = configuredSettings()
        func base64(_ text: String) -> String {
            Data(text.utf8).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
        }
        let jwt = base64("{\"alg\":\"HS256\",\"typ\":\"JWT\"}") + "." + base64("{\"sub\":\"example-user\",\"exp\":1893456000}") + ".example"
        var samples: [LauncherCommand: String] = [
            .json: "{\"name\":\"Bello Box\",\"id\":9007199254740993,\"tools\":[\"clock\",\"screenshot\"]}",
            .compare: "alpha\nbeta\ngamma", .jwt: jwt, .regex: "Fixed BOX-123 and API-456.",
            .url: "https://example.com/search?q=Bello%20Box&tag=swift&tag=macOS#results", .time: "2026-09-06T09:30:00Z",
            .cron: "*/15 9-17 * * MON-FRI", .convert: "name,active\nBello Box,true\nExample,false",
            .snippets: "Dear {{name}}, {{selection}} {{team}} {{topic}} {{extra}}", .http: "curl 'https://example.com' -H 'Accept: text/html'",
            .generate: "", .ai: "Some words to work with", .screenshot: "", .scrollCapture: "", .recording: "", .videoToGIF: "",
            .qr: "https://example.com", .textTools: "Hello there, world", .settings: "", .home: ""
        ]
        for kind in AdditionalUtilityKind.allCases { samples[kind.command] = kind.example }
        for (command, text) in samples {
            let model = model(text, settings: settings)
            defer { model.cancelAll() }
            focus(model, command)
            let session = try XCTUnwrap(model.expandedSession, "\(command)")
            if let workbench = session.workbench { try await waitUntil { workbench.result != nil || workbench.error != nil } }
            if let regex = session.workbench, command == .regex { regex.regexPattern = "[A-Z]+-\\d+"; regex.regexOutput = "Replace" }
            if let tools = session.textTools { tools.category = .hash }
            if let recording = session.recording { recording.options.outputFormat = .gif }
            try await waitUntil { model.expandedPreview != nil }
            let reserved = LauncherModel.previewHeight(for: command)
            // Measured without the reserved frame, so the content's own height is what counts.
            let view = LauncherInteractivePreviewView(command: command, session: session, preview: model.expandedPreview, height: nil,
                hostWindow: { nil }, onOpen: {}, onLaunch: { _ in }, onOpenSettings: {}, onFocusSearch: {}, onCopilotFieldReady: { _ in })
            let fitted = NSHostingView(rootView: view.frame(width: 644)).fittingSize.height
            if Self.hasFlexibleOutput(command) {
                // The output well measures as empty here and takes whatever is
                // left in the palette; the controls must leave it usable room.
                XCTAssertLessThanOrEqual(fitted, reserved - Self.minimumOutputHeight, "\(command) controls leave room for output (chrome \(fitted))")
            } else {
                XCTAssertLessThanOrEqual(fitted, reserved, "\(command) must fit the height the palette reserves (fitted \(fitted))")
                // New swatches/contrast/permission views have an intrinsic output
                // height: it is already included in fitted, unlike a text well.
                if command.additionalTool == nil { XCTAssertGreaterThan(fitted, reserved - 24, "\(command) should not leave a large gap (fitted \(fitted))") }
            }
        }
        // The enlarged QR card and the draft-limit notice fit their rows too.
        let dense = model(String(repeating: "b", count: 1_900), settings: settings)
        defer { dense.cancelAll() }
        focus(dense, .qr)
        let qr = try XCTUnwrap(dense.expandedSession?.qr)
        qr.isEnlarged = true
        let enlarged = LauncherInteractivePreviewView(command: .qr, session: dense.expandedSession, preview: nil, height: nil,
            hostWindow: { nil }, onOpen: {}, onLaunch: { _ in }, onOpenSettings: {}, onFocusSearch: {}, onCopilotFieldReady: { _ in })
        let enlargedHeight = NSHostingView(rootView: enlarged.frame(width: 644)).fittingSize.height
        XCTAssertLessThanOrEqual(enlargedHeight, dense.expandedPreviewHeight, "The enlarged code fits the grown row (\(enlargedHeight))")
        XCTAssertGreaterThan(enlargedHeight, dense.expandedPreviewHeight - 40)
        let big = model("x", settings: settings)
        defer { big.cancelAll() }
        focus(big, .json)
        let json = try XCTUnwrap(big.expandedSession?.workbench)
        json.input = String(repeating: "y", count: LauncherPreview.parsingByteLimit + 1)
        let notice = LauncherInteractivePreviewView(command: .json, session: big.expandedSession, preview: nil, height: nil,
            hostWindow: { nil }, onOpen: {}, onLaunch: { _ in }, onOpenSettings: {}, onFocusSearch: {}, onCopilotFieldReady: { _ in })
        XCTAssertLessThanOrEqual(NSHostingView(rootView: notice.frame(width: 644)).fittingSize.height, LauncherModel.previewHeight(for: .json))
    }
    static let minimumOutputHeight: CGFloat = 64
    static func hasFlexibleOutput(_ command: LauncherCommand) -> Bool {
        switch command {
        case .json, .compare, .jwt, .regex, .url, .time, .cron, .convert, .snippets, .http, .generate, .textTools: return true
        case .qr, .ai, .screenshot, .scrollCapture, .recording, .videoToGIF, .worldClock, .settings, .home, .color, .contrast, .chmod: return false
        default: return true
        }
    }
}
