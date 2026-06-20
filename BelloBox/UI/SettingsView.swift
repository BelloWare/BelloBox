import SwiftUI

/// The Settings window: configure the AI provider, prompt, appearance, behavior,
/// and review permissions.
struct SettingsView: View {
    @ObservedObject var settings: AppSettings

    @State private var accessibilityTrusted = AccessibilityService.isTrusted
    @State private var screenRecordingTrusted = ScreenCapturePermission.isTrusted
    private let permissionTimer = Timer.publish(every: 1.5, on: .main, in: .common).autoconnect()

    var body: some View {
        Form {
            Section("AI Provider") {
                ProviderConfigView(settings: settings)
            }
            promptSection
            appearanceSection
            behaviorSection
            screenshotSection
            permissionSection
        }
        .formStyle(.grouped)
        .frame(width: 640, height: 720)
        .onReceive(permissionTimer) { _ in
            accessibilityTrusted = AccessibilityService.isTrusted
            screenRecordingTrusted = ScreenCapturePermission.isTrusted
        }
    }

    private var promptSection: some View {
        Section("System Prompt") {
            TextEditor(text: $settings.systemPrompt)
                .font(.callout.monospaced())
                .frame(minHeight: 90)
            Button("Reset to default") { settings.resetSystemPrompt() }
                .font(.caption)
        }
    }

    private var appearanceSection: some View {
        Section("Appearance") {
            Picker("Theme", selection: $settings.appearance) {
                ForEach(AppearancePreference.allCases) { preference in
                    Label(preference.label, systemImage: preference.symbol).tag(preference)
                }
            }
            .pickerStyle(.segmented)
            Text("Follow the system setting, or force Light or Dark for Bello Box.")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
    }

    private var behaviorSection: some View {
        Section("Behavior") {
            Toggle("Show auto hint after I select text", isOn: $settings.floatingButtonEnabled)
            Toggle("Enable global shortcut \(settings.globalHotkey.displayString)", isOn: $settings.globalHotkeyEnabled)
            LabeledContent("Shortcut") {
                HotkeyRecorderView(settings: settings)
            }
            Text("Auto hint shows the tool board after mouse selections. The shortcut can summon the same board on the current selection even when auto hint is off.")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
    }

    private var screenshotSection: some View {
        Section("Screenshot") {
            Toggle("Include cursor in screenshots", isOn: $settings.screenshotIncludeCursor)
            Toggle("Auto-copy captured screenshot", isOn: $settings.screenshotAutoCopy)
            Toggle("Auto-run local OCR after capture", isOn: $settings.screenshotAutoRunLocalOCR)
            Toggle("Enable screenshot shortcut \(settings.screenshotHotkey.displayString)", isOn: $settings.screenshotHotkeyEnabled)
            LabeledContent("Screenshot shortcut") {
                ScreenshotHotkeyRecorderView(settings: settings)
            }

            Picker("Default capture", selection: $settings.screenshotDefaultMode) {
                ForEach(ScreenshotDefaultMode.allCases) { mode in
                    Text(mode.label).tag(mode)
                }
            }
            Text("The screenshot shortcut starts the default capture mode above.")
                .font(.caption2)
                .foregroundStyle(.secondary)

            Stepper(value: $settings.scrollingScreenshotMaxFrames, in: 2...60) {
                Text("Scrolling max frames: \(settings.scrollingScreenshotMaxFrames)")
            }
            Toggle("Remove repeated sticky headers/footers", isOn: $settings.scrollingScreenshotAutoCompact)

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
            Stepper(value: $settings.llmOCRMaxUploadLongEdge, in: 800...5000, step: 100) {
                Text("LLM OCR max long edge: \(settings.llmOCRMaxUploadLongEdge) px")
            }
            Toggle("Include Mac OCR as LLM OCR hint", isOn: $settings.llmOCRIncludeLocalOCRHint)
        }
    }

    private var permissionSection: some View {
        Section("Permissions") {
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
                    ? "Granted — Bello Box can capture screenshots."
                    : "Required for screenshots and scrolling capture.",
                trusted: screenRecordingTrusted,
                actionTitle: "Grant…",
                action: {
                    _ = ScreenCapturePermission.requestPrompt()
                    ScreenCapturePermission.openSettings()
                }
            )
        }
    }

    private func permissionRow(title: String, detail: String, trusted: Bool, actionTitle: String, action: @escaping () -> Void) -> some View {
        HStack {
            Image(systemName: trusted ? "checkmark.shield.fill" : "exclamationmark.shield.fill")
                .foregroundStyle(trusted ? .green : .orange)
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
    }
}
