import AppKit
import SwiftUI

// MARK: - Read-only output

/// A native, scrollable, selectable text view for preview output. It holds the
/// complete text (never an excerpt), scrolls with the wheel like any
/// NSScrollView, and is never editable, so the palette keeps its navigation
/// keys while it is clicked (see `LauncherWindowController.handleKeyEvent`).
struct LauncherOutputText: NSViewRepresentable {
    var text: String
    var attributed: NSAttributedString? = nil
    var label: String
    var monospaced = true
    var fontSize: CGFloat = 11

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = false
        scroll.autohidesScrollers = true
        scroll.scrollerStyle = .overlay
        let view = LauncherOutputTextView(frame: .zero)
        view.isEditable = false
        view.isSelectable = true
        view.isRichText = true
        view.importsGraphics = false
        view.drawsBackground = false
        view.textContainerInset = NSSize(width: 4, height: 5)
        view.minSize = .zero
        view.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        view.isVerticallyResizable = true
        view.isHorizontallyResizable = false
        view.autoresizingMask = [.width]
        view.textContainer?.widthTracksTextView = true
        view.textContainer?.lineFragmentPadding = 2
        view.isAutomaticLinkDetectionEnabled = false
        view.isAutomaticDataDetectionEnabled = false
        view.setAccessibilityLabel(label)
        scroll.documentView = view
        apply(to: view)
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let view = scroll.documentView as? LauncherOutputTextView else { return }
        view.setAccessibilityLabel(label)
        apply(to: view)
    }

    private func apply(to view: LauncherOutputTextView) {
        let font: NSFont = monospaced ? .monospacedSystemFont(ofSize: fontSize, weight: .regular) : .systemFont(ofSize: fontSize)
        if let attributed {
            guard view.shownAttributed !== attributed else { return }
            view.shownAttributed = attributed
            view.textStorage?.setAttributedString(attributed)
            view.scroll(.zero)
            return
        }
        guard view.shownAttributed != nil || view.string != text || view.font != font else { return }
        view.shownAttributed = nil
        view.font = font
        view.textColor = NSColor(BoxTheme.primaryText)
        view.string = text
        view.font = font
        view.scroll(.zero)
    }
}

final class LauncherOutputTextView: NSTextView {
    /// The attributed content currently shown, so updates skip identical input.
    var shownAttributed: NSAttributedString?
    override var needsPanelToBecomeKey: Bool { true }
}

// MARK: - Single-line native field

/// A bordered, literal single-line input for preview controls: a native
/// NSTextField that never claims focus when it appears. Up/Down stay with the
/// field, Enter calls `onSubmit`, and Escape returns to the palette search.
struct LauncherPreviewField: View {
    @Binding var text: String
    var placeholder: String
    var label: String
    var monospaced = true
    var onSubmit: () -> Void = {}
    var onEscape: () -> Void

    var body: some View {
        LauncherSearchField(text: $text, onMove: { _ in }, onSubmit: onSubmit, onEscape: onEscape, onReady: { _ in },
                            placeholder: placeholder, accessibilityID: "launcherPreviewField_" + label,
                            accessibilityLabel: label, fontSize: 11, focusesWhenAttached: false, monospaced: monospaced,
                            consumesVerticalArrows: false)
            .frame(height: 18)
            .padding(.horizontal, 7).padding(.vertical, 2)
            .toolSurface(.input, cornerRadius: 6)
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(BoxTheme.separator))
    }
}

/// A short literal multi-line editor for preview drafts (QR text, the second
/// text of a comparison). Native undo and input methods keep working.
struct LauncherPreviewEditor: View {
    @Binding var text: String
    var label: String
    var monospaced = true
    /// nil fills the space the preview leaves for it.
    var height: CGFloat? = nil

    var body: some View {
        LiteralTextEditor(text: $text, label: label, monospaced: monospaced, focusesWhenAttached: false, fontSize: 11)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .frame(height: height)
            .padding(.horizontal, 4).padding(.vertical, 1)
            .toolSurface(.input, cornerRadius: 7)
            .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(BoxTheme.separator))
            .accessibilityLabel(label)
    }
}

// MARK: - Compact controls

/// Small secondary chip for preview actions (Copy, Paste, Regenerate…).
struct LauncherChipButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled
    var prominent = false
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.system(size: 10, weight: .medium)).lineLimit(1)
            .foregroundStyle(prominent ? Color.white : Color.primary)
            .padding(.horizontal, 8).frame(height: 22)
            .background {
                ZStack {
                    if prominent { BoxTheme.accentGradient }
                    else {
                        ToolSurface(role: .control)
                        if configuration.isPressed { BoxTheme.accentSoft }
                    }
                }.clipShape(RoundedRectangle(cornerRadius: 6))
            }
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(prominent ? Color.white.opacity(0.12) : BoxTheme.separator))
            .opacity(isEnabled ? (configuration.isPressed && prominent ? 0.85 : 1) : 0.4)
            .contentShape(RoundedRectangle(cornerRadius: 6))
    }
}

