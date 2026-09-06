import SwiftUI

struct RecordingHUDView: View {
    var runtime: RecordingRuntimeState
    var isPaused: Bool
    var onPauseResume: () -> Void
    var onStop: () -> Void
    var onInputOverlaysChange: (ClickOverlayMode, KeystrokeCaptureMode) -> Void

    @State private var now = Date()

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                Label(isPaused ? "Paused" : "Recording", systemImage: isPaused ? "pause.circle.fill" : "record.circle.fill")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(isPaused ? .orange : .red)
                Text(elapsedText)
                    .font(.system(.body, design: .monospaced).weight(.semibold))
                    .accessibilityLabel("Recording duration")
                    .accessibilityValue(elapsedText)
                Spacer()
                Button(isPaused ? "Resume" : "Pause", action: onPauseResume)
                    .buttonStyle(SecondaryButtonStyle())
                    .controlSize(.small)
                    .help(isPaused ? "Continue this recording" : "Pause without ending the recording")
                Button("Stop & Review", action: onStop)
                    .buttonStyle(PrimaryButtonStyle())
                    .controlSize(.small)
                    .help("Finish saving the recording and open its preview")
            }

            HStack(spacing: 14) {
                audioStatus(enabled: runtime.isMicEnabled, label: "Mic", onSymbol: "mic.fill", offSymbol: "mic.slash", help: "Microphone")
                audioStatus(enabled: runtime.isSystemAudioEnabled, label: "Mac audio", onSymbol: "speaker.wave.2.fill", offSymbol: "speaker.slash", help: "Mac audio")
                Spacer(minLength: 0)
                Menu {
                    Toggle("Show Clicks", isOn: Binding(
                        get: { runtime.clickOverlayMode.isEnabled },
                        set: { onInputOverlaysChange($0 ? .ringsAndLabels : .off, runtime.keystrokeMode) }
                    ))
                    Picker("Show Keys", selection: Binding(
                        get: { runtime.keystrokeMode },
                        set: { onInputOverlaysChange(runtime.clickOverlayMode, $0) }
                    )) {
                        ForEach(KeystrokeCaptureMode.allCases) { mode in
                            Text(mode.label).tag(mode)
                        }
                    }
                    if let warning = runtime.inputOverlayWarning {
                        Divider()
                        Text(warning)
                    }
                } label: {
                    Label(runtime.isInputOverlayEnabled ? "Input On" : "Input Off", systemImage: runtime.inputOverlayWarning == nil ? "keyboard" : "exclamationmark.triangle")
                }
                .fixedSize()
                .help(runtime.inputOverlayWarning ?? "Keys: \(runtime.keystrokeMode.label). Clicks: \(runtime.clickOverlayMode.label). Change tracking while recording or paused.")
                .accessibilityLabel("Input Tracking")
                .accessibilityValue("Keys: \(runtime.keystrokeMode.label). Clicks: \(runtime.clickOverlayMode.label)")
            }

            if runtime.isSecureFieldHidden {
                Label("Secure field hidden", systemImage: "lock.shield")
                    .font(.caption)
                    .foregroundStyle(.orange)
            } else if let warning = secureFieldRedactionWarning {
                Label("Secure-field hiding off", systemImage: "lock.slash")
                    .font(.caption)
                    .foregroundStyle(.orange)
                    .help(warning)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .popupCard()
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

    private var secureFieldRedactionWarning: String? {
        RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: AccessibilityService.isTrusted)
    }

    private func audioStatus(enabled: Bool, label: String, onSymbol: String, offSymbol: String, help: String) -> some View {
        Label("\(label) \(enabled ? "On" : "Off")", systemImage: enabled ? onSymbol : offSymbol)
            .font(.caption)
            .foregroundStyle(enabled ? Color.primary : Color.secondary)
            .help("\(help): \(enabled ? "On" : "Off")")
            .accessibilityLabel("\(help): \(enabled ? "On" : "Off")")
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
            if let warning = secureFieldRedactionWarning {
                Label(warning, systemImage: "lock.slash")
                    .font(.caption2)
                    .foregroundStyle(.orange)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                Text("Bello Box hides detected secure fields and suppresses key overlays while typing into them.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            }
        }
        .padding(20)
        .frame(width: secureFieldRedactionWarning == nil ? 320 : 340, height: secureFieldRedactionWarning == nil ? 240 : 280)
        .popupCard()
    }

    private var secureFieldRedactionWarning: String? {
        RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: AccessibilityService.isTrusted)
    }
}

struct RecordingFinishingView: View {
    var body: some View {
        VStack(spacing: 12) {
            HStack(spacing: 10) {
                Image(systemName: "record.circle")
                    .font(.system(size: 14, weight: .bold))
                    .foregroundStyle(.white)
                    .frame(width: 30, height: 30)
                    .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(BoxTheme.accentGradient))
                VStack(alignment: .leading, spacing: 1) {
                    Text("Finishing Recording").font(.headline)
                    Text("Saving movie").font(.caption2).foregroundStyle(.secondary)
                }
                Spacer()
            }
            ProgressView()
                .controlSize(.large)
            Text("Preparing the recording file.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(20)
        .frame(width: 320, height: 190)
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
