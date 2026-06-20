import SwiftUI

struct RecordingHUDView: View {
    var runtime: RecordingRuntimeState
    var isPaused: Bool
    var onPauseResume: () -> Void
    var onStop: () -> Void

    @State private var now = Date()

    var body: some View {
        HStack(spacing: 12) {
            Circle()
                .fill(isPaused ? .orange : .red)
                .frame(width: 10, height: 10)
            Text(elapsedText)
                .font(.system(.body, design: .monospaced).weight(.semibold))
            statusIcon(enabled: runtime.isMicEnabled, symbol: "mic.fill", help: "Microphone")
            statusIcon(enabled: runtime.isSystemAudioEnabled, symbol: "speaker.wave.2.fill", help: "Mac Audio")
            statusIcon(enabled: runtime.isInputOverlayEnabled, symbol: "keyboard", help: "Input overlays")
            if runtime.isSecureFieldHidden {
                Label("Secure field hidden", systemImage: "lock.shield")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.orange)
            }
            Button(isPaused ? "Resume" : "Pause", action: onPauseResume)
                .buttonStyle(SecondaryButtonStyle())
                .controlSize(.small)
            Button("Stop", action: onStop)
                .buttonStyle(PrimaryButtonStyle())
                .controlSize(.small)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(Capsule().fill(.ultraThinMaterial))
        .overlay(Capsule().strokeBorder(Color.primary.opacity(0.10), lineWidth: 1))
        .shadow(color: .black.opacity(0.18), radius: 10, y: 4)
        .onReceive(Self.timer) { value in
            guard !isPaused else { return }
            now = value
        }
    }

    private var elapsedText: String {
        let elapsed = isPaused ? runtime.elapsed : max(runtime.elapsed, now.timeIntervalSince(runtime.startedAt))
        let total = Int(elapsed.rounded(.down))
        return String(format: "%02d:%02d", total / 60, total % 60)
    }

    private static let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    private func statusIcon(enabled: Bool, symbol: String, help: String) -> some View {
        Image(systemName: symbol)
            .font(.caption.weight(.semibold))
            .foregroundStyle(enabled ? Color.primary : Color.secondary.opacity(0.45))
            .help(help)
    }
}

struct RecordingCountdownView: View {
    let secondsRemaining: Int
    var onCancel: () -> Void

    var body: some View {
        VStack(spacing: 12) {
            PopupHeader(
                icon: "record.circle",
                title: "Recording starts in",
                subtitle: nil,
                onClose: onCancel
            )
            Text("\(secondsRemaining)")
                .font(.system(size: 72, weight: .bold, design: .rounded))
                .monospacedDigit()
                .foregroundStyle(BoxTheme.accent)
            Text("Bello Box hides detected secure fields and suppresses key overlays while typing into them.")
                .font(.caption2)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding(20)
        .frame(width: 320, height: 240)
        .popupCard()
    }
}

struct RecordingErrorView: View {
    let message: String
    var onClose: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            PopupHeader(
                icon: "exclamationmark.triangle.fill",
                title: "Recording",
                subtitle: "Could not continue",
                onClose: onClose
            )
            Text(message)
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack {
                Spacer()
                Button("Close", action: onClose)
                    .buttonStyle(PrimaryButtonStyle())
            }
        }
        .padding(18)
        .frame(width: 420, height: 220)
        .popupCard()
    }
}
