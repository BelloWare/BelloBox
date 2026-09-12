import AppKit
import Combine
import SwiftUI

/// The palette expands exactly one row: the focused command. Moving focus
/// with the arrows collapses the previous row and shows the new command's
/// preview, so what is expanded is always what Enter opens. Each focused row
/// gets an interactive session (`LauncherInteractivePreview`) that keeps its
/// draft for the palette session; a static summary (`LauncherPreview`) is
/// computed off the main actor once per selection and command. Stale work is
/// discarded, and a new selection drops every session.
@MainActor
final class LauncherModel: ObservableObject {
    @Published private(set) var selection: TextSelection
    @Published private(set) var context: LauncherSelectionContext
    let snippets: SnippetStore
    private let defaults: UserDefaults
    private let settings: AppSettings
    private let worldClockPreferences: WorldClockPreferencesStore
    private var workbenches: [LauncherCommand: UtilityWorkbenchModel] = [:]
    typealias PreviewBuilder = @Sendable (String, LauncherCommand, LauncherPreviewContext) throws -> LauncherPreview
    private let previewBuilder: PreviewBuilder
    private var previewTask: Task<Void, Never>?
    private var previewTaskCommand: LauncherCommand?
    /// Changes with the selection; results from an earlier generation are dropped.
    private var previewGeneration = UUID()
    private var sessionObservers: [LauncherCommand: AnyCancellable] = [:]
    private var lastReportedHeight: CGFloat = 0
    private var isHandlingQueryChange = false
    /// Previews built for the current selection, one per command that was focused.
    @Published private(set) var previews: [LauncherCommand: LauncherPreview] = [:]
    /// Interactive sessions for the current selection, one per command that was focused.
    @Published private(set) var sessions: [LauncherCommand: LauncherInteractivePreview] = [:]
    @Published var query = "" {
        didSet {
            isHandlingQueryChange = true
            selectedID = commands.first?.id
            isHandlingQueryChange = false
            presentationChanged()
        }
    }
    @Published var selectedID: String? {
        didSet { if selectedID != oldValue { selectionDidChange() } }
    }
    @Published var contextMessage: String?
    @Published var workbench: UtilityWorkbenchModel?
    @Published private(set) var favorites: Set<String>
    @Published private(set) var recents: [String]
    /// Snapshot for this selection: opening a tool teaches the next palette,
    /// without moving the rows while someone is navigating the current one.
    private var learnedScores: [String: Int] = [:]
    var onCommand: (LauncherCommand, TextSelection, LauncherCommandContext) -> Void = { _, _, _ in }
    var onClose: () -> Void = {}
    var onPresentationChange: () -> Void = {}
    /// The expanded row changed height (another command was focused, or the
    /// copilot transcript was shown or cleared). Hosts resize without touching
    /// keyboard focus; `paletteSize` already reflects the new height when this fires.
    var onPreviewResize: () -> Void = {}
    /// Asks the host to return keyboard focus to the search field.
    var onFocusSearch: () -> Void = {}
    /// The panel hosting the palette, for sheets (Save…) that must stay attached.
    var hostWindow: () -> NSWindow? = { nil }
    var pinnedText: () -> String? = { nil }
    var pinText: (String) -> Void = { _ in }

    /// Test seam: answers copilot questions without a provider or network.
    private let clockResponder: WorldClockCopilotSession.Responder?

