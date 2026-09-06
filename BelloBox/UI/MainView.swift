import AppKit
import SwiftUI

enum HomeCategory: String, CaseIterable, Identifiable {
    case overview = "Overview", developer = "Developer", capture = "Capture", text = "Text & AI"
    var id: Self { self }
    var symbol: String {
        switch self { case .overview: return "square.grid.2x2"; case .developer: return "curlybraces"; case .capture: return "camera.viewfinder"; case .text: return "text.alignleft" }
    }
    var commands: [LauncherCommand] {
        switch self {
        case .overview: return [.screenshot, .recording, .worldClock, .json, .textTools, .qr, .ai, .snippets, .compare]
        case .developer: return LauncherCommand.allCases.filter(\.isDeveloperTool)
        case .capture: return [.screenshot, .scrollCapture, .recording]
        case .text: return [.ai, .textTools, .qr, .worldClock, .snippets, .compare]
        }
    }
    var subtitle: String {
        switch self {
        case .overview: return "A focused space for the things you do every day."
        case .developer: return "Inspect, transform, and build. Your tools stay close."
        case .capture: return "Capture a moment, a longer page, or the whole workflow."
        case .text: return "Give words, ideas, and time a little more structure."
        }
    }
}

struct MainView: View {
    @ObservedObject var settings: AppSettings
    var canCheckForUpdates: Bool
    var onOpenSettings: () -> Void
    var onOpenGuide: () -> Void
    var onOpenLauncher: () -> Void
    var onCapture: () -> Void
    var onScrollCapture: () -> Void
    var onRecord: () -> Void
    var onOpenQR: () -> Void
    var onOpenTextTools: () -> Void
    var onOpenWorldClock: () -> Void
    var onCheckForUpdates: () -> Void
    var onOpenTool: (LauncherCommand) -> Void = { _ in }

    @State private var category: HomeCategory = .overview
    @State private var trusted = AccessibilityService.isTrusted
    private let timer = Timer.publish(every: 1.5, on: .main, in: .common).autoconnect()

