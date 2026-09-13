import SwiftUI

/// The floating card shown while recording: one status row (state, elapsed time,
/// pause and stop) and one row of chips for audio, input tracking and privacy,
/// so nothing important hides in a menu but the card stays two lines tall.
struct RecordingHUDView: View {
    static let preferredSize = CGSize(width: 520, height: 104)

    var runtime: RecordingRuntimeState
    var isPaused: Bool
    var onPauseResume: () -> Void
    var onStop: () -> Void
    var onInputOverlaysChange: (ClickOverlayMode, KeystrokeCaptureMode) -> Void

    @State private var now = Date()

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Label(isPaused ? "Paused" : "Recording", systemImage: isPaused ? "pause.circle.fill" : "record.circle.fill")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(isPaused ? BoxTheme.warning : BoxTheme.danger)
                Text(elapsedText)
                    .font(.system(.body, design: .monospaced).weight(.semibold))
                    .accessibilityLabel("Recording duration")
                    .accessibilityValue(elapsedText)
                if runtime.outputFormat == .gif {
                    Text("GIF")
                        .font(.caption2.weight(.bold))
                        .padding(.horizontal, 6).padding(.vertical, 2)
                        .background(BoxTheme.accentSoft, in: Capsule())
                        .foregroundStyle(BoxTheme.accent)
                        .help("A silent GIF will be written when you stop; the movie is kept too")
                        .accessibilityLabel("Output: GIF")
                }
                Spacer()
                Button(isPaused ? "Resume" : "Pause", action: onPauseResume)
                    .buttonStyle(SecondaryButtonStyle())
                    .controlSize(.small)
                    .help(isPaused ? "Continue this recording" : "Pause without ending the recording")
                Button("Stop & Review", action: onStop)
                    .buttonStyle(PrimaryButtonStyle())
                    .controlSize(.small)
                    .help(runtime.outputFormat == .gif ? "Finish the movie, write the GIF, and open the review" : "Finish saving the recording and open its preview")
            }

            HStack(spacing: 6) {
                chip(runtime.isMicEnabled ? "Mic on" : "Mic off", symbol: runtime.isMicEnabled ? "mic.fill" : "mic.slash",
                     on: runtime.isMicEnabled, help: "Microphone: \(runtime.isMicEnabled ? "on" : "off")",
                     accessibilityLabel: "Microphone \(runtime.isMicEnabled ? "on" : "off")")
                chip(runtime.isSystemAudioEnabled ? "Mac audio on" : "Mac audio off", symbol: runtime.isSystemAudioEnabled ? "speaker.wave.2.fill" : "speaker.slash",
                     on: runtime.isSystemAudioEnabled, help: "Mac audio: \(runtime.isSystemAudioEnabled ? "on" : "off")",
                     accessibilityLabel: "Mac audio \(runtime.isSystemAudioEnabled ? "on" : "off")")
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
                    chip(inputChipTitle, symbol: runtime.inputOverlayWarning == nil ? "keyboard" : "exclamationmark.triangle",
                         on: runtime.isInputOverlayEnabled, help: "", accessibilityLabel: "Input tracking")
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
                .help(runtime.inputOverlayWarning ?? "Keys: \(runtime.keystrokeMode.label). Clicks: \(runtime.clickOverlayMode.label). Change tracking while recording or paused.")
                .accessibilityLabel("Input tracking")
                .accessibilityValue("Keys: \(runtime.keystrokeMode.label). Clicks: \(runtime.clickOverlayMode.label)")
                Spacer(minLength: 0)
                privacyChip
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

    /// Short enough to sit beside the audio and privacy chips at the HUD's width;
    /// the menu's help and accessibility value carry the full modes.
    static func inputChipTitle(for runtime: RecordingRuntimeState) -> String {
        guard runtime.inputOverlayWarning == nil else { return "Input unavailable" }
        guard runtime.isInputOverlayEnabled else { return "Input off" }
        var parts: [String] = []
        if runtime.clickOverlayMode.isEnabled { parts.append("Clicks") }
        if runtime.keystrokeMode != .off { parts.append(runtime.keystrokeMode == .shortcutsOnly ? "shortcuts" : "keys") }
        return parts.joined(separator: " + ")
    }

    private var inputChipTitle: String { Self.inputChipTitle(for: runtime) }

    /// Privacy in one word each; the help and accessibility label spell it out.
    @ViewBuilder private var privacyChip: some View {
        if runtime.isSecureFieldHidden {
            chip("Hiding a field", symbol: "lock.shield.fill", on: true, tint: BoxTheme.warning,
                 help: "A password field is being hidden in the recording right now",
                 accessibilityLabel: "Secure field hidden right now")
        } else if let warning = secureFieldRedactionWarning {
            chip("Not protected", symbol: "lock.slash", on: true, tint: BoxTheme.warning, help: warning,
                 accessibilityLabel: "Secure fields are not hidden. \(warning)")
        } else {
            chip("Protected", symbol: "lock.shield", on: false, help: "Password fields are hidden while you type in them",
                 accessibilityLabel: "Secure fields are hidden while you type in them")
        }
    }

    /// A state chip is never truncated: the row is sized so every chip fits
    /// (`RecordingGIFFlowTests` measures the widest combination).
    private func chip(_ title: String, symbol: String, on: Bool, tint: Color? = nil, help: String, accessibilityLabel: String) -> some View {
        Label(title, systemImage: symbol)
            .font(.caption2.weight(.medium))
            .foregroundStyle(tint ?? (on ? Color.primary : BoxTheme.secondaryText))
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 8).padding(.vertical, 4)
            .background { ToolSurface(role: .control).clipShape(Capsule()) }
            .overlay(Capsule().strokeBorder(BoxTheme.separator))
            .help(help)
            .accessibilityLabel(accessibilityLabel)
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
                    .foregroundStyle(BoxTheme.warning)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                Text("Bello Box hides detected secure fields and suppresses key overlays while typing into them.")
                    .font(.caption2)
                    .foregroundStyle(BoxTheme.secondaryText)
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
                    Text("Saving movie").font(.caption2).foregroundStyle(BoxTheme.secondaryText)
                }
                Spacer()
            }
            ProgressView()
                .controlSize(.large)
            Text("Preparing the recording file.")
                .font(.caption)
                .foregroundStyle(BoxTheme.secondaryText)
        }
        .padding(20)
        .frame(width: 320, height: 190)
        .popupCard()
    }
}

