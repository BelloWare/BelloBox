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
    static let accentSoft = accent.opacity(0.09)
    /// The icon's orange. Never use it for text; it fails small-text contrast.
    static let brand = adaptive(light: (0.89, 0.46, 0.15), dark: (0.95, 0.53, 0.22))
    static let brandSoft = brand.opacity(0.14)
    static let background = adaptive(light: (0.965, 0.960, 0.951), dark: (0.085, 0.083, 0.080))
    static let surface = adaptive(light: (1, 0.998, 0.994), dark: (0.145, 0.141, 0.136))
    static let well = adaptive(light: (0.947, 0.941, 0.931), dark: (0.112, 0.108, 0.103))
    static let border = adaptive(light: (0.865, 0.849, 0.824), dark: (0.280, 0.266, 0.248))
    static let separator = border.opacity(0.55)
    static let sidebarWidth: CGFloat = 200
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
    // Tool identity uses one brand family. Semantic colors remain reserved for
    // states such as recording, errors, warnings, and meeting quality.
    static func tint(for symbol: String) -> Color { accent }

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
            .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(BoxTheme.separator))
    }
}

struct PrimaryButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.system(size: 12, weight: .semibold))
            .foregroundStyle(.white).padding(.horizontal, 13).frame(minHeight: 28)
            .background(BoxTheme.accentGradient, in: RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(.white.opacity(0.12)))
            .opacity(isEnabled ? (configuration.isPressed ? 0.8 : 1) : 0.4)
            .contentShape(RoundedRectangle(cornerRadius: 8))
            .modifier(ControlHoverFeedback(radius: 8))
    }
}

struct SecondaryButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.system(size: 12, weight: .medium))
            .foregroundStyle(configuration.role == .destructive ? BoxTheme.danger : .primary).padding(.horizontal, 11).frame(minHeight: 28)
            .background(configuration.isPressed ? BoxTheme.well : BoxTheme.surface, in: RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(BoxTheme.separator))
            .opacity(isEnabled ? 1 : 0.4)
            .contentShape(RoundedRectangle(cornerRadius: 8))
            .modifier(ControlHoverFeedback(radius: 8))
    }
}

struct ToolCardButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(configuration.isPressed ? BoxTheme.accentSoft : BoxTheme.surface, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(BoxTheme.separator))
            .shadow(color: .black.opacity(0.035), radius: 3, y: 1)
            .opacity(isEnabled ? 1 : 0.4)
            .contentShape(RoundedRectangle(cornerRadius: 12))
            .modifier(ControlHoverFeedback(radius: 12))
    }
}

struct PopupHeader: View {
    let icon: String
    let title: String
    var subtitle: String? = nil
    var onMinimize: (() -> Void)? = nil
    var onClose: () -> Void
    var body: some View {
        HStack(spacing: 9) {
            ToolBadge(symbol: icon, size: 32)
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.system(size: 16, weight: .semibold)).lineLimit(1)
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                }
            }
            Spacer(minLength: 8)
            if let onMinimize { PopupChromeButton(symbol: "minus", label: "Minimize popup", action: onMinimize) }
            PopupChromeButton(symbol: "xmark", label: "Close (Esc)", action: onClose)
        }
        .padding(.bottom, 10)
        .overlay(alignment: .bottom) { Rectangle().fill(BoxTheme.separator).frame(height: 1) }
    }
}

/// Matching hit targets and hover feedback in full and minimized tool headers.
struct PopupChromeButton: View {
    let symbol: String
    let label: String
    var action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 10, weight: .semibold))
                .foregroundStyle(hovered ? BoxTheme.accent : Color.secondary).frame(width: 28, height: 28)
                .background(hovered ? BoxTheme.accentSoft : BoxTheme.well, in: RoundedRectangle(cornerRadius: 7))
                .contentShape(RoundedRectangle(cornerRadius: 7))
        }.buttonStyle(.plain).help(label).accessibilityLabel(label)
            .onHover { hovered = $0 }
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
            PopupChromeButton(symbol: "arrow.up.left.and.arrow.down.right", label: "Restore", action: onRestore)
            PopupChromeButton(symbol: "xmark", label: "Close", action: onClose)
        }.padding(10).popupCard().onExitCommand(perform: onClose)
    }
}

extension View {
    func popupCard() -> some View {
        buttonStyle(SecondaryButtonStyle()).workspaceBackground(role: .popup)
            .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(BoxTheme.separator))
            .tint(BoxTheme.accent).accentColor(BoxTheme.accentFill)
    }
    func surfaceCard() -> some View {
        background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 12))
            .shadow(color: .black.opacity(0.025), radius: 3, y: 1)
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(BoxTheme.separator))
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

/// Native link buttons ignore SwiftUI tint on some macOS versions.
/// Use the same accessible brand ink as the surrounding controls.
struct ToolLinkButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.foregroundStyle(BoxTheme.accent)
            .opacity(!isEnabled ? 0.4 : configuration.isPressed ? 0.65 : 1)
            .contentShape(Rectangle())
    }
}

