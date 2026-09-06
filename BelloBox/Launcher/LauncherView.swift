import SwiftUI

struct LauncherView: View {
    @ObservedObject var model: LauncherModel
    var onSearchReady: (LauncherSearchTextField) -> Void

    var body: some View {
        VStack(spacing: 0) {
            if let workbench = model.workbench {
                UtilityWorkbenchView(model: workbench, onBack: model.back)
            } else {
                search
                if model.context.hasText || model.contextMessage != nil { selectionContext }
                Divider().opacity(0.6)
                HStack {
                    Text(model.query.isEmpty ? (model.suggestions.isEmpty ? "Your tools" : "Suggested for your text") : "Results")
                        .font(.system(size: 11, weight: .medium)).foregroundStyle(.secondary)
                    Spacer()
                    Text("\(model.commands.count)").font(.system(size: 10, design: .monospaced)).foregroundStyle(.tertiary)
                }.padding(.horizontal, 18).frame(height: 25)
                commandList
                Divider().opacity(0.6)
                footer
            }
        }
        .background(WorkspaceBackground()).tint(BoxTheme.accent)
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(.primary.opacity(0.12), lineWidth: 1))
    }

    private var search: some View {
        HStack(spacing: 13) {
            Image(systemName: "magnifyingglass").font(.system(size: 18, weight: .medium)).foregroundStyle(.secondary)
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
                .font(.system(size: 12)).foregroundStyle(.secondary)
                .frame(width: 26, height: 26).background(.primary.opacity(0.05), in: RoundedRectangle(cornerRadius: 6))
            VStack(alignment: .leading, spacing: 3) {
                if let message = model.contextMessage {
                    Text(message).font(.system(size: 11)).foregroundStyle(.secondary)
                } else if model.context.exceedsLimit {
                    Text("Selection exceeds 500 KB").font(.system(size: 11, weight: .medium))
                    Text("Select a smaller passage. Tools will open with an empty input.")
                        .font(.system(size: 10)).foregroundStyle(.secondary)
                } else {
                    HStack(spacing: 5) {
                        Text(model.selection.appName ?? "Selected text")
                        Text("·").foregroundStyle(.tertiary)
                        Text("\(model.context.characterCount.formatted()) characters")
                    }.font(.system(size: 10, weight: .medium)).foregroundStyle(.secondary)
                    Text(model.context.preview).font(.system(size: 11)).lineLimit(1).truncationMode(.tail).foregroundStyle(.secondary)
                }
            }
            Spacer(minLength: 6)
            Button(action: model.clearSelection) { Image(systemName: "xmark").font(.system(size: 9, weight: .semibold)) }
                .buttonStyle(.plain).foregroundStyle(.secondary).help("Clear selected text").accessibilityLabel("Clear selected text")
        }.padding(.horizontal, 18).padding(.bottom, 8).frame(height: 48)
    }

    private var commandList: some View {
        ScrollViewReader { reader in
            ScrollView(.vertical, showsIndicators: false) {
                LazyVStack(spacing: 0) {
                    if model.commands.isEmpty {
                        VStack(spacing: 7) {
                            Text("No matching tools").font(.system(size: 13, weight: .medium))
                            Text("Try JSON, diff, capture, or time.").font(.system(size: 11)).foregroundStyle(.secondary)
                            Button("Clear search") { model.query = "" }.buttonStyle(.link).font(.system(size: 11))
                        }.frame(maxWidth: .infinity).frame(height: 118)
                    }
                    ForEach(model.commands) { command in
                        LauncherCommandRow(command: command, selected: model.selectedID == command.id,
                            favorite: model.favorites.contains(command.id), onOpen: { model.open(command) },
                            onFavorite: { model.toggleFavorite(command) })
                            .id(command.id)
                    }
                }.padding(.horizontal, 8).padding(.vertical, 6)
            }
            .onChange(of: model.selectedID) { id in
                if let id { reader.scrollTo(id) }
            }
        }
    }

    private var footer: some View {
        HStack(spacing: 8) {
            Image(systemName: "shippingbox").font(.system(size: 12)).foregroundStyle(.secondary)
            Text("Bello Box").font(.system(size: 10, weight: .medium)).foregroundStyle(.secondary)
            Divider().frame(height: 12).padding(.horizontal, 3)
            Button { model.useClipboard() } label: {
                Label("Use Clipboard", systemImage: "doc.on.clipboard").font(.system(size: 10))
            }.buttonStyle(.plain).foregroundStyle(.secondary).help("Use clipboard text as input")
            Spacer()
            keycap("↑"); keycap("↓")
            Text("Navigate").font(.system(size: 10)).foregroundStyle(.secondary)
            Divider().frame(height: 12).padding(.horizontal, 3)
            Text("Open").font(.system(size: 10, weight: .medium)).foregroundStyle(.secondary)
            keycap("↵")
        }.padding(.horizontal, 16).frame(height: 41)
            .background(BoxTheme.surface.opacity(0.45))
    }
    private func keycap(_ key: String) -> some View {
        Text(key).font(.system(size: 10, weight: .medium)).foregroundStyle(.secondary)
            .padding(.horizontal, 5).frame(height: 19)
            .background(.primary.opacity(0.045), in: RoundedRectangle(cornerRadius: 4))
            .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(.primary.opacity(0.06)))
            .accessibilityHidden(true)
    }
}

private struct LauncherCommandRow: View {
    let command: LauncherCommand
    let selected: Bool
    let favorite: Bool
    let onOpen: () -> Void
    let onFavorite: () -> Void
    @State private var hovered = false

    var body: some View {
        HStack(spacing: 8) {
            Button(action: onOpen) {
                HStack(spacing: 11) {
                    ToolBadge(symbol: command.symbol, size: 27)
                    Text(command.title).font(.system(size: 13, weight: selected ? .semibold : .medium)).lineLimit(1)
                    Spacer(minLength: 12)
                    Text(category).font(.system(size: 10)).foregroundStyle(.tertiary)
                }.contentShape(Rectangle())
            }.buttonStyle(.plain).help(command.subtitle).accessibilityIdentifier("command_\(command.id)")
                .accessibilityLabel(command.title).accessibilityHint(command.subtitle)
                .accessibilityValue(selected ? "Selected" : "Not selected")
            Button(action: onFavorite) {
                Image(systemName: favorite ? "star.fill" : "star").font(.system(size: 10))
                    .foregroundStyle(favorite ? Color.secondary : Color.secondary.opacity(0.6))
                    .opacity(favorite || selected || hovered ? 1 : 0)
                    .frame(width: 24, height: 28).contentShape(Rectangle())
            }.buttonStyle(.plain).help(favorite ? "Remove favorite" : "Add favorite")
                .accessibilityLabel("\(favorite ? "Unfavorite" : "Favorite") \(command.title)")
        }
        .padding(.horizontal, 10).frame(height: 42)
        .background((selected ? BoxTheme.accentSoft : hovered ? Color.primary.opacity(0.035) : .clear),
                    in: RoundedRectangle(cornerRadius: 8))
        .onHover { hovered = $0 }
    }
    private var category: String {
        if command.isDeveloperTool { return "Developer" }
        return [.settings, .home].contains(command) ? "Bello Box" : "Utility"
    }
}