/// Progress of the GIF being written, updated in place so the card is presented once.
@MainActor
final class RecordingConversionProgress: ObservableObject {
    @Published var progress: Double
    init(progress: Double) { self.progress = progress }
}

/// Shown while the finished movie is turned into a GIF. Progress is real (frames
/// written over frames planned); cancelling keeps the movie and reviews it.
struct RecordingConversionView: View {
    static let preferredSize = CGSize(width: 360, height: 190)
    @ObservedObject var model: RecordingConversionProgress
    var onCancel: () -> Void
    private var progress: Double { model.progress }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 10) {
                Image(systemName: "photo.stack")
                    .font(.system(size: 14, weight: .bold))
                    .foregroundStyle(.white)
                    .frame(width: 30, height: 30)
                    .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(BoxTheme.accentGradient))
                VStack(alignment: .leading, spacing: 1) {
                    Text("Writing GIF").font(.headline)
                    Text("The movie is saved; the GIF is being built from it").font(.caption2).foregroundStyle(BoxTheme.secondaryText)
                }
                Spacer()
            }
            ProgressView(value: min(max(progress, 0), 1))
                .progressViewStyle(.linear)
                .tint(BoxTheme.accent)
                .accessibilityLabel("GIF progress")
                .accessibilityValue("\(Int((progress * 100).rounded())) percent")
            HStack {
                Text("\(Int((progress * 100).rounded()))% · frames written")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(BoxTheme.secondaryText)
                Spacer()
                Button("Keep Movie Only", action: onCancel)
                    .buttonStyle(SecondaryButtonStyle())
                    .keyboardShortcut(.cancelAction)
                    .help("Stop writing the GIF and review the movie instead")
            }
        }
        .padding(18)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height)
        .popupCard()
        .onExitCommand(perform: onCancel)
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
                .foregroundStyle(BoxTheme.secondaryText)
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