    init(selection: TextSelection, snippets: SnippetStore, defaults: UserDefaults = .standard,
         settings: AppSettings = .shared, worldClockPreferences: WorldClockPreferencesStore? = nil,
         clockResponder: WorldClockCopilotSession.Responder? = nil,
         previewBuilder: @escaping PreviewBuilder = { try LauncherPreview.make(text: $0, command: $1, context: $2) }) {
        let context = LauncherSelectionContext(text: selection.text)
        self.context = context
        self.selection = context.usableSelection(selection)
        self.snippets = snippets; self.defaults = defaults
        self.settings = settings
        self.worldClockPreferences = worldClockPreferences ?? WorldClockPreferencesStore(defaults: defaults)
        self.clockResponder = clockResponder
        self.previewBuilder = previewBuilder
        favorites = Set(defaults.stringArray(forKey: "launcherFavorites") ?? ["json", "compare", "screenshot", "worldClock"])
        recents = defaults.stringArray(forKey: "launcherRecents") ?? []
        learnedScores = LauncherUsageStore(defaults: defaults).scores(for: context.contentKind)
        selectedID = commands.first?.id
        // The first row's session exists before the initial height is read.
        ensurePreviewForSelection()
        lastReportedHeight = paletteSize.height
    }
    deinit { previewTask?.cancel() }
    var suggestions: [LauncherCommand] { context.suggestions }
    var commands: [LauncherCommand] {
        LauncherCommand.search(query, input: "", favorites: favorites, recents: recents, suggested: suggestions, learnedScores: learnedScores)
    }
    var selectedCommand: LauncherCommand? { commands.first { $0.id == selectedID } }
    /// The top interpretation of the selection, labelled "Best match" while the
    /// search is empty. It does not move with the focus.
    var bestMatch: LauncherCommand? {
        guard query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, context.hasText, !context.exceedsLimit else { return nil }
        return commands.first
    }
    /// The row that shows its preview: the focused command.
    var expandedCommand: LauncherCommand? { selectedCommand }
    var expandedPreview: LauncherPreview? { expandedCommand.flatMap { previews[$0] } }
    /// The focused row's interactive session, once it exists.
    var expandedSession: LauncherInteractivePreview? { expandedCommand.flatMap { sessions[$0] } }
    func preview(for command: LauncherCommand) -> LauncherPreview? { previews[command] }
    func session(for command: LauncherCommand) -> LauncherInteractivePreview? { sessions[command] }
    /// The World Clock planner, when it has been installed for this selection.
    var clockPreview: WorldClockViewModel? { sessions[.worldClock]?.clock }
    /// The selection is a timestamp, so World Clock ranks first and its planner
    /// opens at that instant instead of following the current time.
    var isTimestampSelection: Bool { context.hasText && !context.exceedsLimit && suggestions.first == .worldClock }
    /// The World Clock row is focused; its planner height is reserved
    /// synchronously so the palette never jumps when the planner arrives.
    var expandsClock: Bool { expandedCommand == .worldClock }
    /// Whether the expanded row is the interactive clock, so arrow keys nudge time.
    var featuresClock: Bool { expandsClock && clockPreview != nil }
    /// Whether the selection is small enough for a preview to process it. Larger
    /// selections show a notice and open the complete text in the full tool.
    var fitsPreviewLimit: Bool { context.fitsPreviewLimit }
    /// Height of the static (summary-only) preview, used for notices and while a
    /// session is being prepared.
    static let previewHeight: CGFloat = 150
    /// Measured natural height of `LauncherClockPreviewView` at the palette
    /// width; `testClockPreviewLayoutFitsTheHeightThePaletteReserves` keeps it honest.
    static let clockPreviewHeight: CGFloat = 261
    static let copilotTranscriptHeight: CGFloat = WorldClockCopilotView.compactTranscriptHeight + 6
    /// The height each interactive preview reserves. Measured by
    /// `LauncherInteractivePreviewTests.testEveryInteractivePreviewFitsItsReservedHeight`.
    static func previewHeight(for command: LauncherCommand) -> CGFloat {
        switch command {
        case .jsonSchema, .jsonMerge, .jsonRedact, .jsonLines, .csvExplore, .envFile, .plist, .sqlFormat, .httpHeaders, .cookies, .certificate, .sshKey, .uuidInspect, .bitwise, .statistics, .dateMath, .aspectRatio, .bezier, .boxShadow, .textTable: return command.additionalTool!.extendedDefinition!.height
        case .calculator, .subnet: return 232
        case .units, .numberBase, .color, .contrast, .gradient: return 256
        case .chmod, .listSet: return 280
        case .markdown, .jsonPointer, .jsonFlatten, .jsonCode, .sqlInsert, .xmlJSON, .unicode, .stringEscape, .extract, .semver, .hmac: return 304
        case .worldClock: return clockPreviewHeight
        case .json, .compare, .jwt, .regex, .url, .time, .cron, .convert, .snippets, .http, .generate: return 224
        case .qr, .textTools: return 224
        case .ai: return 152
        case .recording: return 136
        case .screenshot, .scrollCapture: return 116
        case .settings, .home: return 104
        case .videoToGIF: return 92
        }
    }
    var expandedPreviewHeight: CGFloat {
        guard let command = expandedCommand else { return Self.previewHeight }
        if command == .worldClock {
            let transcript = clockPreview?.copilot.isTranscriptVisible ?? false
            return Self.clockPreviewHeight + (transcript ? Self.copilotTranscriptHeight : 0)
        }
        // A text tool without a session (rejected or oversized selection) only gets the compact notice.
        if Self.consumesText(command), sessions[command] == nil { return Self.previewHeight }
        if command == .qr { return Self.previewHeight(for: command) + (sessions[.qr]?.qr?.extraHeight ?? 0) }
        return Self.previewHeight(for: command)
    }
    /// Commands whose preview works on the selection, so an oversized one is
    /// shown as a notice instead of being handed to editors and parsers.
    static func consumesText(_ command: LauncherCommand) -> Bool {
        switch command {
        case .calculator, .units, .numberBase, .color, .contrast, .gradient, .markdown, .jsonPointer, .jsonFlatten, .jsonCode, .sqlInsert, .xmlJSON, .unicode, .stringEscape, .extract, .listSet, .semver, .subnet, .chmod, .hmac, .jsonSchema, .jsonMerge, .jsonRedact, .jsonLines, .csvExplore, .envFile, .plist, .sqlFormat, .httpHeaders, .cookies, .certificate, .sshKey, .uuidInspect, .bitwise, .statistics, .dateMath, .aspectRatio, .bezier, .boxShadow, .textTable: return true
        case .json, .compare, .jwt, .regex, .url, .time, .cron, .convert, .snippets, .http, .qr, .textTools: return true
        case .generate, .ai, .screenshot, .scrollCapture, .recording, .videoToGIF, .worldClock, .settings, .home: return false
        }
    }
    var paletteSize: NSSize {
        let contextHeight: CGFloat = context.hasText || contextMessage != nil ? 48 : 0
        let expanded = expandedCommand != nil
        let listHeight: CGFloat = commands.isEmpty ? 130 : CGFloat(min(5, commands.count)) * 42 + 12 + (expanded ? expandedPreviewHeight : 0)
        return NSSize(width: 680, height: 64 + contextHeight + 26 + listHeight + 42)
    }
    func useClipboard(_ text: String? = NSPasteboard.general.string(forType: .string)) {
        guard let text, !text.isEmpty else {
            contextMessage = "The clipboard has no text."; presentationChanged(); return
        }
        replaceContext(TextSelection(text: text, anchorRect: nil, appName: "Clipboard", bundleID: nil, pid: nil))
    }
    func clearSelection() {
        replaceContext(TextSelection(text: "", anchorRect: nil, appName: nil, bundleID: nil, pid: nil))
    }
    private func replaceContext(_ selection: TextSelection) {
        cancelAll()
        workbench = nil
        workbenches = [:]
        context = LauncherSelectionContext(text: selection.text)
        learnedScores = LauncherUsageStore(defaults: defaults).scores(for: context.contentKind)
        self.selection = context.usableSelection(selection)
        contextMessage = nil
        query = ""
        ensurePreviewForSelection()
    }
    private func presentationChanged() {
        lastReportedHeight = paletteSize.height
        onPresentationChange()
    }
    private func selectionDidChange() {
        ensurePreviewForSelection()
        if !isHandlingQueryChange { reconcilePaletteHeight() }
    }
    /// Reports a height change through the no-refocus path, once per change.
    private func reconcilePaletteHeight() {
        let height = paletteSize.height
        guard height != lastReportedHeight else { return }
        lastReportedHeight = height
        onPreviewResize()
    }
    /// Starts the focused command's summary unless it is cached or already
    /// running, and installs its interactive session. Work for a command that
    /// lost focus is cancelled and discarded; sessions stay.
    private func ensurePreviewForSelection() {
        guard let command = expandedCommand else { cancelPreviewTask(); return }
        ensureSession(for: command)
        if previews[command] != nil { cancelPreviewTask(); return }
        if previewTask != nil, previewTaskCommand == command { return }
        cancelPreviewTask()
        let generation = previewGeneration, text = selection.text, builder = previewBuilder
        let previewContext = LauncherPreviewContext(
            zoneIDs: worldClockPreferences.loadZoneIDs(),
            snippetCount: snippets.snippets.count,
            aiProviderName: settings.isConfigured ? settings.providerKind.displayName : nil)
        previewTaskCommand = command
        previewTask = Task { [weak self] in
            let worker = Task.detached(priority: .userInitiated) { try builder(text, command, previewContext) }
            do {
                let value = try await withTaskCancellationHandler { try await worker.value } onCancel: { worker.cancel() }
                guard let self, self.previewGeneration == generation, !Task.isCancelled else { return }
                self.store(value, for: command, savedZones: previewContext.zoneIDs)
            } catch {
                guard let self, self.previewGeneration == generation, !Task.isCancelled else { return }
                self.store(LauncherPreview.failure(error, command: command), for: command, savedZones: previewContext.zoneIDs)
            }
        }
    }
    private func cancelPreviewTask() {
        previewTask?.cancel()
        previewTask = nil
        previewTaskCommand = nil
    }
    private func store(_ value: LauncherPreview, for command: LauncherCommand, savedZones: [String]) {
        previews[command] = value
        if previewTaskCommand == command { previewTask = nil; previewTaskCommand = nil }
        if command == .worldClock, clockPreview == nil, case let .clocks(_, instant) = value.content {
            installClockPreview(at: instant, savedZones: savedZones)
        }
    }