/// A row of choices (mode, format, style) that fits a preview: the palette's
/// own segmented look, drawn with buttons so it never claims keyboard focus.
struct LauncherChoiceBar<Value: Hashable>: View {
    @Binding var selection: Value
    let choices: [(Value, String)]
    var label: String

    var body: some View {
        ToolChoiceBar(selection: $selection, choices: choices, label: label, compact: true, identifierPrefix: "launcherChoice")
            .fixedSize(horizontal: true, vertical: false)
    }

}

/// A compact popover menu for option lists too long for a choice bar.
struct LauncherOptionMenu<Value: Hashable>: View {
    @Binding var selection: Value
    let choices: [(Value, String)]
    var label: String
    var symbol: String? = nil

    var body: some View {
        Menu {
            ForEach(choices, id: \.0) { value, title in
                Button { selection = value } label: {
                    if value == selection { Label(title, systemImage: "checkmark") } else { Text(title) }
                }
            }
        } label: {
            HStack(spacing: 4) {
                if let symbol { Image(systemName: symbol) }
                Text(choices.first { $0.0 == selection }?.1 ?? label)
            }.font(.system(size: 10, weight: .medium)).lineLimit(1)
        }
        .menuStyle(.borderlessButton).fixedSize()
        .accessibilityLabel(label)
        .accessibilityIdentifier("launcherOption_\(label)")
    }
}

/// The compact card every preview output sits in.
struct LauncherOutputWell<Content: View>: View {
    /// nil fills the space the preview leaves for it.
    var height: CGFloat? = nil
    @ViewBuilder var content: Content
    var body: some View {
        content
            // Ideal height zero: the well takes whatever the row leaves, and
            // never asks for more when the content is measured on its own.
            .frame(maxWidth: .infinity, minHeight: 0, idealHeight: height ?? 0, maxHeight: .infinity)
            .frame(height: height)
            .toolSurface(.output, cornerRadius: 8)
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(BoxTheme.separator))
    }
}

/// One line at the top of an expanded row: what the tool recognized and a
/// short status, in the same voice as the World Clock header.
struct LauncherPreviewHeader<Trailing: View>: View {
    let title: String
    var subtitle: String? = nil
    var warning = false
    @ViewBuilder var trailing: Trailing

    init(title: String, subtitle: String? = nil, warning: Bool = false, @ViewBuilder trailing: () -> Trailing = { EmptyView() }) {
        self.title = title; self.subtitle = subtitle; self.warning = warning; self.trailing = trailing()
    }

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: warning ? "exclamationmark.circle" : "bolt.fill")
                .foregroundStyle(warning ? BoxTheme.warning : BoxTheme.accent)
            Text(title).fontWeight(.medium).lineLimit(1)
            if let subtitle, !subtitle.isEmpty {
                Text("·").foregroundStyle(.tertiary)
                Text(subtitle).foregroundStyle(BoxTheme.secondaryText).lineLimit(1).truncationMode(.tail)
            }
            Spacer(minLength: 4)
            trailing
        }.font(.system(size: 10)).lineLimit(1)
    }
}

/// A status line for a workbench-backed preview: an error, a message, or the
/// engine's status text, whichever is most relevant right now.
struct LauncherStatusLine: View {
    let text: String
    var warning = false
    var body: some View {
        HStack(spacing: 5) {
            if warning { Image(systemName: "exclamationmark.triangle").foregroundStyle(BoxTheme.warning) }
            Text(text).foregroundStyle(warning ? BoxTheme.warning : .secondary).lineLimit(2).truncationMode(.tail)
                .fixedSize(horizontal: false, vertical: true)
        }.font(.system(size: 10))
    }
}

/// Shown instead of editors and results when the current draft has grown past
/// what a row previews. The complete draft stays with the full tool; nothing
/// here is a truncation of it.
struct LauncherDraftLimitNotice: View {
    let byteCount: Int
    let detail: String
    let openTitle: String
    var onOpen: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            LauncherPreviewHeader(title: "Draft ready · \(Self.kilobytes(byteCount))",
                                  subtitle: "Larger than the row previews (64 KB)") {
                Button(openTitle, action: onOpen).buttonStyle(LauncherChipButtonStyle(prominent: true))
                    .help("Open the full tool with the complete draft")
                    .accessibilityIdentifier("launcherPreviewOpenFull")
            }
            Text(detail).font(.system(size: 12)).foregroundStyle(BoxTheme.secondaryText).lineLimit(4)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .padding(12).toolSurface(.card, cornerRadius: 9)
        }
    }
    static func kilobytes(_ bytes: Int) -> String { "\((Double(bytes) / 1_000).formatted(.number.precision(.fractionLength(0)))) KB" }
}

extension View {
    /// The small caption label beside preview controls.
    func previewCaption() -> some View { font(.system(size: 10)).foregroundStyle(BoxTheme.secondaryText).lineLimit(1) }
}
