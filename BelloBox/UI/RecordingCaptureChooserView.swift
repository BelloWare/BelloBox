import SwiftUI

struct RecordingCaptureChooserView: View {
    static let preferredSize = CGSize(width: 560, height: 620)

    @ObservedObject var settings: AppSettings
    var initialMode: RecordingCaptureMode?
    var onArea: (RecordingOptions) -> Void
    var onWindow: (RecordingOptions) -> Void
    var onDisplay: (RecordingOptions) -> Void
    var onCancel: () -> Void

    @State private var mode: RecordingCaptureMode
    @State private var options: RecordingOptions
    @State private var showAdvancedPrivacy = false

    init(
        settings: AppSettings,
        initialMode: RecordingCaptureMode? = nil,
        onArea: @escaping (RecordingOptions) -> Void,
        onWindow: @escaping (RecordingOptions) -> Void,
        onDisplay: @escaping (RecordingOptions) -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.settings = settings
        self.initialMode = initialMode
        self.onArea = onArea
        self.onWindow = onWindow
        self.onDisplay = onDisplay
        self.onCancel = onCancel
        _mode = State(initialValue: initialMode ?? .area)
        _options = State(initialValue: settings.recordingOptions)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            PopupHeader(
                icon: "record.circle",
                title: "Record",
                subtitle: "Capture video, audio, and safe teaching overlays",
                onClose: onCancel
            )

            SectionBox(title: "Capture") {
                Picker("Capture", selection: $mode) {
                    ForEach(RecordingCaptureMode.allCases) { mode in
                        Label(mode.label, systemImage: mode.symbol).tag(mode)
                    }
                }
                .pickerStyle(.segmented)
            }

            SectionBox(title: "Audio") {
                AudioSourcePickerView(
                    audioSource: $options.audioSource,
                    microphoneDeviceID: $options.microphoneDeviceID
                )
                Text("Mac Audio records sound playing from your Mac, excluding Bello Box when possible.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }

            SectionBox(title: "Viewer overlays") {
                Toggle("Show clicks", isOn: Binding(
                    get: { options.clickOverlayMode != .off },
                    set: { options.clickOverlayMode = $0 ? .ringsAndLabels : .off }
                ))
                Toggle("Show keystrokes", isOn: Binding(
                    get: { options.keystrokeMode != .off },
                    set: { options.keystrokeMode = $0 ? .shortcutsOnly : .off }
                ))
                Picker("Keystroke mode", selection: $options.keystrokeMode) {
                    ForEach(KeystrokeCaptureMode.allCases) { mode in
                        Text(mode.label).tag(mode)
                    }
                }
                .disabled(options.keystrokeMode == .off)
            }

            SectionBox(title: "Privacy") {
                Label("Secure-field protection is always enabled for recordings.", systemImage: "lock.shield")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                DisclosureGroup("Advanced redaction mode", isExpanded: $showAdvancedPrivacy) {
                    Picker("Redaction", selection: $options.secureFieldRedactionMode) {
                        ForEach(SecureFieldRedactionMode.allCases) { mode in
                            Text(mode.label).tag(mode)
                        }
                    }
                }
                Text("Bello Box hides detected secure fields and suppresses key overlays while typing into them. Microphone audio may still include anything spoken aloud.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            SectionBox(title: "Output") {
                Picker("Quality", selection: $options.quality) {
                    ForEach(RecordingQualityPreset.allCases) { preset in
                        Text(preset.label).tag(preset)
                    }
                }
                .pickerStyle(.segmented)
                Stepper(value: $options.countdownSeconds, in: 0...10) {
                    Text("Countdown: \(options.countdownSeconds)s")
                }
                Toggle("Include cursor", isOn: $options.includeCursor)
            }

            Spacer(minLength: 0)

            HStack {
                Button("Cancel", action: onCancel)
                    .buttonStyle(SecondaryButtonStyle())
                Spacer()
                Button("Start Recording", action: start)
                    .buttonStyle(PrimaryButtonStyle())
            }
        }
        .padding(18)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height)
        .popupCard()
    }

    private func start() {
        persistDefaults()
        switch mode {
        case .area: onArea(options)
        case .window: onWindow(options)
        case .display: onDisplay(options)
        }
    }

    private func persistDefaults() {
        settings.recordingAudioSource = options.audioSource
        settings.recordingIncludeCursor = options.includeCursor
        settings.recordingClickOverlayMode = options.clickOverlayMode
        settings.recordingKeystrokeMode = options.keystrokeMode
        settings.recordingSecureFieldRedactionMode = options.secureFieldRedactionMode
        settings.recordingQualityPreset = options.quality
        settings.recordingCountdownSeconds = options.countdownSeconds
        settings.recordingLastMicrophoneDeviceID = options.microphoneDeviceID
    }
}

private struct SectionBox<Content: View>: View {
    let title: String
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            content
        }
        .toolPanel()
    }
}
