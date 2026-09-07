import AppKit
import Combine
import SwiftUI

/// Extra state that travels with a command when it leaves the palette.
struct LauncherCommandContext: Equatable {
    /// What the World Clock preview was showing when Enter was pressed: the
    /// instant, the chosen reference, and the ephemeral copilot conversation.
    var worldClock: WorldClockHandoff?
}

/// The palette expands exactly one row: the focused command. Moving focus
/// with the arrows collapses the previous row and shows the new command's
/// preview, so what is expanded is always what Enter opens. Previews are
/// bounded, computed off the main actor once per selection and command, and
/// cached until the selection changes; stale work is discarded.
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
    private var clockObservers: [AnyCancellable] = []
    private var lastReportedHeight: CGFloat = 0
    private var isHandlingQueryChange = false
    /// Previews built for the current selection, one per command that was focused.
    @Published private(set) var previews: [LauncherCommand: LauncherPreview] = [:]
    /// Interactive World Clock planner for a recognized timestamp. It never
    /// persists locations and is discarded with the selection.
    @Published private(set) var clockPreview: WorldClockViewModel?
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
        lastReportedHeight = paletteSize.height
        ensurePreviewForSelection()
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
    func preview(for command: LauncherCommand) -> LauncherPreview? { previews[command] }
    /// The selection is a timestamp, so World Clock gets the interactive planner
    /// rather than static clocks. Known synchronously so the height never jumps
    /// when the planner arrives.
    var isTimestampSelection: Bool { context.hasText && !context.exceedsLimit && suggestions.first == .worldClock }
    var expandsClock: Bool { expandedCommand == .worldClock && isTimestampSelection }
    /// Whether the expanded row is the interactive clock, so arrow keys nudge time.
    var featuresClock: Bool { expandsClock && clockPreview != nil }
    static let previewHeight: CGFloat = 150
    /// Measured natural height of `LauncherClockPreviewView` at the palette
    /// width; `testClockPreviewLayoutFitsTheHeightThePaletteReserves` keeps it honest.
    static let clockPreviewHeight: CGFloat = 261
    static let copilotTranscriptHeight: CGFloat = WorldClockCopilotView.compactTranscriptHeight + 6
    var expandedPreviewHeight: CGFloat {
        guard expandsClock else { return Self.previewHeight }
        let transcript = clockPreview?.copilot.isTranscriptVisible ?? false
        return Self.clockPreviewHeight + (transcript ? Self.copilotTranscriptHeight : 0)
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
    /// Starts the focused command's preview unless it is cached or already
    /// running. Work for a command that lost focus is cancelled and discarded.
    private func ensurePreviewForSelection() {
        guard let command = expandedCommand else { cancelPreviewTask(); return }
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
        if command == .worldClock, clockPreview == nil, isTimestampSelection, case let .clocks(_, instant) = value.content {
            installClockPreview(at: instant, savedZones: savedZones)
        }
    }
    private func installClockPreview(at instant: Date, savedZones: [String]) {
        let zoneIDs = WorldClockViewModel.previewZoneIDs(saved: savedZones, localZone: .current)
        let anchor = worldClockPreferences.loadAnchorZoneID(validZoneIDs: zoneIDs)
        let clock = WorldClockViewModel(settings: settings, seedDate: instant, preferences: worldClockPreferences,
                                        mode: .preview, zoneIDs: zoneIDs, anchorZoneID: anchor, askCopilot: clockResponder)
        clockPreview = clock
        // The palette grows when the transcript appears and shrinks when it is
        // cleared. The session reports after its state settled, so the size
        // read inside the callback is the new one.
        clock.copilot.onStateChange = { [weak self] in self?.reconcilePaletteHeight() }
        clockObservers = [
            clock.objectWillChange.sink { [weak self] in self?.objectWillChange.send() },
            clock.copilot.objectWillChange.sink { [weak self] in self?.objectWillChange.send() },
        ]
    }
    private func discardClockPreview() {
        clockObservers = []
        clockPreview?.copilot.onStateChange = {}
        clockPreview?.cancelAI()
        clockPreview = nil
    }
    /// What Enter hands to the dedicated World Clock: the previewed instant
    /// and reference plus the copilot conversation, all in memory only. A
    /// selection without a timestamp opens the live clock instead.
    var worldClockHandoff: WorldClockHandoff? {
        if let clockPreview {
            return WorldClockHandoff(instant: clockPreview.selectedInstant, anchorZoneID: clockPreview.anchorZoneID,
                                     copilot: clockPreview.copilot.snapshot())
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
    func open(_ command: LauncherCommand) {
        selectedID = command.id
        LauncherUsageStore(defaults: defaults).record(command, kind: context.contentKind)
        recents = [command.id] + recents.filter { $0 != command.id }.prefix(7)
        defaults.set(recents, forKey: "launcherRecents")
        if command.isDeveloperTool {
            if let existing = workbenches[command] {
                workbench = existing
                if existing.result == nil && existing.error == nil && command != .http { existing.schedule() }
            } else {
                let tool = UtilityWorkbenchModel(command: command, selection: selection, snippets: snippets,
                    inputNotice: context.exceedsLimit ? LauncherSelectionContext.limitNotice : nil)
                tool.pinnedText = pinnedText; tool.pinText = pinText
                tool.onReplace = { [weak self] text in
                    guard let self, let pid = self.selection.pid else { return }
                    self.onClose()
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { AccessibilityService().replaceSelection(with: text, pid: pid) }
                }
                workbenches[command] = tool
                workbench = tool
                tool.schedule()
            }
            presentationChanged()
        } else {
            onCommand(command, selection, LauncherCommandContext(worldClock: command == .worldClock ? worldClockHandoff : nil))
        }
    }
    /// The instant World Clock should open at: the previewed time if the user
    /// scrubbed it, otherwise the recognized timestamp.
    var previewedInstant: Date? {
        if let clockPreview { return clockPreview.selectedInstant }
        if isTimestampSelection, case let .clocks(_, instant)? = previews[.worldClock]?.content { return instant }
        return nil
    }
    func back() {
        if workbench?.busy == true { workbench?.cancel() }
        workbench = nil
        presentationChanged()
    }
    func cancelAll() {
        cancelPreviewTask()
        previewGeneration = UUID()
        previews = [:]
        discardClockPreview()
        workbenches.values.forEach { $0.cancel() }
    }
}
