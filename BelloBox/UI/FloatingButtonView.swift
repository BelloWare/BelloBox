import SwiftUI

/// The floating toolbar that appears next to a fresh text selection. It offers
/// the available tools (AI actions and QR code) without stealing focus.
struct FloatingToolbarView: View {
    static let preferredSize = CGSize(width: 300, height: 52)
    static let timestampPreferredSize = CGSize(width: 330, height: 104)

    var onAI: () -> Void
    var onScreenshot: () -> Void
    var onRecord: () -> Void
    var onQR: () -> Void
    var onTools: () -> Void
    var onAllTools: () -> Void = {}
    var onOpenWorldClock: (Date) -> Void = { _ in }
    var onHoverHelp: (String?) -> Void = { _ in }
    var timestampSummary: TimestampSummary? = nil

    @State private var timestampHovering = false

    var body: some View {
        Group {
            if let timestampSummary {
                timestampToolbar(timestampSummary)
            } else {
                actionToolbar
            }
        }
    }

    private var actionToolbar: some View {
        actionButtons
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .popupCard()
            .padding(5)
    }

    private func timestampToolbar(_ summary: TimestampSummary) -> some View {
        VStack(spacing: 0) {
            Button {
                onOpenWorldClock(summary.date)
            } label: {
                HStack(spacing: 9) {
                    Image(systemName: "clock")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(.primary)
                        .frame(width: 28, height: 28)
                        .background(Circle().fill(BoxTheme.accentSoft))

                    VStack(alignment: .leading, spacing: 2) {
                        Text(summary.relativeTime)
                            .font(.system(size: 13, weight: .semibold))
                        Text(summary.localDateTime)
                            .font(.system(size: 11))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .minimumScaleFactor(0.8)
                    }
                    .foregroundStyle(.primary)
                    .frame(maxWidth: .infinity, alignment: .leading)

                    Image(systemName: "globe")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.secondary)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 7)
                .background(BoxTheme.accent.opacity(timestampHovering ? 0.10 : 0))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .onHover { hovering in
                timestampHovering = hovering
                onHoverHelp(hovering ? "Open this time in World Clock" : nil)
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Selected timestamp")
            .accessibilityValue("\(summary.relativeTime), \(summary.localDateTime). Open in World Clock.")

            Rectangle()
                .fill(BoxTheme.border)
                .frame(height: 1)

            actionButtons
                .padding(.horizontal, 8)
                .padding(.vertical, 5)
        }
        .frame(width: 320)
        .popupCard()
        .padding(5)
    }

    private var actionButtons: some View {
        HStack(spacing: 4) {
            ToolIcon(symbol: "wand.and.stars", help: "Ask Bello Box AI about the selection", onHoverHelp: onHoverHelp, action: onAI)
            divider
            ToolIcon(symbol: "camera.viewfinder", help: "Capture and annotate a screenshot", onHoverHelp: onHoverHelp, action: onScreenshot)
            divider
            ToolIcon(symbol: "record.circle", help: "Record screen video", onHoverHelp: onHoverHelp, action: onRecord)
            divider
            ToolIcon(symbol: "qrcode", help: "Generate a QR code from the selection", onHoverHelp: onHoverHelp, action: onQR)
            divider
            ToolIcon(symbol: "wrench.and.screwdriver", help: "Text tools (case, encode, hash, count…)", onHoverHelp: onHoverHelp, action: onTools)
            divider
            ToolIcon(symbol: "magnifyingglass", help: "Search all tools and suggested actions", onHoverHelp: onHoverHelp, action: onAllTools)
        }
    }

    private var divider: some View {
        Rectangle().fill(BoxTheme.border).frame(width: 1, height: 22)
    }
}

private struct ToolIcon: View {
    let symbol: String
    let help: String
    let onHoverHelp: (String?) -> Void
    let action: () -> Void

    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(.primary)
                .frame(width: 38, height: 32)
                .background(
                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                        .fill(BoxTheme.accent.opacity(hovering ? 0.16 : 0))
                )
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(help)
        .onHover { isHovering in
            hovering = isHovering
            onHoverHelp(isHovering ? help : nil)
        }
        .onDisappear { onHoverHelp(nil) }
        .animation(.easeOut(duration: 0.1), value: hovering)
    }
}