/// A shared literal-looking field surface for settings and full tool controls.
/// The native editor still handles focus, selection, keyboard input and undo.
struct ToolTextFieldStyle: TextFieldStyle {
    func _body(configuration: TextField<Self._Label>) -> some View {
        configuration.textFieldStyle(.plain).font(.system(size: 13))
            .padding(.horizontal, 10).padding(.vertical, 7)
            .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(BoxTheme.separator))
    }
}

/// The same segmented control in compact previews and full-size tools.
struct ToolChoiceBar<Value: Hashable>: View {
    @Binding var selection: Value
    let choices: [(Value, String)]
    var label: String
    var compact = false
    var identifierPrefix = "toolChoice"
    var numberedShortcuts = false
    @Environment(\.isEnabled) private var isEnabled

    var body: some View {
        HStack(spacing: 2) {
            ForEach(Array(choices.enumerated()), id: \.element.0) { index, choice in
                let (value, title) = choice
                let selected = selection == value
                Button { selection = value } label: {
                    Text(title).font(.system(size: compact ? 10 : 12, weight: selected ? .semibold : .medium)).lineLimit(1)
                        .foregroundStyle(selected ? BoxTheme.accent : Color.secondary)
                        .padding(.horizontal, compact ? 8 : 10).frame(height: compact ? 20 : 26)
                        .frame(maxWidth: compact ? nil : .infinity)
                        .background(selected ? AnyShapeStyle(BoxTheme.surface) : AnyShapeStyle(Color.clear), in: RoundedRectangle(cornerRadius: 5))
                        .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(selected ? BoxTheme.separator : .clear))
                        .shadow(color: .black.opacity(selected ? 0.05 : 0), radius: 2, y: 1)
                        .contentShape(RoundedRectangle(cornerRadius: 5))
                }.buttonStyle(.plain)
                    .modifier(ToolNumberShortcut(index: index, enabled: numberedShortcuts))
                    .accessibilityLabel("\(label): \(title)")
                    .accessibilityValue(selected ? "Selected" : "Not selected")
                    .accessibilityIdentifier("\(identifierPrefix)_\(label)_\(title)")
            }
        }.padding(2)
            .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(BoxTheme.separator))
            .opacity(isEnabled ? 1 : 0.45)
            .accessibilityElement(children: .contain)
    }
}

/// Square actions use consistent hit targets instead of text-button padding.
struct ToolIconButtonStyle: ButtonStyle {
    var selected = false
    var size: CGFloat = 28
    @Environment(\.isEnabled) private var isEnabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.system(size: size * 0.43, weight: .medium))
            .foregroundStyle(selected ? BoxTheme.accent : Color.primary)
            .frame(width: size, height: size)
            .background(selected || configuration.isPressed ? BoxTheme.accentSoft : Color.clear, in: RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(selected ? BoxTheme.accent.opacity(0.5) : Color.clear))
            .opacity(isEnabled ? 1 : 0.35)
            .contentShape(RoundedRectangle(cornerRadius: 6))
            .modifier(ControlHoverFeedback(radius: 6))
    }
}

private struct ToolNumberShortcut: ViewModifier {
    let index: Int
    let enabled: Bool
    @ViewBuilder func body(content: Content) -> some View {
        if enabled, index < 9 {
            content.keyboardShortcut(KeyEquivalent(Character(String(index + 1))), modifiers: .command)
        } else { content }
    }
}

private struct ControlHoverFeedback: ViewModifier {
    let radius: CGFloat
    @State private var hovered = false
    @Environment(\.isEnabled) private var isEnabled
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    func body(content: Content) -> some View {
        content.overlay(RoundedRectangle(cornerRadius: radius)
            .strokeBorder(BoxTheme.accent.opacity(hovered && isEnabled ? 0.45 : 0))
            .allowsHitTesting(false))
            .onHover { hovered = $0 }
            .animation(reduceMotion ? nil : .easeOut(duration: 0.1), value: hovered)
    }
}

/// Shared section hierarchy without a second card around every label.
struct ToolSectionHeading: View {
    let title: String
    var detail: String? = nil
    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(title).font(.system(size: 12, weight: .semibold))
            Spacer(minLength: 8)
            if let detail { Text(detail).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1) }
        }
    }
}

/// A common sidebar target in Home and Settings, with a clear current page.
struct SidebarItemStyle: ButtonStyle {
    var selected = false
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(selected ? BoxTheme.accent : Color.primary)
            .padding(.horizontal, 12).frame(height: 40)
            .background(selected ? BoxTheme.accentSoft : configuration.isPressed ? BoxTheme.well : .clear,
                        in: RoundedRectangle(cornerRadius: 8))
            .overlay(alignment: .leading) {
                if selected { Capsule().fill(BoxTheme.accent).frame(width: 3, height: 16) }
            }
            .contentShape(RoundedRectangle(cornerRadius: 8))
            .modifier(ControlHoverFeedback(radius: 8))
    }
}
