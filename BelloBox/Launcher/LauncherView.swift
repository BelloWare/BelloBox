import SwiftUI

struct LauncherView: View {
    @ObservedObject var model: LauncherModel
    var onSearchReady: (LauncherSearchTextField) -> Void
    var onCopilotFieldReady: (LauncherSearchTextField) -> Void = { _ in }
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 0) {
            search
            if model.context.hasText || model.contextMessage != nil { selectionContext }
            Divider().opacity(0.6)
            HStack {
                Text(model.query.isEmpty ? (model.suggestions.isEmpty ? "Your tools" : "Suggested for your selection") : "Results")
                    .font(.system(size: 11, weight: .medium)).foregroundStyle(BoxTheme.secondaryText)
                Spacer()
                Text("\(model.commands.count)").font(.system(size: 10, design: .monospaced)).foregroundStyle(.tertiary)
            }.padding(.horizontal, 18).frame(height: 25)
            commandList
            Divider().opacity(0.6)
            footer
        }
        .workspaceBackground(role: .popup).tint(BoxTheme.accent).accentColor(BoxTheme.accentFill)
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(.primary.opacity(0.12), lineWidth: 1))
    }

    private var search: some View {
        HStack(spacing: 13) {
            Image(systemName: "magnifyingglass").font(.system(size: 18, weight: .medium)).foregroundStyle(BoxTheme.secondaryText)
            LauncherSearchField(text: $model.query, onMove: model.move, onSubmit: model.openSelected,
                onEscape: model.onClose, onReady: onSearchReady).frame(height: 26)
            if !model.query.isEmpty {
                Button { model.query = "" } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(.tertiary) }
                    .buttonStyle(.plain).accessibilityLabel("Clear search")
            }
            keycap("esc")
        }.padding(.horizontal, 20).frame(height: 64)
    }

    private var selectionContext: some View {
        HStack(spacing: 10) {
            Image(systemName: model.context.exceedsLimit ? "text.badge.minus" : "text.alignleft")
                .font(.system(size: 12)).foregroundStyle(BoxTheme.secondaryText)
                .frame(width: 26, height: 26).background(.primary.opacity(0.05), in: RoundedRectangle(cornerRadius: 6))
            VStack(alignment: .leading, spacing: 3) {
                if let message = model.contextMessage {
                    Text(message).font(.system(size: 11)).foregroundStyle(BoxTheme.secondaryText)
                } else if model.context.exceedsLimit {
                    Text("Selection exceeds 500 KB").font(.system(size: 11, weight: .medium))
                    Text("Select a smaller passage. Tools will open with an empty input.")
                        .font(.system(size: 10)).foregroundStyle(BoxTheme.secondaryText)
                } else {
                    HStack(spacing: 5) {
                        Text(model.selection.appName ?? "Selected text")
                        Text("·").foregroundStyle(.tertiary)
                        Text("\(model.context.characterCount.formatted()) characters")
                    }.font(.system(size: 10, weight: .medium)).foregroundStyle(BoxTheme.secondaryText)
                    Text(model.context.preview).font(.system(size: 11)).lineLimit(1).truncationMode(.tail).foregroundStyle(BoxTheme.secondaryText)
                }
            }
            Spacer(minLength: 6)
            Button(action: model.clearSelection) { Image(systemName: "xmark").font(.system(size: 9, weight: .semibold)) }
                .buttonStyle(.plain).foregroundStyle(BoxTheme.secondaryText).help("Clear selected text").accessibilityLabel("Clear selected text")
        }.padding(.horizontal, 18).padding(.bottom, 8).frame(height: 48)
    }

    private var commandList: some View {
        ScrollViewReader { reader in
            ScrollView(.vertical, showsIndicators: false) {
                LazyVStack(spacing: 0) {
                    if model.commands.isEmpty {
                        VStack(spacing: 7) {
                            Text("No matching tools").font(.system(size: 13, weight: .medium))
                            Text("Try JSON, diff, capture, or time.").font(.system(size: 11)).foregroundStyle(BoxTheme.secondaryText)
                            Button("Clear search") { model.query = "" }.buttonStyle(ToolLinkButtonStyle()).font(.system(size: 11))
                        }.frame(maxWidth: .infinity).frame(height: 118)
                    }
                    ForEach(model.commands) { command in
                        let expanded = model.expandedCommand == command
                        LauncherCommandRow(command: command, selected: model.selectedID == command.id,
                            favorite: model.favorites.contains(command.id), bestMatch: model.bestMatch == command,
                            expanded: expanded, preview: expanded ? model.expandedPreview : nil,
                            session: expanded ? model.expandedSession : nil,
                            previewHeight: model.expandedPreviewHeight,
                            hostWindow: model.hostWindow,
                            onOpen: { model.open(command) },
                            onLaunch: { customize in model.open(command, customize: customize) },
                            onFavorite: { model.toggleFavorite(command) },
                            onOpenSettings: { model.open(.settings) },
                            onFocusSearch: model.onFocusSearch,
                            onCopilotFieldReady: onCopilotFieldReady)
                            .id(command.id)
                    }
                }.padding(.horizontal, 8).padding(.vertical, 6)
            }
            .onChange(of: model.selectedID) { id in
                if let id { withAnimation(reduceMotion ? nil : .easeOut(duration: 0.12)) { reader.scrollTo(id) } }
            }
        }
    }

    private var footer: some View {
        HStack(spacing: 8) {
            Image(systemName: "shippingbox").font(.system(size: 12)).foregroundStyle(BoxTheme.secondaryText)
            Text("Bello Box").font(.system(size: 10, weight: .medium)).foregroundStyle(BoxTheme.secondaryText)
            Divider().frame(height: 12).padding(.horizontal, 3)
            Button { model.useClipboard() } label: {
                Label("Use Clipboard", systemImage: "doc.on.clipboard").font(.system(size: 10))
            }.buttonStyle(.plain).foregroundStyle(BoxTheme.secondaryText).help("Use clipboard text as input")
            Spacer()
            if model.featuresClock, model.query.isEmpty {
                keycap("←"); keycap("→")
                Text("Time").font(.system(size: 10)).foregroundStyle(BoxTheme.secondaryText)
                    .help("← → move 15 minutes, ⌥ moves an hour, ⇧ moves a day")
                Divider().frame(height: 12).padding(.horizontal, 3)
            }
            keycap("↑"); keycap("↓")
            Text("Navigate").font(.system(size: 10)).foregroundStyle(BoxTheme.secondaryText)
            Divider().frame(height: 12).padding(.horizontal, 3)
            Text("Open").font(.system(size: 10, weight: .medium)).foregroundStyle(BoxTheme.secondaryText)
            keycap("↵")
        }.padding(.horizontal, 16).frame(height: 41)
            .background(ChromeSurface())
    }
    private func keycap(_ key: String) -> some View {
        Text(key).font(.system(size: 10, weight: .medium)).foregroundStyle(BoxTheme.secondaryText)
            .padding(.horizontal, 5).frame(height: 19)
            .background(.primary.opacity(0.045), in: RoundedRectangle(cornerRadius: 4))
            .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(.primary.opacity(0.06)))
            .accessibilityHidden(true)
    }
}