    var body: some View {
        HStack(spacing: 0) {
            sidebar
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    header
                    searchButton
                    if !trusted { permissionNotice }
                    HStack {
                        Text(category == .overview ? "QUICK ACCESS" : "\(category.rawValue.uppercased()) TOOLS")
                            .font(.system(size: 10, weight: .semibold)).tracking(1.5).foregroundStyle(.secondary)
                        Spacer()
                        Text("\(category.commands.count) tools").font(.caption).foregroundStyle(.secondary)
                    }
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 210), spacing: 12)], spacing: 12) {
                        ForEach(category.commands) { command in homeTool(command) }
                    }
                    HStack(alignment: .top, spacing: 10) {
                        Image(systemName: "cursorarrow.rays").foregroundStyle(BoxTheme.accent)
                        Text(settings.globalHotkeyEnabled
                             ? "Select text in another app, then press \(settings.globalHotkey.displayString) for tools that fit your selection."
                             : "Enable the command palette shortcut in Settings to open your tools from any app.")
                            .font(.system(size: 11)).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                    }.frame(maxWidth: .infinity, alignment: .leading).padding(14).surfaceCard()
                }.padding(28)
            }.id(category)
        }
        .background(WorkspaceBackground()).tint(BoxTheme.accent)
        .frame(minWidth: 900, minHeight: 640)
        .onReceive(timer) { _ in trusted = AccessibilityService.isTrusted }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 10) {
                ToolBadge(symbol: "shippingbox.fill", size: 34)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Bello Box").font(.system(size: 15, weight: .semibold))
                    Text("YOUR WORKSPACE").font(.system(size: 8, weight: .semibold)).tracking(1.4).foregroundStyle(.secondary)
                }
            }.padding(.vertical, 22).padding(.horizontal, 16)
            ForEach(Array(HomeCategory.allCases.enumerated()), id: \.element.id) { index, item in
                Button { category = item } label: {
                    HStack(spacing: 10) {
                        Image(systemName: item.symbol).frame(width: 20)
                        Text(item.rawValue).font(.system(size: 12, weight: category == item ? .semibold : .medium))
                        Spacer()
                        if category == item { Circle().fill(BoxTheme.accent).frame(width: 4, height: 4) }
                    }.foregroundStyle(category == item ? BoxTheme.accent : .primary)
                        .padding(.horizontal, 12).frame(height: 38)
                        .background(category == item ? BoxTheme.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 8))
                        .contentShape(Rectangle())
                }.buttonStyle(.plain).padding(.horizontal, 10)
                    .accessibilityValue(category == item ? "Selected" : "Not selected")
                    .accessibilityIdentifier("homeCategory_\(item.id)")
                    .keyboardShortcut(KeyEquivalent(Character(String(index + 1))), modifiers: .command)
            }
            Spacer(minLength: 20)
            VStack(alignment: .leading, spacing: 12) {
                Label(trusted ? "Selection tools ready" : "Selection access needed", systemImage: trusted ? "checkmark.shield" : "lock")
                    .foregroundStyle(trusted ? BoxTheme.teal : .secondary)
                Label(settings.isConfigured ? "AI connected" : "AI is optional", systemImage: "sparkles").foregroundStyle(.secondary)
            }.font(.system(size: 10)).padding(18)
            Divider().padding(.horizontal, 16)
            sidebarAction("Settings", symbol: "gearshape", action: onOpenSettings)
            sidebarAction("Setup guide", symbol: "questionmark.circle", action: onOpenGuide)
            if canCheckForUpdates { sidebarAction("Check for updates", symbol: "arrow.triangle.2.circlepath", action: onCheckForUpdates) }
            Text("Version \(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "")")
                .font(.system(size: 10)).foregroundStyle(.tertiary).padding(18)
        }.frame(width: 186).background(BoxTheme.surface.opacity(0.6))
    }
    private func sidebarAction(_ title: String, symbol: String, action: @escaping () -> Void) -> some View {
        Button(action: action) { Label(title, systemImage: symbol).font(.system(size: 11)).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 18).padding(.vertical, 7) }
            .buttonStyle(.plain)
    }
    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(category == .overview ? "Everything within reach." : category.rawValue)
                .font(.system(size: 28, weight: .semibold)).tracking(-0.6)
            Text(category.subtitle).font(.system(size: 12)).foregroundStyle(.secondary)
        }
    }
    private var searchButton: some View {
        Button(action: onOpenLauncher) {
            HStack(spacing: 12) {
                Image(systemName: "magnifyingglass").font(.system(size: 16)).foregroundStyle(BoxTheme.accent)
                VStack(alignment: .leading, spacing: 3) {
                    Text("Search all tools and commands").font(.system(size: 13, weight: .medium))
                    Text("20 commands. One place to start.").font(.system(size: 10)).foregroundStyle(.secondary)
                }
                Spacer()
                ShortcutBadge(text: settings.globalHotkeyEnabled ? settings.globalHotkey.displayString : "Open")
            }.padding(16)
        }.buttonStyle(ToolCardButtonStyle()).accessibilityIdentifier("mainLauncherButton")
            .keyboardShortcut("k", modifiers: .command).help("Search all tools (⌘K)")
    }
    private var permissionNotice: some View {
        HStack(spacing: 12) {
            Image(systemName: "lock.open").foregroundStyle(BoxTheme.accent)
            VStack(alignment: .leading, spacing: 3) {
                Text("Connect to your selection").font(.system(size: 12, weight: .semibold))
                Text("Allow Accessibility to read and replace selected text. You can use the other tools now.")
                    .font(.system(size: 11)).foregroundStyle(.secondary)
            }
            Spacer()
            Button("Allow access") { AccessibilityService.requestPermissionPrompt(); AccessibilityService.openAccessibilitySettings() }
                .buttonStyle(SecondaryButtonStyle())
        }.padding(14).surfaceCard()
    }
    private func homeTool(_ command: LauncherCommand) -> some View {
        HomeToolCard(command: command) { open(command) }.accessibilityIdentifier(accessibilityID(command))
    }
    private func accessibilityID(_ command: LauncherCommand) -> String {
        switch command {
        case .screenshot: return "mainScreenshotButton"
        case .scrollCapture: return "mainScrollButton"
        case .recording: return "mainRecordingButton"
        case .worldClock: return "mainWorldClockButton"
        case .qr: return "mainQRButton"
        case .textTools: return "mainTextToolsButton"
        default: return "homeTool_\(command.id)"
        }
    }
    private func open(_ command: LauncherCommand) {
        switch command {
        case .screenshot: onCapture()
        case .scrollCapture: onScrollCapture()
        case .recording: onRecord()
        case .worldClock: onOpenWorldClock()
        case .qr: onOpenQR()
        case .textTools: onOpenTextTools()
        default: onOpenTool(command)
        }
    }
}

private struct HomeToolCard: View {
    let command: LauncherCommand
    let action: () -> Void
    @State private var hovered = false
    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    ToolBadge(symbol: command.symbol, size: 32)
                    Spacer()
                    Image(systemName: "arrow.up.right").font(.system(size: 10, weight: .medium))
                        .foregroundStyle(hovered ? BoxTheme.accent : Color.secondary.opacity(0.45))
                }
                VStack(alignment: .leading, spacing: 5) {
                    Text(command.title).font(.system(size: 12, weight: .semibold)).lineLimit(1)
                    Text(command.subtitle).font(.system(size: 10)).foregroundStyle(.secondary).lineLimit(2)
                        .frame(height: 28, alignment: .topLeading)
                }.frame(maxWidth: .infinity, alignment: .leading)
            }.padding(15)
        }.buttonStyle(ToolCardButtonStyle())
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(BoxTheme.accent.opacity(hovered ? 0.4 : 0)))
            .onHover { hovered = $0 }.help(command.subtitle)
            .accessibilityLabel(command.title).accessibilityHint(command.subtitle)
    }
}
