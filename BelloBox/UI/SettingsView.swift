import SwiftUI

/// The Settings window: configure the AI provider, prompt, appearance, behavior,
/// and review permissions.
struct SettingsView: View {
    @ObservedObject var settings: AppSettings

    @State private var accessibilityTrusted = AccessibilityService.isTrusted
    private let permissionTimer = Timer.publish(every: 1.5, on: .main, in: .common).autoconnect()

    var body: some View {
        Form {
            Section("AI Provider") {
                ProviderConfigView(settings: settings)
            }
            promptSection
            appearanceSection
            behaviorSection
            permissionSection
        }
        .formStyle(.grouped)
        .frame(width: 640, height: 720)
        .onReceive(permissionTimer) { _ in
            accessibilityTrusted = AccessibilityService.isTrusted
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
            Toggle("Show the floating button when I select text", isOn: $settings.floatingButtonEnabled)
            Text("You can also summon Bello Box on the current selection with ⌃⌥⌘B.")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
    }

    private var permissionSection: some View {
        Section("Permissions") {
            HStack {
                Image(systemName: accessibilityTrusted ? "checkmark.shield.fill" : "exclamationmark.shield.fill")
                    .foregroundStyle(accessibilityTrusted ? .green : .orange)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Accessibility access")
                    Text(accessibilityTrusted
                        ? "Granted — Bello Box can read your selection and paste replacements."
                        : "Required to read selected text and replace it.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                if !accessibilityTrusted {
                    Button("Grant…") {
                        AccessibilityService.requestPermissionPrompt()
                        AccessibilityService.openAccessibilitySettings()
                    }
                }
            }
        }
    }
}
