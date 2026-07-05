import AppKit
import SwiftUI

/// The Settings window: configure the AI provider, prompt, appearance, behavior,
/// capture tools, and permissions.
struct SettingsView: View {
    @ObservedObject var settings: AppSettings

    @State private var selectedCategory: SettingsCategory = .general
    @State private var accessibilityTrusted = AccessibilityService.isTrusted
    @State private var screenRecordingTrusted = ScreenCapturePermission.isTrusted
    @State private var microphonePermission = MicrophonePermission.status()
    @State private var inputMonitoringPermission = InputMonitoringPermission.status()
    @State private var diagnosticsExportMessage: String?
    private let permissionTimer = Timer.publish(every: 1.5, on: .main, in: .common).autoconnect()

    var body: some View {
        HStack(spacing: 0) {
            sidebar
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    header
                    selectedContent
                }
                .padding(24)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }
        }
        .frame(width: 900, height: 720)
        .onReceive(permissionTimer) { _ in
            accessibilityTrusted = AccessibilityService.isTrusted
            screenRecordingTrusted = ScreenCapturePermission.isTrusted
            microphonePermission = MicrophonePermission.status()
            inputMonitoringPermission = InputMonitoringPermission.status()
        }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                if let icon = NSApp.applicationIconImage {
                    Image(nsImage: icon).resizable().frame(width: 34, height: 34)
                }
                VStack(alignment: .leading, spacing: 1) {
                    Text("Bello Box").font(.headline)
                    Text("Settings").font(.caption).foregroundStyle(.secondary)
                }
            }
            .padding(.horizontal, 12)
            .padding(.top, 14)
            .padding(.bottom, 8)

            ForEach(SettingsCategory.allCases) { category in
                sidebarButton(category)
            }

            Spacer()
        }
        .frame(width: 214)
        .background(Color(nsColor: .windowBackgroundColor).opacity(0.78))
    }

    private func sidebarButton(_ category: SettingsCategory) -> some View {
        Button {
            selectedCategory = category
        } label: {
            HStack(spacing: 10) {
                Image(systemName: category.symbol)
                    .font(.system(size: 14, weight: .semibold))
                    .frame(width: 22)
                VStack(alignment: .leading, spacing: 1) {
                    Text(category.title)
                        .font(.callout.weight(.semibold))
                    Text(category.subtitle)
                        .font(.caption2)
                        .foregroundColor(selectedCategory == category ? Color.white.opacity(0.82) : .secondary)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            .foregroundColor(selectedCategory == category ? .white : .primary)
            .padding(.horizontal, 12)
            .padding(.vertical, 9)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(selectedCategory == category ? BoxTheme.accent : Color.clear)
            )
            .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 10)
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 5) {
            Label(selectedCategory.title, systemImage: selectedCategory.symbol)
                .font(.system(size: 25, weight: .bold))
            Text(selectedCategory.explanation)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    @ViewBuilder
    private var selectedContent: some View {
        switch selectedCategory {
        case .general:
            generalPage
        case .ai:
            aiPage
        case .capture:
            capturePage
        case .recording:
            recordingPage
        case .ocr:
            ocrPage
        case .permissions:
            permissionsPage
        case .prompt:
            promptPage
        }
    }

    private var generalPage: some View {
        VStack(alignment: .leading, spacing: 14) {
            settingsSection("Startup", subtitle: "Choose whether Bello Box is ready as soon as you sign in.", systemImage: "power") {
                Toggle("Open Bello Box when I start my Mac", isOn: $settings.launchAtLoginEnabled)
            }

            settingsSection("Tool Board", subtitle: "Control how the small selection toolbar appears.", systemImage: "cursorarrow.rays") {
                Toggle("Show auto hint after I select text", isOn: $settings.floatingButtonEnabled)
                Toggle("Enable global shortcut \(settings.globalHotkey.displayString)", isOn: $settings.globalHotkeyEnabled)
                LabeledContent("Shortcut") {
                    HotkeyRecorderView(settings: settings)
                        .disabled(!settings.globalHotkeyEnabled)
                }
                hotkeyConflictWarnings()
                helpText("Auto hint appears after mouse selections. The shortcut summons the same board for the current selection.")
            }

            settingsSection("Appearance", subtitle: "Match the system or keep Bello Box fixed in one theme.", systemImage: "circle.lefthalf.filled") {
                Picker("Theme", selection: $settings.appearance) {
                    ForEach(AppearancePreference.allCases) { preference in
                        Label(preference.label, systemImage: preference.symbol).tag(preference)
                    }
                }
                .pickerStyle(.segmented)
            }
        }
    }

    private var aiPage: some View {
        settingsSection("Provider", subtitle: "Bring your own endpoint, API key, model, or local Codex app-server.", systemImage: "antenna.radiowaves.left.and.right") {
            ProviderConfigView(settings: settings)
        }
    }

    private var capturePage: some View {
        VStack(alignment: .leading, spacing: 14) {
            settingsSection("Screenshot Shortcut", subtitle: "Open the unified selector: hover a window, click blank space for the screen, or drag a rectangle.", systemImage: "camera.viewfinder") {
                Toggle("Enable screenshot shortcut \(settings.screenshotHotkey.displayString)", isOn: $settings.screenshotHotkeyEnabled)
                LabeledContent("Shortcut") {
                    ScreenshotHotkeyRecorderView(settings: settings)
                        .disabled(!settings.screenshotHotkeyEnabled)
                }
                hotkeyConflictWarnings()
                Toggle("Include cursor in screenshots", isOn: $settings.screenshotIncludeCursor)
                Toggle("Auto-copy captured screenshot", isOn: $settings.screenshotAutoCopy)
            }

            settingsSection("Capture Behavior", subtitle: "These controls affect screenshot capture and scrolling screenshots.", systemImage: "rectangle.dashed") {
                VStack(alignment: .leading, spacing: 8) {
                    captureHint("Hover", "Highlights the window under the pointer.")
                    captureHint("Click", "Captures the highlighted window, or the whole screen on blank space.")
                    captureHint("Drag", "Captures the rectangle you draw and keeps editing inline.")
                }
                Divider()
                Stepper(value: $settings.scrollingScreenshotMaxFrames, in: 2...60) {
                    Text("Scrolling max frames: \(settings.scrollingScreenshotMaxFrames)")
                }
                Toggle("Remove repeated sticky headers/footers", isOn: $settings.scrollingScreenshotAutoCompact)
                Divider()
                Picker("Advanced capture engine", selection: $settings.screenshotCaptureEngine) {
                    ForEach(ScreenshotCaptureEngine.allCases) { engine in
                        Text(engine.label).tag(engine)
                    }
                }
                .pickerStyle(.menu)
                helpText("Scrolling capture stays available from the menu. OCR only runs from the screenshot editor when you ask for it.")
            }

            settingsSection("Diagnostics", subtitle: "Capture display metadata when screenshot behavior needs debugging.", systemImage: "stethoscope") {
                Toggle("Enable screenshot diagnostics logging", isOn: $settings.captureDiagnosticsEnabled)
                HStack(spacing: 10) {
                    Button {
                        exportCaptureDiagnostics()
                    } label: {
                        Label("Export Diagnostics Log…", systemImage: "square.and.arrow.up")
                    }
                    if let diagnosticsExportMessage {
                        Text(diagnosticsExportMessage)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
                helpText("Logs include display IDs, screen frames, overlay decisions, and capture errors only. Bello Box does not log screenshot pixels, OCR text, image payloads, or API keys.")
            }
        }
    }

    private var recordingPage: some View {
        settingsSection("Recording Defaults", subtitle: "Set the options used when a recording target is selected.", systemImage: "record.circle") {
            Toggle("Include cursor in recordings", isOn: $settings.recordingIncludeCursor)
            AudioSourcePickerView(
                audioSource: Binding(
                    get: { settings.recordingAudioSource },
                    set: { settings.recordingAudioSource = $0 }
                ),
                microphoneDeviceID: $settings.recordingLastMicrophoneDeviceID
            )
            Picker("Click overlays", selection: Binding(
                get: { settings.recordingClickOverlayMode },
                set: { settings.recordingClickOverlayMode = $0 }
            )) {
                ForEach(ClickOverlayMode.allCases) { mode in
                    Text(mode.label).tag(mode)
                }
            }
            Picker("Keystroke overlays", selection: Binding(
                get: { settings.recordingKeystrokeMode },
                set: { settings.recordingKeystrokeMode = $0 }
            )) {
                ForEach(KeystrokeCaptureMode.allCases) { mode in
                    Text(mode.label).tag(mode)
                }
            }
            Picker("Secure-field protection", selection: Binding(
                get: { settings.recordingSecureFieldRedactionMode },
                set: { settings.recordingSecureFieldRedactionMode = $0 }
            )) {
                ForEach(SecureFieldRedactionMode.allCases) { mode in
                    Text(mode.label).tag(mode)
                }
            }
            if let warning = RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: accessibilityTrusted) {
                permissionWarning(warning) {
                    AccessibilityService.requestPermissionPrompt()
                    AccessibilityService.openAccessibilitySettings()
                }
            }
            Picker("Quality", selection: Binding(
                get: { settings.recordingQualityPreset },
                set: { settings.recordingQualityPreset = $0 }
            )) {
                ForEach(RecordingQualityPreset.allCases) { preset in
                    Text(preset.label).tag(preset)
                }
            }
            Stepper(value: $settings.recordingCountdownSeconds, in: 0...10) {
                Text("Countdown: \(settings.recordingCountdownSeconds)s")
            }
            Divider()
            Toggle("Enable recording shortcut \(settings.recordingHotkey.displayString)", isOn: $settings.recordingHotkeyEnabled)
            LabeledContent("Shortcut") {
                RecordingHotkeyRecorderView(settings: settings)
                    .disabled(!settings.recordingHotkeyEnabled)
            }
            hotkeyConflictWarnings()
            helpText("Default keystroke capture is shortcuts-only. Bello Box suppresses printable key overlays while typing into secure fields.")
        }
    }

    private var ocrPage: some View {
        settingsSection("Screenshot OCR", subtitle: "OCR is never automatic. These defaults apply only after you request OCR in the screenshot editor.", systemImage: "text.viewfinder") {
            Picker("OCR recognition", selection: $settings.ocrRecognitionLevel) {
                ForEach(OCRRecognitionLevel.allCases) { level in
                    Text(level.label).tag(level)
                }
            }
            Toggle("Use OCR language correction", isOn: $settings.ocrUseLanguageCorrection)
            Toggle("Show OCR text-region overlay by default", isOn: $settings.ocrShowTextRegions)
            TextField("OCR language hints, comma separated", text: Binding(
                get: { settings.ocrLanguageHintsText },
                set: { settings.setOCRLanguageHintsText($0) }
            ))
            Divider()
            Stepper(value: $settings.llmOCRMaxUploadLongEdge, in: 800...5000, step: 100) {
                Text("LLM OCR max long edge: \(settings.llmOCRMaxUploadLongEdge) px")
            }
            Toggle("Include Mac OCR as LLM OCR hint", isOn: $settings.llmOCRIncludeLocalOCRHint)
            helpText("LLM OCR still asks before uploading the edited screenshot image.")
        }
    }

    private var permissionsPage: some View {
        settingsSection("macOS Permissions", subtitle: "Bello Box asks lazily, but granting here makes setup predictable.", systemImage: "lock.shield") {
            permissionRow(
                title: "Accessibility access",
                detail: accessibilityTrusted
                    ? "Granted — Bello Box can read your selection and paste replacements."
                    : "Required to read selected text and replace it.",
                trusted: accessibilityTrusted,
                actionTitle: "Grant…",
                action: {
                    AccessibilityService.requestPermissionPrompt()
                    AccessibilityService.openAccessibilitySettings()
                }
            )
            permissionRow(
                title: "Screen Recording",
                detail: screenRecordingTrusted
                    ? "Granted — Bello Box can capture screenshots and recordings."
                    : "Required for screenshots and recordings.",
                trusted: screenRecordingTrusted,
                actionTitle: "Grant…",
                action: {
                    _ = ScreenCapturePermission.requestPrompt()
                    ScreenCapturePermission.openSettings()
                }
            )
            permissionRow(
                title: "Microphone",
                detail: microphonePermission == .granted
                    ? "Granted — Bello Box can record microphone audio when enabled."
                    : "Optional for recording microphone audio.",
                trusted: microphonePermission == .granted,
                actionTitle: "Grant…",
                action: {
                    Task {
                        microphonePermission = await MicrophonePermission.request()
                    }
                }
            )
            permissionRow(
                title: "Input Monitoring",
                detail: inputMonitoringPermission == .granted
                    ? "Granted — Bello Box can show click and keyboard overlays while recording."
                    : "Optional for click and keyboard overlays while recording.",
                trusted: inputMonitoringPermission == .granted,
                actionTitle: "Grant…",
                action: {
                    inputMonitoringPermission = InputMonitoringPermission.request()
                }
            )
        }
    }

    private var promptPage: some View {
        settingsSection("System Prompt", subtitle: "This instruction is sent with text AI actions.", systemImage: "text.alignleft") {
            TextEditor(text: $settings.systemPrompt)
                .font(.callout.monospaced())
                .frame(minHeight: 220)
            Button("Reset to default") { settings.resetSystemPrompt() }
                .font(.caption)
        }
    }

    private func settingsSection<Content: View>(
        _ title: String,
        subtitle: String,
        systemImage: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: systemImage)
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(BoxTheme.accent)
                    .frame(width: 26, height: 26)
                    .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(BoxTheme.accentSoft))
                VStack(alignment: .leading, spacing: 3) {
                    Text(title).font(.headline)
                    Text(subtitle).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                }
            }

            VStack(alignment: .leading, spacing: 12) {
                content()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(16)
        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(.primary.opacity(0.045)))
        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(.primary.opacity(0.07), lineWidth: 1))
    }

    private func captureHint(_ title: String, _ detail: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(title)
                .font(.caption.weight(.bold))
                .foregroundStyle(BoxTheme.accent)
                .frame(width: 54, alignment: .leading)
            Text(detail)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private func helpText(_ text: String) -> some View {
        Text(text)
            .font(.caption2)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func permissionWarning(_ message: String, action: @escaping () -> Void) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "lock.slash")
                .foregroundStyle(.orange)
            VStack(alignment: .leading, spacing: 5) {
                Text(message)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                Button("Open Accessibility Settings", action: action)
                    .font(.caption)
            }
        }
        .padding(10)
        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Color.orange.opacity(0.10)))
    }

    @ViewBuilder
    private func hotkeyConflictWarnings() -> some View {
        ForEach(settings.hotkeyConflictMessages, id: \.self) { message in
            Label(message, systemImage: "exclamationmark.triangle.fill")
                .font(.caption2)
                .foregroundStyle(.orange)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func permissionRow(title: String, detail: String, trusted: Bool, actionTitle: String, action: @escaping () -> Void) -> some View {
        HStack(alignment: .center, spacing: 12) {
            Image(systemName: trusted ? "checkmark.shield.fill" : "exclamationmark.shield.fill")
                .font(.system(size: 18, weight: .semibold))
                .foregroundStyle(trusted ? .green : .orange)
                .frame(width: 24)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                Text(detail)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            if !trusted {
                Button(actionTitle, action: action)
            }
        }
        .padding(.vertical, 3)
    }

    private func exportCaptureDiagnostics() {
        let panel = NSSavePanel()
        panel.title = "Export Bello Box Diagnostics"
        panel.nameFieldStringValue = "bello-box-capture-diagnostics.log"
        panel.canCreateDirectories = true

        guard panel.runModal() == .OK, let url = panel.url else { return }
        do {
            try CaptureDiagnostics.exportLog(to: url)
            diagnosticsExportMessage = "Exported."
        } catch {
            diagnosticsExportMessage = error.localizedDescription
        }
    }
}

private enum SettingsCategory: String, CaseIterable, Identifiable {
    case general
    case ai
    case capture
    case recording
    case ocr
    case permissions
    case prompt

    var id: String { rawValue }

    var title: String {
        switch self {
        case .general: return "General"
        case .ai: return "AI Provider"
        case .capture: return "Screenshots"
        case .recording: return "Recording"
        case .ocr: return "OCR"
        case .permissions: return "Permissions"
        case .prompt: return "Prompt"
        }
    }

    var subtitle: String {
        switch self {
        case .general: return "Startup & hint"
        case .ai: return "Endpoint & model"
        case .capture: return "Shortcut & flow"
        case .recording: return "Audio & privacy"
        case .ocr: return "Screenshot text"
        case .permissions: return "macOS access"
        case .prompt: return "AI instruction"
        }
    }

    var explanation: String {
        switch self {
        case .general:
            return "Set how Bello Box starts, appears, and follows your system theme."
        case .ai:
            return "Choose the AI provider Bello Box uses for rewrite, summarize, translate, and ask actions."
        case .capture:
            return "Configure screenshot shortcuts and the capture behavior users see before editing."
        case .recording:
            return "Set default recording options before choosing an area, window, or screen."
        case .ocr:
            return "Tune OCR defaults. OCR runs only from the screenshot editor after you request it."
        case .permissions:
            return "Review the macOS permissions needed for selection tools, screenshots, and recordings."
        case .prompt:
            return "Customize the system prompt used for text AI actions."
        }
    }

    var symbol: String {
        switch self {
        case .general: return "gearshape"
        case .ai: return "sparkles"
        case .capture: return "camera.viewfinder"
        case .recording: return "record.circle"
        case .ocr: return "text.viewfinder"
        case .permissions: return "lock.shield"
        case .prompt: return "text.alignleft"
        }
    }
}
