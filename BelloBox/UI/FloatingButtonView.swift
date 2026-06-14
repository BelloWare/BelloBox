import SwiftUI

/// Visual palette shared by the overlay surfaces, echoing the Bello Box icon.
enum BoxTheme {
    static let accent = Color(red: 0.84, green: 0.46, blue: 0.12)
    static let accentDeep = Color(red: 0.72, green: 0.36, blue: 0.07)
    static let accentSoft = Color(red: 0.84, green: 0.46, blue: 0.12).opacity(0.12)
}

/// The floating toolbar that appears next to a fresh text selection. It offers
/// the available tools (AI actions and QR code) without stealing focus.
struct FloatingToolbarView: View {
    static let preferredSize = CGSize(width: 156, height: 52)

    var onAI: () -> Void
    var onQR: () -> Void
    var onTools: () -> Void

    var body: some View {
        HStack(spacing: 4) {
            ToolIcon(symbol: "wand.and.stars", help: "Ask Bello Box AI about the selection", action: onAI)
            divider
            ToolIcon(symbol: "qrcode", help: "Generate a QR code from the selection", action: onQR)
            divider
            ToolIcon(symbol: "wrench.and.screwdriver", help: "Text tools (case, encode, hash, count…)", action: onTools)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .background(
            Capsule().fill(
                LinearGradient(colors: [BoxTheme.accent, BoxTheme.accentDeep], startPoint: .top, endPoint: .bottom)
            )
        )
        .overlay(Capsule().strokeBorder(.white.opacity(0.22), lineWidth: 0.5))
        .shadow(color: .black.opacity(0.22), radius: 5, y: 2)
        .padding(5)
    }

    private var divider: some View {
        Rectangle().fill(.white.opacity(0.28)).frame(width: 1, height: 22)
    }
}

private struct ToolIcon: View {
    let symbol: String
    let help: String
    let action: () -> Void

    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 38, height: 32)
                .background(
                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                        .fill(.white.opacity(hovering ? 0.22 : 0))
                )
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(help)
        .onHover { hovering = $0 }
        .animation(.easeOut(duration: 0.1), value: hovering)
    }
}
