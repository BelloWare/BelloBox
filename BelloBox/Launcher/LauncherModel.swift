import AppKit
import Combine
import SwiftUI

/// Extra state that travels with a command when it leaves the palette.
struct LauncherCommandContext: Equatable {
    /// What the World Clock preview was showing when Enter was pressed: the
    /// instant, the chosen reference, and the ephemeral copilot conversation.
    var worldClock: WorldClockHandoff?
}

@MainActor
final class LauncherModel: ObservableObject {
    @Published private(set) var selection: TextSelection
    @Published private(set) var context: LauncherSelectionContext
    let snippets: SnippetStore
    private let defaults: UserDefaults
    private let settings: AppSettings
    private let worldClockPreferences: WorldClockPreferencesStore
    private var workbenches: [LauncherCommand: UtilityWorkbenchModel] = [:]
    typealias PreviewBuilder = @Sendable (String, LauncherCommand, [String]) throws -> LauncherPreview
    private let previewBuilder: PreviewBuilder
    private var previewTask: Task<Void, Never>?
    private var previewRunID = UUID()
    private var clockObservers: [AnyCancellable] = []
    private var clockTranscriptVisible = false
    @Published private(set) var preview: LauncherPreview?
    /// Interactive World Clock planner for a recognized timestamp. It never
    /// persists locations and is discarded with the selection.
    @Published private(set) var clockPreview: WorldClockViewModel?
    @Published var query = "" {
        didSet { selectedID = commands.first?.id; onPresentationChange() }
    }
    @Published var selectedID: String?
    @Published var contextMessage: String?
    @Published var workbench: UtilityWorkbenchModel?
    @Published private(set) var favorites: Set<String>
    @Published private(set) var recents: [String]
    var onCommand: (LauncherCommand, TextSelection, LauncherCommandContext) -> Void = { _, _, _ in }
    var onClose: () -> Void = {}
    var onPresentationChange: () -> Void = {}
    /// The featured preview changed height (copilot transcript shown or
    /// cleared). Hosts resize without touching keyboard focus; `paletteSize`
    /// already reflects the new height when this fires.
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
         previewBuilder: @escaping PreviewBuilder = { try LauncherPreview.make(text: $0, command: $1, zoneIDs: $2) }) {
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
        selectedID = commands.first?.id
        preparePreview()
    }
    deinit { previewTask?.cancel() }
    var suggestions: [LauncherCommand] { context.suggestions }
    var commands: [LauncherCommand] {
        LauncherCommand.search(query, input: "", favorites: favorites, recents: recents, suggested: suggestions)
    }
    var selectedCommand: LauncherCommand? { commands.first { $0.id == selectedID } }
    var featuredCommand: LauncherCommand? {
        guard query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, context.hasText, !context.exceedsLimit else { return nil }
        return suggestions.first
    }
    /// Whether the featured row is the interactive clock, so arrow keys nudge time.
    var featuresClock: Bool { featuredCommand == .worldClock }
    static let previewHeight: CGFloat = 150
    /// Measured natural height of `LauncherClockPreviewView` at the palette
    /// width; `testClockPreviewLayoutFitsTheHeightThePaletteReserves` keeps it honest.
    static let clockPreviewHeight: CGFloat = 261
    static let copilotTranscriptHeight: CGFloat = WorldClockCopilotView.compactTranscriptHeight + 6
    var featuredPreviewHeight: CGFloat {
        guard featuresClock else { return Self.previewHeight }
        let transcript = clockPreview?.copilot.isTranscriptVisible ?? false
        return Self.clockPreviewHeight + (transcript ? Self.copilotTranscriptHeight : 0)
    }
    var paletteSize: NSSize {
        let contextHeight: CGFloat = context.hasText || contextMessage != nil ? 48 : 0
        let featured = featuredCommand != nil
        let listHeight: CGFloat = commands.isEmpty ? 130 : CGFloat(min(featured ? 5 : 7, commands.count)) * 42 + 12 + (featured ? featuredPreviewHeight : 0)
        return NSSize(width: 680, height: 64 + contextHeight + 26 + listHeight + 42)
    }
    func useClipboard(_ text: String? = NSPasteboard.general.string(forType: .string)) {
        guard let text, !text.isEmpty else {
            contextMessage = "The clipboard has no text."; onPresentationChange(); return
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
        self.selection = context.usableSelection(selection)
        contextMessage = nil
        query = ""
        preparePreview()
    }
    private func preparePreview() {
        previewTask?.cancel()
        previewRunID = UUID()
        preview = nil
        discardClockPreview()
        guard context.hasText, !context.exceedsLimit, let command = suggestions.first else { return }
        let id = previewRunID, text = selection.text, builder = previewBuilder
        let zones = worldClockPreferences.loadZoneIDs()
        previewTask = Task { [weak self] in
            let worker = Task.detached(priority: .userInitiated) { try builder(text, command, zones) }
            do {
                let value = try await withTaskCancellationHandler { try await worker.value } onCancel: { worker.cancel() }
                guard let self, self.previewRunID == id, !Task.isCancelled else { return }
                self.preview = value
                if command == .worldClock, case let .clocks(_, instant) = value.content { self.installClockPreview(at: instant, savedZones: zones) }
            } catch {
                guard let self, self.previewRunID == id, !Task.isCancelled else { return }
                self.preview = LauncherPreview.failure(error, command: command)
            }
        }
    }
    private func installClockPreview(at instant: Date, savedZones: [String]) {
        let zoneIDs = WorldClockViewModel.previewZoneIDs(saved: savedZones, localZone: .current)
        let anchor = worldClockPreferences.loadAnchorZoneID(validZoneIDs: zoneIDs)
        let clock = WorldClockViewModel(settings: settings, seedDate: instant, preferences: worldClockPreferences,
                                        mode: .preview, zoneIDs: zoneIDs, anchorZoneID: anchor, askCopilot: clockResponder)
        clockPreview = clock
        clockTranscriptVisible = clock.copilot.isTranscriptVisible
        // The palette grows when the transcript appears and shrinks when it is
        // cleared. The session reports after its state settled, so the size
        // read inside the callback is the new one.
        clock.copilot.onStateChange = { [weak self] in self?.reconcileClockPreviewSize() }
        clockObservers = [
            clock.objectWillChange.sink { [weak self] in self?.objectWillChange.send() },
            clock.copilot.objectWillChange.sink { [weak self] in self?.objectWillChange.send() },
        ]
    }
    private func reconcileClockPreviewSize() {
        let visible = clockPreview?.copilot.isTranscriptVisible ?? false
        guard visible != clockTranscriptVisible else { return }
        clockTranscriptVisible = visible
        onPreviewResize()
    }
    private func discardClockPreview() {
        clockObservers = []
        clockPreview?.copilot.onStateChange = {}
        clockPreview?.cancelAI()
        clockPreview = nil
        clockTranscriptVisible = false
    }
    /// What Enter hands to the dedicated World Clock: the previewed instant
    /// and reference plus the copilot conversation, all in memory only.
    var worldClockHandoff: WorldClockHandoff? {
        if let clockPreview {
            return WorldClockHandoff(instant: clockPreview.selectedInstant, anchorZoneID: clockPreview.anchorZoneID,
                                     copilot: clockPreview.copilot.snapshot())
        }
        if case let .clocks(_, instant)? = preview?.content { return WorldClockHandoff(instant: instant) }
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
            onPresentationChange()
        } else {
            onCommand(command, selection, LauncherCommandContext(worldClock: command == .worldClock ? worldClockHandoff : nil))
        }
    }
    /// The instant World Clock should open at: the previewed time if the user
    /// scrubbed it, otherwise the recognized timestamp.
    var previewedInstant: Date? {
        if let clockPreview { return clockPreview.selectedInstant }
        if case let .clocks(_, instant)? = preview?.content { return instant }
        return nil
    }
    func back() {
        if workbench?.busy == true { workbench?.cancel() }
        workbench = nil
        onPresentationChange()
    }
    func cancelAll() {
        previewTask?.cancel(); previewTask = nil; previewRunID = UUID(); preview = nil
        discardClockPreview()
        workbenches.values.forEach { $0.cancel() }
    }
}
