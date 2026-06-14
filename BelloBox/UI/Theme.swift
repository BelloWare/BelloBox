import SwiftUI

/// Shared visual language for the overlay surfaces.
extension BoxTheme {
    static var accentGradient: LinearGradient {
        LinearGradient(colors: [accent, accentDeep], startPoint: .topLeading, endPoint: .bottomTrailing)
    }
}

/// A filled, gradient primary action button.
struct PrimaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.callout.weight(.semibold))
            .foregroundStyle(.white)
            .padding(.vertical, 7)
            .padding(.horizontal, 14)
            .background(Capsule().fill(BoxTheme.accentGradient))
            .opacity(configuration.isPressed ? 0.85 : 1)
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
            .contentShape(Capsule())
    }
}

/// A soft, bordered secondary action button.
struct SecondaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.callout.weight(.medium))
            .foregroundStyle(.primary)
            .padding(.vertical, 7)
            .padding(.horizontal, 14)
            .background(Capsule().fill(.primary.opacity(configuration.isPressed ? 0.12 : 0.07)))
            .overlay(Capsule().strokeBorder(.primary.opacity(0.08), lineWidth: 1))
            .contentShape(Capsule())
    }
}

/// A consistent header for every popup: a gradient icon badge, title, optional
/// subtitle, and a round close button.
struct PopupHeader: View {
    let icon: String
    let title: String
    var subtitle: String? = nil
    var onMinimize: (() -> Void)? = nil
    var onClose: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: icon)
                .font(.system(size: 14, weight: .bold))
                .foregroundStyle(.white)
                .frame(width: 30, height: 30)
                .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(BoxTheme.accentGradient))
                .shadow(color: BoxTheme.accent.opacity(0.35), radius: 4, y: 2)
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.headline)
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle).font(.caption2).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                }
            }
            Spacer(minLength: 8)
            if let onMinimize {
                chromeButton(systemName: "minus", help: "Minify", action: onMinimize)
            }
            chromeButton(systemName: "xmark", help: "Close", action: onClose)
        }
    }

    private func chromeButton(systemName: String, help: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 10, weight: .bold))
                .foregroundStyle(.secondary)
                .frame(width: 22, height: 22)
                .background(Circle().fill(.primary.opacity(0.08)))
        }
        .buttonStyle(.plain)
        .help(help)
    }
}

/// Compact chrome used when a popup is minified. The panel itself remains
/// draggable by background, while these controls restore or close it.
struct MinimizedPopupBar: View {
    let icon: String
    let title: String
    var subtitle: String? = nil
    var onRestore: () -> Void
    var onClose: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: icon)
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(.white)
                .frame(width: 28, height: 28)
                .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(BoxTheme.accentGradient))

            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.callout.weight(.semibold)).lineLimit(1)
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle).font(.caption2).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                }
            }

            Spacer(minLength: 8)

            Button(action: onRestore) {
                Image(systemName: "arrow.up.left.and.arrow.down.right")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(.secondary)
                    .frame(width: 24, height: 24)
                    .background(Circle().fill(.primary.opacity(0.08)))
            }
            .buttonStyle(.plain)
            .help("Restore")

            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(.secondary)
                    .frame(width: 24, height: 24)
                    .background(Circle().fill(.primary.opacity(0.08)))
            }
            .buttonStyle(.plain)
            .help("Close")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .popupCard()
        .onExitCommand(perform: onClose)
    }
}

extension View {
    /// The frosted card chrome shared by all popups.
    func popupCard() -> some View {
        background(
            ZStack {
                RoundedRectangle(cornerRadius: 18, style: .continuous).fill(.regularMaterial)
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .fill(LinearGradient(
                        colors: [BoxTheme.accent.opacity(0.10), .clear],
                        startPoint: .topLeading,
                        endPoint: .center
                    ))
            }
        )
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(.white.opacity(0.10), lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .shadow(color: .black.opacity(0.22), radius: 18, y: 10)
    }

    /// A subtle scale + fade entrance, so popups feel smooth.
    func appearPop() -> some View { modifier(AppearAnimation()) }

    /// A tinted, rounded inset container for grouped content.
    func toolPanel() -> some View {
        padding(8)
            .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(.primary.opacity(0.05)))
    }
}

struct AppearAnimation: ViewModifier {
    @State private var shown = false
    func body(content: Content) -> some View {
        content
            .opacity(shown ? 1 : 0)
            .scaleEffect(shown ? 1 : 0.96, anchor: .top)
            .onAppear {
                withAnimation(.spring(response: 0.30, dampingFraction: 0.82)) { shown = true }
            }
    }
}