/// One command. The focused row is the expanded one: it shows the command's
/// interactive preview under its title, so arrowing through the list tries
/// each tool on the selection without opening anything.
private struct LauncherCommandRow: View {
    let command: LauncherCommand
    let selected: Bool
    let favorite: Bool
    /// The top interpretation of the selection; labelled, whether or not focused.
    let bestMatch: Bool
    let expanded: Bool
    let preview: LauncherPreview?
    let session: LauncherInteractivePreview?
    let previewHeight: CGFloat
    let hostWindow: () -> NSWindow?
    let onOpen: () -> Void
    let onLaunch: ((inout LauncherCommandContext) -> Void) -> Void
    let onFavorite: () -> Void
    let onOpenSettings: () -> Void
    let onFocusSearch: () -> Void
    let onCopilotFieldReady: (LauncherSearchTextField) -> Void
    @State private var hovered = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 0) {
          HStack(spacing: 8) {
            Button(action: onOpen) {
                HStack(spacing: 11) {
                    ToolBadge(symbol: command.symbol, size: 27)
                    Text(command.title).font(.system(size: 13, weight: selected ? .semibold : .medium)).lineLimit(1)
                    Spacer(minLength: 12)
                    Text(bestMatch ? "Best match" : category).font(.system(size: 10))
                        .foregroundStyle(bestMatch ? BoxTheme.accent : .secondary)
                }.contentShape(Rectangle())
            }.buttonStyle(.plain).help(command.subtitle).accessibilityIdentifier("command_\(command.id)")
                .accessibilityLabel(command.title).accessibilityHint(command.subtitle)
                .accessibilityValue((selected ? "Selected" : "Not selected") + (expanded ? ". " + accessibilityPreview : ""))
            Button(action: onFavorite) {
                Image(systemName: favorite ? "star.fill" : "star").font(.system(size: 10))
                    .foregroundStyle(favorite ? BoxTheme.secondaryText : BoxTheme.secondaryText.opacity(0.6))
                    .opacity(favorite || selected || hovered ? 1 : 0)
                    .frame(width: 24, height: 28).contentShape(Rectangle())
            }.buttonStyle(.plain).help(favorite ? "Remove favorite" : "Add favorite")
                .accessibilityLabel("\(favorite ? "Unfavorite" : "Favorite") \(command.title)")
          }.padding(.horizontal, 10).frame(height: 42)
          if expanded {
              // Interactive: the preview's controls own their input. Enter still opens.
              // A group, so the preview's own controls keep their labels and identifiers.
              LauncherInteractivePreviewView(command: command, session: session, preview: preview, height: previewHeight,
                  hostWindow: hostWindow, onOpen: onOpen, onLaunch: onLaunch, onOpenSettings: onOpenSettings,
                  onFocusSearch: onFocusSearch, onCopilotFieldReady: onCopilotFieldReady)
                  .accessibilityElement(children: .contain)
                  .accessibilityLabel("\(command.title) preview")
                  .accessibilityIdentifier("launcherPreview_\(command.id)")
          }
        }
        .background {
            if selected || hovered {
                ToolSurface(role: selected ? .card : .control)
                    .clipShape(RoundedRectangle(cornerRadius: expanded ? 12 : 8))
            }
        }
        .overlay(RoundedRectangle(cornerRadius: expanded ? 12 : 8).strokeBorder(expanded ? BoxTheme.accent.opacity(0.28) : .clear))
        .onHover { hovered = $0 }
        .animation(reduceMotion ? nil : .easeOut(duration: 0.12), value: selected)
        .animation(reduceMotion ? nil : .easeOut(duration: 0.12), value: hovered)
    }
    /// The session's current state; the original selection's summary only
    /// stands in while there is no session (an oversized selection's notice).
    private var accessibilityPreview: String {
        if let session { return session.accessibilitySummary }
        return preview?.accessibilitySummary ?? "Preparing preview"
    }
    private var category: String {
        if command.isDeveloperTool { return "Developer" }
        return [.settings, .home].contains(command) ? "Bello Box" : "Utility"
    }
}
