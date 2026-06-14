import SwiftUI

/// Visual palette shared by the overlay surfaces, echoing the BelloBox icon.
enum BoxTheme {
    static let accent = Color(red: 0.84, green: 0.46, blue: 0.12)
    static let accentDeep = Color(red: 0.72, green: 0.36, blue: 0.07)
    static let accentSoft = Color(red: 0.84, green: 0.46, blue: 0.12).opacity(0.12)
}

/// The small floating button that appears next to a fresh text selection.
struct FloatingButtonView: View {
    static let preferredSize = CGSize(width: 42, height: 34)

    var onActivate: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: onActivate) {
            Image(systemName: "wand.and.stars")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: Self.preferredSize.width, height: Self.preferredSize.height)
                .background(
                    RoundedRectangle(cornerRadius: 9, style: .continuous)
                        .fill(
                            LinearGradient(
                                colors: [BoxTheme.accent, BoxTheme.accentDeep],
                                startPoint: .top,
                                endPoint: .bottom
                            )
                        )
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 9, style: .continuous)
                        .strokeBorder(.white.opacity(0.25), lineWidth: 0.5)
                )
                .scaleEffect(hovering ? 1.06 : 1.0)
                .shadow(color: .black.opacity(0.22), radius: 5, y: 2)
        }
        .buttonStyle(.plain)
        .help("Ask BelloBox about the selected text")
        .onHover { hovering = $0 }
        .animation(.easeOut(duration: 0.12), value: hovering)
        .padding(4)
    }
}
