import AppKit
import SwiftUI

/// Shared color, surface, and control tokens for every Bello Box window.
///
/// The palette follows the app icon: a warm orange toolbox on cream. `brand` is
/// the icon's own orange and is decorative only (washes, badges, marker
/// fills). `accent` is the readable ink version of that orange, darkened in
/// light mode and lifted to peach in dark mode so small text still clears
/// 4.5:1 on every surface. `accentFill`/`accentDeep` sit behind white labels.
enum BoxTheme {
    static let accent = adaptive(light: (0.64, 0.27, 0.04), dark: (1, 0.64, 0.40))
    static let accentFill = Color(red: 0.74, green: 0.34, blue: 0.05)
    static let accentDeep = Color(red: 0.64, green: 0.28, blue: 0.05)
    static let accentSoft = accent.opacity(0.12)
    /// The icon's orange. Never use it for text; it fails small-text contrast.
    static let brand = adaptive(light: (0.89, 0.46, 0.15), dark: (0.95, 0.53, 0.22))
    static let brandSoft = brand.opacity(0.14)
    static let background = adaptive(light: (0.985, 0.972, 0.955), dark: (0.082, 0.074, 0.068))
    static let surface = adaptive(light: (1, 1, 1), dark: (0.13, 0.12, 0.11))
    static let well = adaptive(light: (0.965, 0.948, 0.925), dark: (0.095, 0.086, 0.078))
    static let border = adaptive(light: (0.87, 0.83, 0.78), dark: (0.29, 0.26, 0.23))
    static let success = adaptive(light: (0.08, 0.42, 0.27), dark: (0.40, 0.83, 0.63))
    // Warning stays golden so it never reads as the orange accent.
    static let warning = adaptive(light: (0.50, 0.36, 0), dark: (1, 0.80, 0.42))
    static let danger = adaptive(light: (0.70, 0.14, 0.26), dark: (1, 0.53, 0.59))
    static let teal = adaptive(light: (0.06, 0.40, 0.43), dark: (0.38, 0.81, 0.81))
    static let cyan = adaptive(light: (0.04, 0.39, 0.54), dark: (0.40, 0.79, 0.96))
    static let purple = adaptive(light: (0.46, 0.26, 0.70), dark: (0.77, 0.63, 1))
    static let pink = adaptive(light: (0.65, 0.20, 0.43), dark: (1, 0.57, 0.75))
    static var accentGradient: LinearGradient {
        LinearGradient(colors: [accentFill, accentDeep], startPoint: .topLeading, endPoint: .bottomTrailing)
    }
    private static func adaptive(light: (Double, Double, Double), dark: (Double, Double, Double)) -> Color {
        Color(nsColor: NSColor(name: nil) { appearance in
            let c = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? dark : light
            return NSColor(srgbRed: c.0, green: c.1, blue: c.2, alpha: 1)
        })
    }
    static func tint(for symbol: String) -> Color {
        if ["record.circle", "record.circle.fill", "video"].contains(symbol) { return pink }
        if symbol.contains("clock") || symbol.contains("globe") || symbol.contains("calendar") { return teal }
        if symbol.contains("camera") || symbol == "arrow.down.doc" || symbol == "qrcode" { return cyan }
        if symbol.contains("wand") || symbol.contains("sparkle") || symbol == "key.horizontal" { return purple }
        return accent
    }
}

struct WorkspaceBackground: View {
    var body: some View {
        BoxTheme.background.overlay(alignment: .topLeading) {
            LinearGradient(colors: [BoxTheme.brand.opacity(0.09), .clear], startPoint: .top, endPoint: .bottom)
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
            .foregroundStyle(configuration.role == .destructive ? BoxTheme.danger : .primary).padding(.vertical, 8).padding(.horizontal, 12)
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

/// Small, literal swatches make the appearance choice visible before applying it.
struct AppearanceChoice: View {
    let preference: AppearancePreference
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 0) {
                    if preference != .dark { preview(dark: false) }
                    if preference != .light { preview(dark: true) }
                }
                .frame(height: 62).clipShape(RoundedRectangle(cornerRadius: 6))
                .accessibilityHidden(true)
                HStack {
                    Label(preference.label, systemImage: preference.symbol)
                        .font(.system(size: 12, weight: .semibold))
                    Spacer(minLength: 2)
                    if isSelected { Image(systemName: "checkmark.circle.fill").foregroundStyle(BoxTheme.accent) }
                }
                Text(preference == .system ? "Follow macOS" : "Always \(preference.label.lowercased())")
                    .font(.system(size: 10)).foregroundStyle(.secondary)
            }
            .padding(10).frame(maxWidth: .infinity)
            .background(isSelected ? BoxTheme.accentSoft : BoxTheme.well, in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(isSelected ? BoxTheme.accent : BoxTheme.border))
            .contentShape(RoundedRectangle(cornerRadius: 10))
        }.buttonStyle(.plain)
            .accessibilityLabel("\(preference.label) theme")
            .accessibilityValue(isSelected ? "Selected" : "Not selected")
            .accessibilityIdentifier("appearance_\(preference.rawValue)")
    }

    private func preview(dark: Bool) -> some View {
        HStack(spacing: 5) {
            RoundedRectangle(cornerRadius: 3).fill(BoxTheme.brand.opacity(dark ? 0.7 : 0.35)).frame(width: 14)
            VStack(alignment: .leading, spacing: 5) {
                Capsule().fill(dark ? Color.white.opacity(0.65) : Color.black.opacity(0.45)).frame(width: 25, height: 3)
                RoundedRectangle(cornerRadius: 3).fill(dark ? Color.white.opacity(0.10) : Color.white)
                RoundedRectangle(cornerRadius: 3).fill(dark ? Color.white.opacity(0.10) : Color.white)
            }
        }
        .padding(8).frame(maxWidth: .infinity)
        .background(dark ? Color(red: 0.082, green: 0.074, blue: 0.068) : Color(red: 0.965, green: 0.948, blue: 0.925))
    }
}
