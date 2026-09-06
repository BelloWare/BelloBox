import AppKit
import SwiftUI

@MainActor
final class LauncherModel: ObservableObject {
    @Published private(set) var selection: TextSelection
    @Published private(set) var context: LauncherSelectionContext
    let snippets: SnippetStore
    private let defaults: UserDefaults
    private var workbenches: [LauncherCommand: UtilityWorkbenchModel] = [:]
    typealias PreviewBuilder = @Sendable (String, LauncherCommand, [String]) throws -> LauncherPreview
    private let previewBuilder: PreviewBuilder
    private var previewTask: Task<Void, Never>?
    private var previewRunID = UUID()
    @Published private(set) var preview: LauncherPreview?
    @Published var query = "" {
        didSet { selectedID = commands.first?.id; onPresentationChange() }
    }
    @Published var selectedID: String?
    @Published var contextMessage: String?
    @Published var workbench: UtilityWorkbenchModel?
    @Published private(set) var favorites: Set<String>
    @Published private(set) var recents: [String]
    var onCommand: (LauncherCommand, TextSelection) -> Void = { _, _ in }
    var onClose: () -> Void = {}
    var onPresentationChange: () -> Void = {}
    var pinnedText: () -> String? = { nil }
    var pinText: (String) -> Void = { _ in }

    init(selection: TextSelection, snippets: SnippetStore, defaults: UserDefaults = .standard,
         previewBuilder: @escaping PreviewBuilder = { try LauncherPreview.make(text: $0, command: $1, zoneIDs: $2) }) {
        let context = LauncherSelectionContext(text: selection.text)
        self.context = context
        self.selection = context.usableSelection(selection)
        self.snippets = snippets; self.defaults = defaults
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
    static let previewHeight: CGFloat = 150
    var paletteSize: NSSize {
        let contextHeight: CGFloat = context.hasText || contextMessage != nil ? 48 : 0
        let featured = featuredCommand != nil
        let listHeight: CGFloat = commands.isEmpty ? 130 : CGFloat(min(featured ? 5 : 7, commands.count)) * 42 + 12 + (featured ? Self.previewHeight : 0)
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
        guard context.hasText, !context.exceedsLimit, let command = suggestions.first else { return }
        let id = previewRunID, text = selection.text, builder = previewBuilder
        let zones = WorldClockPreferencesStore(defaults: defaults).loadZoneIDs()
        previewTask = Task { [weak self] in
            let worker = Task.detached(priority: .userInitiated) { try builder(text, command, zones) }
            do {
                let value = try await withTaskCancellationHandler { try await worker.value } onCancel: { worker.cancel() }
                guard let self, self.previewRunID == id, !Task.isCancelled else { return }
                self.preview = value
            } catch {
                guard let self, self.previewRunID == id, !Task.isCancelled else { return }
                self.preview = LauncherPreview.failure(error, command: command)
            }
        }
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
        } else { onCommand(command, selection) }
    }
    func back() {
        if workbench?.busy == true { workbench?.cancel() }
        workbench = nil
        onPresentationChange()
    }
    func cancelAll() {
        previewTask?.cancel(); previewTask = nil; previewRunID = UUID(); preview = nil
        workbenches.values.forEach { $0.cancel() }
    }
}