    // MARK: - Interactive sessions

    /// Creates the focused command's session on first focus. Creating one has
    /// no side effects beyond a bounded, cancellable preview calculation:
    /// nothing is recorded as a use, saved, copied, or sent.
    private func ensureSession(for command: LauncherCommand) {
        guard sessions[command] == nil else { return }
        // Rejected and oversized selections only get the compact notice; the
        // full tool opens with the complete text on Enter, never a truncation.
        // A tool that was opened already has a draft of its own: its session
        // follows that draft (a notice while it is large, the preview once it
        // was edited down), always on the same model instance.
        if Self.consumesText(command), workbenches[command] == nil, !fitsPreviewLimit { return }
        let session: LauncherInteractivePreview
        switch command {
        case .json, .compare, .jwt, .regex, .url, .time, .cron, .convert, .snippets, .http, .generate, .calculator, .units, .numberBase, .color, .contrast, .gradient, .markdown, .jsonPointer, .jsonFlatten, .jsonCode, .sqlInsert, .xmlJSON, .unicode, .stringEscape, .extract, .listSet, .semver, .subnet, .chmod, .hmac, .jsonSchema, .jsonMerge, .jsonRedact, .jsonLines, .csvExplore, .envFile, .plist, .sqlFormat, .httpHeaders, .cookies, .certificate, .sshKey, .uuidInspect, .bitwise, .statistics, .dateMath, .aspectRatio, .bezier, .boxShadow, .textTable:
            let tool = workbenchModel(for: command)
            tool.previewsOnly = true
            if tool.result == nil, tool.error == nil, !tool.busy { tool.schedule() }
            session = .workbench(tool)
        case .worldClock:
            return // Installed with the parsed instant by `store`.
        case .qr:
            let qr = LauncherQRPreview(text: selection.text)
            // Enlarging the code grows the row; the host resizes once, without refocusing.
            qr.onStateChange = { [weak self] in self?.reconcilePaletteHeight() }
            session = .qr(qr)
        case .textTools:
            session = .textTools(TextToolsPopupViewModel(selection: selection, settings: settings, accessibility: AccessibilityService()))
        case .ai:
            session = .ai(LauncherAIPreview(text: selection.text, settings: settings))
        case .screenshot, .scrollCapture:
            session = .capture(LauncherCapturePreview(command: command, settings: settings))
        case .recording:
            session = .recording(LauncherRecordingPreview(options: settings.recordingOptions))
        case .videoToGIF:
            session = .videoToGIF(LauncherGIFPreview(options: settings.gifExportOptions))
        case .settings, .home:
            session = .appStatus(LauncherAppStatusPreview(settings: settings))
        }
        install(session, for: command)
    }
    private func install(_ session: LauncherInteractivePreview, for command: LauncherCommand) {
        sessions[command] = session
        sessionObservers[command] = session.objectWillChange.sink { [weak self] in self?.objectWillChange.send() }
    }
    /// The developer tool's model for this selection: the preview edits it and
    /// Enter opens the same instance.
    private func workbenchModel(for command: LauncherCommand) -> UtilityWorkbenchModel {
        if let existing = workbenches[command] { return existing }
        let tool = UtilityWorkbenchModel(command: command, selection: selection, snippets: snippets,
            inputNotice: context.exceedsLimit ? LauncherSelectionContext.limitNotice : nil)
        // Forwarded, not copied: the host installs the pin storage after the
        // first row (which may be Compare) already has its session.
        tool.pinnedText = { [weak self] in self?.pinnedText() }
        tool.pinText = { [weak self] in self?.pinText($0) }
        tool.onReplace = { [weak self] text in
            guard let self, let pid = self.selection.pid else { return }
            self.onClose()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { AccessibilityService().replaceSelection(with: text, pid: pid) }
        }
        workbenches[command] = tool
        return tool
    }
    private func installClockPreview(at instant: Date, savedZones: [String]) {
        let zoneIDs = WorldClockViewModel.previewZoneIDs(saved: savedZones, localZone: .current)
        let anchor = worldClockPreferences.loadAnchorZoneID(validZoneIDs: zoneIDs)
        // Without a timestamp the planner follows the current time, like the
        // dedicated window does when it opens live.
        let clock = WorldClockViewModel(settings: settings, seedDate: isTimestampSelection ? instant : nil, preferences: worldClockPreferences,
                                        mode: .preview, zoneIDs: zoneIDs, anchorZoneID: anchor, askCopilot: clockResponder)
        // The palette grows when the transcript appears and shrinks when it is
        // cleared. The session reports after its state settled, so the size
        // read inside the callback is the new one.
        clock.copilot.onStateChange = { [weak self] in self?.reconcilePaletteHeight() }
        install(.clock(clock), for: .worldClock)
    }
    /// What Enter hands to the dedicated World Clock: exactly what the row
    /// shows. A planned instant or explicit live intent, the reference, and
    /// the copilot conversation (empty or not), all in memory only, so an
    /// already open window adopts the preview instead of keeping an older plan.
    var worldClockHandoff: WorldClockHandoff? {
        if let clockPreview {
            return WorldClockHandoff(instant: clockPreview.isFollowingNow ? nil : clockPreview.selectedInstant,
                                     anchorZoneID: clockPreview.anchorZoneID, copilot: clockPreview.copilot.snapshot(),
                                     followsNow: clockPreview.isFollowingNow)
        }
        if isTimestampSelection, case let .clocks(_, instant)? = previews[.worldClock]?.content { return WorldClockHandoff(instant: instant) }
        return nil
    }
    /// Arrow keys nudge the previewed time while the search field is empty.
    func nudgeClock(by steps: Int, step: TimeInterval) {
        guard featuresClock, let clockPreview else { return }
        clockPreview.nudge(by: steps, step: step)
    }
    func toggleFavorite(_ command: LauncherCommand) {
        if favorites.contains(command.id) { favorites.remove(command.id) } else { favorites.insert(command.id) }
        defaults.set(favorites.sorted(), forKey: "launcherFavorites")
    }
    func move(_ direction: Int) {
        let commands = commands
        guard !commands.isEmpty else { return }
        let index = commands.firstIndex(where: { $0.id == selectedID }) ?? 0
        selectedID = commands[((index + direction) % commands.count + commands.count) % commands.count].id
    }
    func openSelected() { if let command = selectedCommand ?? commands.first { open(command) } }
    /// Opens a command with what its preview was showing. `customize` lets a
    /// preview button choose a specific action (a capture mode, a quick action).
    func open(_ command: LauncherCommand, customize: (inout LauncherCommandContext) -> Void = { _ in }) {
        selectedID = command.id
        LauncherUsageStore(defaults: defaults).record(command, kind: context.contentKind)
        recents = [command.id] + recents.filter { $0 != command.id }.prefix(7)
        defaults.set(recents, forKey: "launcherRecents")
        if command.isDeveloperTool {
            let tool = workbenchModel(for: command)
            tool.previewsOnly = false
            workbench = tool
            if tool.result == nil && tool.error == nil && !tool.busy && command != .http { tool.schedule() }
            presentationChanged()
        } else {
            var handoff = context(for: command)
            customize(&handoff)
            onCommand(command, launchSelection(for: command, context: handoff), handoff)
        }
    }
    /// The draft the focused preview hands to the full tool.
    func context(for command: LauncherCommand) -> LauncherCommandContext {
        var context = LauncherCommandContext()
        switch command {
        case .worldClock: context.worldClock = worldClockHandoff
        case .qr:
            if let qr = sessions[.qr]?.qr, qr.text != selection.text { context.qrText = qr.text }
        case .textTools:
            if let tools = sessions[.textTools]?.textTools {
                context.textTools = TextToolsHandoff(category: tools.category, caseStyle: tools.caseStyle, encodeMethod: tools.encodeMethod,
                                                     decodeFormat: tools.decodeFormat, lineOp: tools.lineOp)
            }
        // Enter is Open: the prompt travels, nothing is sent.
        case .ai: context.ai = sessions[.ai]?.ai?.openHandoff
        case .recording: context.recording = sessions[.recording]?.recording?.options
        case .videoToGIF:
            if let gif = sessions[.videoToGIF]?.videoToGIF { context.videoToGIF = VideoToGIFHandoff(options: gif.options, chooseFile: false) }
        default: break
        }
        return context
    }
    /// The QR popup opens with the edited text; every other tool gets the selection.
    private func launchSelection(for command: LauncherCommand, context: LauncherCommandContext) -> TextSelection {
        guard command == .qr, let text = context.qrText else { return selection }
        return TextSelection(text: text, anchorRect: selection.anchorRect, appName: selection.appName, bundleID: selection.bundleID, pid: selection.pid)
    }
    /// The instant World Clock should open at: the previewed time if the user
    /// scrubbed it, otherwise the recognized timestamp.
    var previewedInstant: Date? {
        if let clockPreview { return isTimestampSelection || !clockPreview.isFollowingNow ? clockPreview.selectedInstant : nil }
        if isTimestampSelection, case let .clocks(_, instant)? = previews[.worldClock]?.content { return instant }
        return nil
    }
    func back() {
        if let tool = workbench {
            if tool.busy { tool.cancel() }
            tool.previewsOnly = true
            // A calculation interrupted by Back is restarted for the row when
            // the draft fits it; an oversized draft waits for the next open.
            if tool.result == nil, tool.error == nil, !tool.draftExceedsPreviewLimit { tool.schedule() }
        }
        workbench = nil
        // The tool's draft may now fit the row (or not); the row follows it.
        ensurePreviewForSelection()
        presentationChanged()
        onFocusSearch()
    }
    func cancelAll() {
        cancelPreviewTask()
        previewGeneration = UUID()
        previews = [:]
        for session in sessions.values { session.cancel() }
        if let clock = clockPreview { clock.copilot.onStateChange = {} }
        sessionObservers = [:]
        sessions = [:]
        workbenches.values.forEach { $0.cancel() }
    }
}
