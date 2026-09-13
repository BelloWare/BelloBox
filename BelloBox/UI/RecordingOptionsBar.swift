import SwiftUI

/// The pre-flight card shown beside the chosen recording target. Four short
/// rows, each with a label on the left: output, audio and cursor, clicks and
/// keys, privacy. Nothing is hidden behind a disclosure; the GIF row only
/// appears when GIF is the output, and small screens get a stacked layout.
struct RecordingOptionsBar: View {
    /// The card for movie output; GIF output adds one row (`preferredSize(for:)`).
    static let preferredSize = CGSize(width: 660, height: 392)
    /// Below this width the sections stack in one column (and scroll).
    static let minimumWidth: CGFloat = 420
    static let compactWidthThreshold: CGFloat = 600

    static func usesCompactLayout(width: CGFloat) -> Bool { width < compactWidthThreshold }

    /// Hosts size the card for the chosen output; `onFormatChange` lets them follow the
    /// picker so neither layout leaves an empty band.
    static func preferredSize(for format: RecordingOutputFormat) -> CGSize {
        CGSize(width: preferredSize.width, height: format == .gif ? preferredSize.height + 64 : preferredSize.height)
    }

    @ObservedObject var settings: AppSettings
    var targetLabel: String
    /// Stack the sections in one column; the host decides from the width it has.
    var compact = false
    var onFormatChange: (RecordingOutputFormat) -> Void = { _ in }
    var onStart: (RecordingOptions) -> Void
    var onCancel: () -> Void

    @State private var options: RecordingOptions

    init(
        settings: AppSettings,
        targetLabel: String,
        initialOptions: RecordingOptions,
        compact: Bool = false,
        onFormatChange: @escaping (RecordingOutputFormat) -> Void = { _ in },
        onStart: @escaping (RecordingOptions) -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.settings = settings
        self.targetLabel = targetLabel
        self.compact = compact
        self.onFormatChange = onFormatChange
        self.onStart = onStart
        self.onCancel = onCancel
        _options = State(initialValue: initialOptions)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            PopupHeader(icon: "record.circle", title: "Screen Recording", subtitle: targetLabel, onClose: onCancel)

            ScrollView {
                Group {
                    if compact { narrowRows } else { wideRows }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            footer
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .popupCard()
        .onExitCommand(perform: onCancel)
        .onChange(of: options.outputFormat) { onFormatChange($0) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Recording options")
    }

    // MARK: Layouts

    private var wideRows: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top, spacing: 12) {
                section("Output", systemImage: "square.and.arrow.down") { outputControls }
                section("Quality", systemImage: "dial.medium") { qualityControls }
            }
            if options.outputFormat == .gif {
                section("GIF", systemImage: "photo.stack") { gifControls }
            }
            HStack(alignment: .top, spacing: 12) {
                section("Audio & cursor", systemImage: "waveform") { audioControls }
                section("Clicks & keys", systemImage: "keyboard") { inputControls }
            }
            section("Privacy", systemImage: "lock.shield") { privacyControls }
        }
    }

    private var narrowRows: some View {
        VStack(alignment: .leading, spacing: 8) {
            section("Output", systemImage: "square.and.arrow.down") { outputControls }
            section("Quality", systemImage: "dial.medium") { qualityControls }
            if options.outputFormat == .gif {
                section("GIF", systemImage: "photo.stack") { gifControls }
            }
            section("Audio & cursor", systemImage: "waveform") { audioControls }
            section("Clicks & keys", systemImage: "keyboard") { inputControls }
            section("Privacy", systemImage: "lock.shield") { privacyControls }
        }
    }

