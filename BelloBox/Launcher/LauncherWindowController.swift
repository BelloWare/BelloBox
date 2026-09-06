import AppKit
import SwiftUI

@MainActor
final class LauncherModel: ObservableObject {
    @Published private(set) var selection: TextSelection
    let snippets: SnippetStore
    private let defaults: UserDefaults
    private var workbenches: [LauncherCommand: UtilityWorkbenchModel] = [:]
    @Published var query = "" { didSet { selectedID = commands.first?.id } }
    @Published var selectedID: String?
    @Published var contextMessage: String?
    @Published var workbench: UtilityWorkbenchModel?
    @Published private(set) var favorites: Set<String>
    @Published private(set) var recents: [String]
    var onCommand: (LauncherCommand, TextSelection) -> Void = { _, _ in }
    var onClose: () -> Void = {}
    var pinnedText: () -> String? = { nil }
    var pinText: (String) -> Void = { _ in }

    init(selection: TextSelection, snippets: SnippetStore, defaults: UserDefaults = .standard) {
        self.selection = selection; self.snippets = snippets; self.defaults = defaults
        favorites = Set(defaults.stringArray(forKey: "launcherFavorites") ?? ["json", "compare", "screenshot", "worldClock"])
        recents = defaults.stringArray(forKey: "launcherRecents") ?? []
        selectedID = commands.first?.id
    }
    var suggestions: [LauncherCommand] { LauncherCommand.suggestions(for: selection.text) }
    var commands: [LauncherCommand] { LauncherCommand.search(query, input: selection.text, favorites: favorites, recents: recents) }
    func useClipboard(_ text: String? = NSPasteboard.general.string(forType: .string)) {
        guard let text, !text.isEmpty else { contextMessage = "The clipboard has no text."; return }
        guard text.utf8.count <= UtilityLimits.inputBytes else { contextMessage = "Use up to 500 KB of clipboard text."; return }
        workbenches.values.forEach { $0.cancel() }
        workbenches = [:]
        selection = TextSelection(text: text, anchorRect: nil, appName: "Clipboard", bundleID: nil, pid: nil)
        contextMessage = nil
        query = ""
    }
    func toggleFavorite(_ command: LauncherCommand) {
        if favorites.contains(command.id) { favorites.remove(command.id) } else { favorites.insert(command.id) }
        defaults.set(favorites.sorted(), forKey: "launcherFavorites")
    }
    func move(_ direction: Int) {
        let commands = commands
        guard !commands.isEmpty else { return }
        let index = commands.firstIndex(where: { $0.id == selectedID }) ?? 0
        selectedID = commands[(index + direction + commands.count) % commands.count].id
    }
    func openSelected() { if let command = commands.first(where: { $0.id == selectedID }) ?? commands.first { open(command) } }
    func open(_ command: LauncherCommand) {
        recents = [command.id] + recents.filter { $0 != command.id }.prefix(7)
        defaults.set(recents, forKey: "launcherRecents")
        if command.isDeveloperTool {
            if let existing = workbenches[command] {
                workbench = existing
                if existing.result == nil && existing.error == nil && command != .http { existing.schedule() }
                return
            }
            let tool = UtilityWorkbenchModel(command: command, selection: selection, snippets: snippets)
            tool.pinnedText = pinnedText; tool.pinText = pinText
            tool.onReplace = { [weak self] text in
                guard let self, let pid = self.selection.pid else { return }
                self.onClose()
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { AccessibilityService().replaceSelection(with: text, pid: pid) }
            }
            workbenches[command] = tool
            workbench = tool
            tool.schedule()
        } else { onCommand(command, selection) }
    }
    func back() { if workbench?.busy == true { workbench?.cancel() }; workbench = nil }
}

@MainActor
final class LauncherWindowController: NSObject, NSWindowDelegate {
    private var panel: NSPanel?
    private var keyMonitor: Any?
    private var model: LauncherModel?
#if DEBUG
    private let snippets = SnippetStore(url: ProcessInfo.processInfo.environment["BELLOBOX_E2E_SNIPPETS_PATH"].map { URL(fileURLWithPath: $0) })
#else
    private let snippets = SnippetStore()
#endif
    private var pinned: String?
    var isVisible: Bool { panel?.isVisible == true }
    var onCommand: (LauncherCommand, TextSelection) -> Void = { _, _ in }

    func show(selection: TextSelection, initialCommand: LauncherCommand? = nil) {
        close()
        let model = LauncherModel(selection: selection, snippets: snippets)
        model.onClose = { [weak self] in self?.close() }
        model.onCommand = { [weak self] command, selection in
            self?.close(); self?.onCommand(command, selection)
        }
        model.pinnedText = { [weak self] in self?.pinned }
        model.pinText = { [weak self] in self?.pinned = $0 }
        let panel = NSPanel(contentRect: .zero, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        panel.title = "Bello Box"
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.delegate = self
        panel.contentViewController = NSHostingController(rootView: LauncherView(model: model))
        panel.setContentSize(NSSize(width: 860, height: 710))
        panel.contentMinSize = NSSize(width: 740, height: 560)
        let screen = ScreenPlacement.screen(containing: NSEvent.mouseLocation)
        let size = panel.frame.size
        panel.setFrameOrigin(ScreenPlacement.clamp(origin: CGPoint(x: screen.visibleFrame.midX - size.width / 2, y: screen.visibleFrame.midY - size.height / 2 + 40), size: size, into: screen))
        self.panel = panel; self.model = model
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self, weak panel] event in
            guard let self, let panel, event.window === panel else { return event }
            if event.keyCode == 53 {
                if model.workbench != nil { model.back() } else { self.close() }
                return nil
            }
            if event.modifierFlags.intersection([.command, .control, .option, .shift]) == .command && event.charactersIgnoringModifiers?.lowercased() == "k" {
                model.back(); model.query = ""; return nil
            }
            guard model.workbench == nil else { return event }
            if event.keyCode == 125 { model.move(1); return nil }
            if event.keyCode == 126 { model.move(-1); return nil }
            if event.keyCode == 36 { model.openSelected(); return nil }
            return event
        }
        AppActivation.bringAppForward()
        panel.makeKeyAndOrderFront(nil)
        if let initialCommand { model.open(initialCommand) }
    }
    func close() {
        model?.workbench?.cancel()
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor); self.keyMonitor = nil }
        panel?.delegate = nil
        panel?.close()
        panel = nil; model = nil
    }
    func windowWillClose(_ notification: Notification) { close() }
}

