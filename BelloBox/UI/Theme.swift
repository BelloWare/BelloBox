import AppKit
import SwiftUI

/// Shared color, surface, and control tokens for every Bello Box window.
enum BoxTheme {
    static let accent = Color(red: 0.38, green: 0.43, blue: 0.96)
    static let accentDeep = Color(red: 0.26, green: 0.32, blue: 0.82)
    static let accentSoft = accent.opacity(0.12)
    static let background = adaptive(light: (0.96, 0.97, 0.99), dark: (0.065, 0.075, 0.105))
    static let surface = adaptive(light: (1, 1, 1), dark: (0.105, 0.12, 0.16))
    static let well = adaptive(light: (0.94, 0.95, 0.975), dark: (0.075, 0.085, 0.12))
    static let border = Color.primary.opacity(0.085)
    static var accentGradient: LinearGradient {
        LinearGradient(colors: [accent, accentDeep], startPoint: .topLeading, endPoint: .bottomTrailing)
    }
    private static func adaptive(light: (Double, Double, Double), dark: (Double, Double, Double)) -> Color {
        Color(nsColor: NSColor(name: nil) { appearance in
            let c = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? dark : light
            return NSColor(srgbRed: c.0, green: c.1, blue: c.2, alpha: 1)
        })
    }
    static func tint(for symbol: String) -> Color {
        if ["record.circle", "record.circle.fill", "video"].contains(symbol) { return .pink }
        if symbol.contains("clock") || symbol.contains("globe") || symbol.contains("calendar") { return .teal }
        if symbol.contains("camera") || symbol == "arrow.down.doc" || symbol == "qrcode" { return .cyan }
        if symbol.contains("wand") || symbol.contains("sparkle") || symbol == "key.horizontal" { return .purple }
        return accent
    }
}

struct WorkspaceBackground: View {
    var body: some View {
        BoxTheme.background.overlay(alignment: .topLeading) {
            LinearGradient(colors: [BoxTheme.accent.opacity(0.065), .clear], startPoint: .top, endPoint: .bottom)
                .frame(height: 240).allowsHitTesting(false)
        }
    }
}

struct ToolBadge: View {
    let symbol: String
    var size: CGFloat = 34
    var body: some View {
        Image(systemName: symbol)
            .font(.system(size: size * 0.43, weight: .semibold))
            .foregroundStyle(BoxTheme.tint(for: symbol))
            .frame(width: size, height: size)
            .background(BoxTheme.tint(for: symbol).opacity(0.11), in: RoundedRectangle(cornerRadius: size * 0.28))
            .overlay(RoundedRectangle(cornerRadius: size * 0.28).strokeBorder(BoxTheme.tint(for: symbol).opacity(0.1)))
            .accessibilityHidden(true)
    }
}

struct ShortcutBadge: View {
    let text: String
    var body: some View {
        Text(text).font(.system(size: 11, weight: .medium, design: .monospaced))
            .foregroundStyle(.secondary).padding(.horizontal, 7).padding(.vertical, 4)
            .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 5))
            .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(BoxTheme.border))
    }
}

struct PrimaryButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.system(size: 12, weight: .semibold))
            .foregroundStyle(.white).padding(.vertical, 8).padding(.horizontal, 13)
            .background(BoxTheme.accentGradient, in: RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(.white.opacity(0.12)))
            .opacity(isEnabled ? (configuration.isPressed ? 0.8 : 1) : 0.4)
            .contentShape(RoundedRectangle(cornerRadius: 8))
    }
}

struct SecondaryButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.system(size: 12, weight: .medium))
            .foregroundStyle(.primary).padding(.vertical, 8).padding(.horizontal, 12)
            .background(configuration.isPressed ? BoxTheme.well : BoxTheme.surface, in: RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(BoxTheme.border))
            .opacity(isEnabled ? 1 : 0.4)
            .contentShape(RoundedRectangle(cornerRadius: 8))
    }
}

struct ToolCardButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(configuration.isPressed ? BoxTheme.accentSoft : BoxTheme.surface, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(BoxTheme.border))
            .opacity(isEnabled ? 1 : 0.4)
            .contentShape(RoundedRectangle(cornerRadius: 12))
    }
}

struct PopupHeader: View {
    let icon: String
    let title: String
    var subtitle: String? = nil
    var onMinimize: (() -> Void)? = nil
    var onClose: () -> Void
    var body: some View {
        HStack(spacing: 11) {
            ToolBadge(symbol: icon)
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.system(size: 15, weight: .semibold))
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                }
            }
            Spacer(minLength: 8)
            if let onMinimize { chromeButton("minus", help: "Minimize popup", action: onMinimize) }
            chromeButton("xmark", help: "Close (Esc)", action: onClose)
        }
        .padding(.bottom, 8)
        .overlay(alignment: .bottom) { Rectangle().fill(BoxTheme.border).frame(height: 1) }
    }
    private func chromeButton(_ symbol: String, help: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 10, weight: .semibold))
                .foregroundStyle(.secondary).frame(width: 26, height: 26)
                .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 7))
        }.buttonStyle(.plain).help(help).accessibilityLabel(help)
    }
}

struct MinimizedPopupBar: View {
    let icon: String
    let title: String
    var subtitle: String? = nil
    var onRestore: () -> Void
    var onClose: () -> Void
    var body: some View {
        HStack(spacing: 10) {
            ToolBadge(symbol: icon, size: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.callout.weight(.semibold)).lineLimit(1)
                if let subtitle, !subtitle.isEmpty { Text(subtitle).font(.caption2).foregroundStyle(.secondary).lineLimit(1) }
            }
            Spacer(minLength: 8)
            Button(action: onRestore) { Image(systemName: "arrow.up.left.and.arrow.down.right") }
                .buttonStyle(SecondaryButtonStyle()).help("Restore").accessibilityLabel("Restore")
            Button(action: onClose) { Image(systemName: "xmark") }
                .buttonStyle(SecondaryButtonStyle()).help("Close").accessibilityLabel("Close")
        }.padding(10).popupCard().onExitCommand(perform: onClose)
    }
}

extension View {
    func popupCard() -> some View {
        background(WorkspaceBackground())
            .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(BoxTheme.border))
            .tint(BoxTheme.accent)
    }
    func surfaceCard() -> some View {
        background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(BoxTheme.border))
    }
    func toolPanel() -> some View { padding(10).surfaceCard() }
    func appearPop() -> some View { modifier(AppearAnimation()) }
}

struct AppearAnimation: ViewModifier {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false
    func body(content: Content) -> some View {
        content.opacity(shown ? 1 : 0)
            .onAppear {
                withAnimation(reduceMotion ? nil : .easeOut(duration: 0.14)) { shown = true }
            }
    }
}