    private func section<Content: View>(_ title: String, systemImage: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Label(title, systemImage: systemImage)
                .font(.system(size: 12, weight: .semibold))
                .labelStyle(.titleAndIcon)
            content()
        }
        .padding(.horizontal, 10).padding(.vertical, 7)
        .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
        .surfaceCard()
    }

    // MARK: Rows

    private var outputControls: some View {
        VStack(alignment: .leading, spacing: 5) {
            ToolMenuPicker("Output format", value: options.outputFormat == .gif ? "Animated GIF" : "Movie",
                           showsLabel: false, compact: true, selection: $options.outputFormat) {
                Text("Movie").tag(RecordingOutputFormat.movie)
                Text("Animated GIF").tag(RecordingOutputFormat.gif)
            }.fixedSize()
            .accessibilityLabel("Output format")
            .help("Movie keeps audio. GIF is a silent looping image; the movie is kept as well.")
            Text(options.outputFormat.detail)
                .font(.caption2)
                .foregroundStyle(BoxTheme.secondaryText)
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var qualityControls: some View {
        VStack(alignment: .leading, spacing: 5) {
            ToolChoiceBar(selection: $options.quality, choices: RecordingQualityPreset.allCases.map { ($0, $0.label) }, label: "Quality", compact: true)
            .accessibilityLabel("Recording quality")
            Stepper(value: $options.countdownSeconds, in: 0...10) {
                Text(options.countdownSeconds == 0 ? "No countdown" : "Countdown \(options.countdownSeconds) s")
                    .font(.caption)
            }
            .accessibilityLabel("Countdown seconds")
            .accessibilityValue("\(options.countdownSeconds)")
        }
    }

    /// The short note beside the GIF controls; wraps to a second line before it
    /// would ever truncate.
    static let gifGuidance = "Silent · up to \(Int(GIFExportOptions.maxDuration)) s, trim after recording"

    /// The same frame rate, longest edge and loop controls as the converter and
    /// the review, so the labels and help read the same everywhere. The controls
    /// keep their natural width (a squeezed toggle would wrap its label); the
    /// note takes what is left, or its own line in the stacked layout.
    @ViewBuilder private var gifControls: some View {
        let note = Text(Self.gifGuidance)
            .font(.caption2).foregroundStyle(BoxTheme.secondaryText).lineLimit(2)
            .fixedSize(horizontal: false, vertical: true)
        if compact {
            VStack(alignment: .leading, spacing: 5) {
                GIFFormatControls(options: $options.gif).fixedSize()
                note
            }
        } else {
            HStack(spacing: 14) {
                GIFFormatControls(options: $options.gif).fixedSize()
                Spacer(minLength: 0)
                note
            }
        }
    }

    private var audioControls: some View {
        VStack(alignment: .leading, spacing: 5) {
            AudioSourcePickerView(
                audioSource: $options.audioSource,
                microphoneDeviceID: $options.microphoneDeviceID,
                compact: true
            )
            .accessibilityLabel("Audio source")
            if options.outputFormat == .gif {
                Text(options.audioSource == .none ? "GIFs are silent." : "GIFs are silent; audio stays in the kept movie.")
                    .font(.caption2).foregroundStyle(BoxTheme.secondaryText).lineLimit(2)
            }
            Toggle("Show cursor", isOn: $options.includeCursor)
                .font(.caption)
        }
    }

    private var inputControls: some View {
        VStack(alignment: .leading, spacing: 5) {
            Toggle("Show click rings", isOn: Binding(
                get: { options.clickOverlayMode != .off },
                set: { options.clickOverlayMode = $0 ? .ringsAndLabels : .off }
            ))
            .font(.caption)
            HStack(spacing: 6) {
                Text("Keys").font(.caption)
                ToolMenuPicker("Show keys", value: options.keystrokeMode.label, showsLabel: false, compact: true, selection: $options.keystrokeMode) {
                    ForEach(KeystrokeCaptureMode.allCases) { mode in
                        Text(mode.label).tag(mode)
                    }
                }
                .labelsHidden()
                .fixedSize()
                .accessibilityLabel("Keystroke overlay")
            }
        }
    }

    private var privacyControls: some View {
        HStack(alignment: .center, spacing: 10) {
            Image(systemName: secureFieldRedactionWarning == nil ? "lock.shield.fill" : "lock.slash")
                .foregroundStyle(secureFieldRedactionWarning == nil ? BoxTheme.success : BoxTheme.warning)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(secureFieldRedactionWarning == nil ? "Secure fields are hidden while you type in them" : "Secure-field hiding needs Accessibility")
                    .font(.caption)
                if let warning = secureFieldRedactionWarning {
                    Text(warning).font(.caption2).foregroundStyle(BoxTheme.secondaryText).lineLimit(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 6)
            if secureFieldRedactionWarning != nil {
                Button("Open Accessibility Settings") { AccessibilityService.openAccessibilitySettings() }
                    .buttonStyle(ToolLinkButtonStyle()).font(.caption)
            }
            ToolMenuPicker("Redaction", value: options.secureFieldRedactionMode.label, showsLabel: false, compact: true, selection: $options.secureFieldRedactionMode) {
                ForEach(SecureFieldRedactionMode.allCases) { mode in
                    Text(mode.label).tag(mode)
                }
            }
            .labelsHidden()
            .fixedSize()
            .accessibilityLabel("Secure-field redaction")
            .help("How aggressively password fields are hidden: Strict hides more, Visual field only hides just the field")
        }
    }

    private var footer: some View {
        HStack(spacing: 10) {
            Text(startSummary)
                .font(.caption).foregroundStyle(BoxTheme.secondaryText).lineLimit(2)
            Spacer()
            Button("Cancel", action: onCancel)
                .buttonStyle(SecondaryButtonStyle())
                .keyboardShortcut(.cancelAction)
            Button(options.outputFormat == .gif ? "Record GIF" : "Start Recording") {
                persistDefaults()
                onStart(options)
            }
            .buttonStyle(PrimaryButtonStyle())
            .keyboardShortcut(.defaultAction)
        }
    }

    private var startSummary: String {
        let timing = options.countdownSeconds == 0 ? "Starts immediately" : "Starts after a \(options.countdownSeconds)-second countdown"
        let delivery = options.outputFormat == .gif ? "records a movie, then writes the GIF" : "saves a movie you can review"
        return "\(timing) · \(delivery)."
    }

    private var secureFieldRedactionWarning: String? {
        RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: AccessibilityService.isTrusted)
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
        settings.recordingOutputFormat = options.outputFormat
        settings.gifExportOptions = options.gif
    }
}
