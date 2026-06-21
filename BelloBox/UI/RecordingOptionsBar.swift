import SwiftUI

struct RecordingOptionsBar: View {
    @ObservedObject var settings: AppSettings
    var targetLabel: String
    var onStart: (RecordingOptions) -> Void
    var onCancel: () -> Void

    @State private var options: RecordingOptions
    @State private var showAdvanced = false

    init(
        settings: AppSettings,
        targetLabel: String,
        initialOptions: RecordingOptions,
        onStart: @escaping (RecordingOptions) -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.settings = settings
        self.targetLabel = targetLabel
        self.onStart = onStart
        self.onCancel = onCancel
        _options = State(initialValue: initialOptions)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                Label(targetLabel, systemImage: "record.circle")
                    .font(.headline)
                    .lineLimit(1)
                Spacer()
                Button(action: onCancel) {
                    Image(systemName: "xmark")
                }
                .buttonStyle(SecondaryButtonStyle())
                .help("Cancel")
            }

            ViewThatFits(in: .horizontal) {
                HStack(alignment: .top, spacing: 12) {
                    audioColumn.frame(width: 220, alignment: .leading)
                    inputColumn.frame(width: 190, alignment: .leading)
                    qualityColumn.frame(width: 220, alignment: .leading)
                }

                VStack(alignment: .leading, spacing: 12) {
                    audioColumn
                    Divider()
                    inputColumn
                    Divider()
                    qualityColumn
                }
            }

            DisclosureGroup(privacyDisclosureTitle, isExpanded: $showAdvanced) {
                VStack(alignment: .leading, spacing: 8) {
                    Picker("Redaction", selection: $options.secureFieldRedactionMode) {
                        ForEach(SecureFieldRedactionMode.allCases) { mode in
                            Text(mode.label).tag(mode)
                        }
                    }
                    .frame(width: 260)

                    if let warning = secureFieldRedactionWarning {
                        HStack(alignment: .top, spacing: 8) {
                            Image(systemName: "lock.slash")
                                .foregroundStyle(.orange)
                            VStack(alignment: .leading, spacing: 6) {
                                Text(warning)
                                    .foregroundStyle(.secondary)
                                    .fixedSize(horizontal: false, vertical: true)
                                Button("Open Accessibility Settings") {
                                    AccessibilityService.openAccessibilitySettings()
                                }
                                .buttonStyle(.link)
                            }
                        }
                        .padding(8)
                        .frame(maxWidth: 360, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.orange.opacity(0.10)))
                    }
                }
            }
            .font(.caption)

            HStack {
                Spacer()
                Button("Start Recording") {
                    persistDefaults()
                    onStart(options)
                }
                .buttonStyle(PrimaryButtonStyle())
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(.regularMaterial))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(.white.opacity(0.14), lineWidth: 1))
        .shadow(color: .black.opacity(0.28), radius: 16, y: 8)
    }

    private var audioColumn: some View {
        VStack(alignment: .leading, spacing: 8) {
            AudioSourcePickerView(
                audioSource: $options.audioSource,
                microphoneDeviceID: $options.microphoneDeviceID
            )
            Toggle("Cursor", isOn: $options.includeCursor)
        }
    }

    private var inputColumn: some View {
        VStack(alignment: .leading, spacing: 8) {
            Toggle("Clicks", isOn: Binding(
                get: { options.clickOverlayMode != .off },
                set: { options.clickOverlayMode = $0 ? .ringsAndLabels : .off }
            ))
            Picker("Keys", selection: $options.keystrokeMode) {
                ForEach(KeystrokeCaptureMode.allCases) { mode in
                    Text(mode.label).tag(mode)
                }
            }
        }
    }

    private var qualityColumn: some View {
        VStack(alignment: .leading, spacing: 8) {
            Picker("Quality", selection: $options.quality) {
                ForEach(RecordingQualityPreset.allCases) { preset in
                    Text(preset.label).tag(preset)
                }
            }
            .pickerStyle(.segmented)
            Stepper(value: $options.countdownSeconds, in: 0...10) {
                Text("Countdown \(options.countdownSeconds)s")
            }
        }
    }

    private var secureFieldRedactionWarning: String? {
        RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: AccessibilityService.isTrusted)
    }

    private var privacyDisclosureTitle: String {
        secureFieldRedactionWarning == nil
            ? "Privacy: secure fields are hidden"
            : "Privacy: secure-field hiding needs Accessibility"
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