struct LauncherView: View {
    @ObservedObject var model: LauncherModel
    @FocusState private var searchFocused: Bool
    var body: some View {
        VStack(spacing: 0) {
            if let workbench = model.workbench {
                UtilityWorkbenchView(model: workbench, onBack: model.back)
            } else {
                search
                Divider()
                commandList
                Divider()
                HStack(spacing: 16) {
                    Label("Navigate", systemImage: "arrow.up.arrow.down")
                    Label("Open", systemImage: "return")
                    Spacer()
                    Text("Esc to close")
                    Text("\(model.commands.count) tools")
                }
                .font(.caption).foregroundStyle(.secondary).padding(16)
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
        .onAppear { searchFocused = true }
        .onChange(of: model.workbench == nil) { if $0 { searchFocused = true } }
    }
    private var search: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Image(systemName: "magnifyingglass").font(.title2).foregroundStyle(BoxTheme.accent)
                TextField("Search tools and commands…", text: $model.query)
                    .textFieldStyle(.plain).font(.system(size: 23, weight: .medium))
                    .focused($searchFocused)
                    .accessibilityIdentifier("launcherSearch")
                if !model.query.isEmpty {
                    Button { model.query = "" } label: { Image(systemName: "xmark.circle.fill") }
                        .buttonStyle(.plain).foregroundStyle(.secondary).accessibilityLabel("Clear search")
                }
            }
            if model.selection.text.isEmpty {
                HStack {
                    Text(model.contextMessage ?? "Your everyday tools, one shortcut away.").font(.caption).foregroundStyle(.secondary)
                    Spacer()
                    Button("Use Clipboard") { model.useClipboard() }.buttonStyle(.link).font(.caption)
                }
            } else {
                HStack {
                    Label("\(model.selection.text.count.formatted()) characters" + (model.selection.appName.map { " from \($0)" } ?? " selected"), systemImage: "text.cursor")
                    Spacer()
                    Text("Suggestions ready").foregroundStyle(BoxTheme.accent)
                }.font(.caption).foregroundStyle(.secondary)
                Text(model.selection.text.replacingOccurrences(of: "\n", with: " ").prefix(180))
                    .font(.system(.caption, design: .monospaced)).lineLimit(1).foregroundStyle(.secondary)
            }
        }.padding(22)
    }
    private var commandList: some View {
        ScrollViewReader { reader in
            ScrollView {
                LazyVStack(spacing: 3) {
                    if model.commands.isEmpty {
                        VStack(spacing: 12) {
                            Image(systemName: "magnifyingglass").font(.largeTitle)
                            Text("No matching tools").font(.headline)
                            Text("Try JSON, diff, URL, regex, capture, or time.").foregroundStyle(.secondary)
                            Button("Show all tools") { model.query = "" }.buttonStyle(SecondaryButtonStyle())
                        }.padding(50).frame(maxWidth: .infinity)
                    }
                    ForEach(model.commands) { command in
                        HStack(spacing: 12) {
                            Button { model.selectedID = command.id; model.open(command) } label: {
                                HStack(spacing: 12) {
                                    Image(systemName: command.symbol)
                                        .font(.system(size: 18, weight: .medium)).foregroundStyle(BoxTheme.accent)
                                        .frame(width: 38, height: 38).background(RoundedRectangle(cornerRadius: 10).fill(BoxTheme.accentSoft))
                                    VStack(alignment: .leading, spacing: 3) {
                                        Text(command.title).font(.system(size: 14, weight: .semibold))
                                        Text(command.subtitle).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                                    }
                                    Spacer()
                                    if model.query.isEmpty && model.suggestions.contains(command) {
                                        Text("Suggested").font(.caption2.weight(.medium)).foregroundStyle(BoxTheme.accent)
                                    }
                                }.contentShape(Rectangle())
                            }.buttonStyle(.plain).accessibilityIdentifier("command_\(command.id)")
                            Button { model.toggleFavorite(command) } label: {
                                Image(systemName: model.favorites.contains(command.id) ? "star.fill" : "star")
                                    .foregroundStyle(model.favorites.contains(command.id) ? BoxTheme.accent : Color.secondary.opacity(0.5))
                            }.buttonStyle(.plain).help("Add or remove favorite").accessibilityLabel("Favorite \(command.title)")
                        }
                        .padding(.horizontal, 12).padding(.vertical, 10)
                        .background(RoundedRectangle(cornerRadius: 11).fill(model.selectedID == command.id ? BoxTheme.accentSoft : Color.clear))
                        .overlay(RoundedRectangle(cornerRadius: 11).stroke(model.selectedID == command.id ? BoxTheme.accent.opacity(0.25) : .clear))
                        .accessibilityValue(model.selectedID == command.id ? "Selected" : "Not selected")
                        .id(command.id)
                    }
                }.padding(12)
            }
            .onChange(of: model.selectedID) { id in if let id { reader.scrollTo(id, anchor: .center) } }
        }
    }
}
