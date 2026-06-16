import AppKit
import SwiftUI

/// First-run onboarding: explains the app, walks the user through granting
/// Accessibility, and helps them connect an AI provider.
struct OnboardingView: View {
    @ObservedObject var settings: AppSettings
    /// Called when Accessibility flips to granted (so monitors can restart).
    var onPermissionGranted: () -> Void
    var onFinish: () -> Void

    @State private var step = 0
    @State private var trusted = AccessibilityService.isTrusted

    private let stepCount = 5
    private let poll = Timer.publish(every: 1.0, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(spacing: 0) {
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .padding(34)

            Divider()
            footer
                .padding(.horizontal, 28)
                .padding(.vertical, 16)
        }
        .frame(width: 680, height: 720)
        .onReceive(poll) { _ in
            let now = AccessibilityService.isTrusted
            if now != trusted {
                trusted = now
                if now { onPermissionGranted() }
            }
        }
    }

    // MARK: - Steps

    @ViewBuilder
    private var content: some View {
        switch step {
        case 0: welcomeStep
        case 1: permissionStep
        case 2: behaviorStep
        case 3: providerStep
        default: doneStep
        }
    }

    private var welcomeStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            appBadge
            Text("Welcome to Bello Box")
                .font(.system(size: 32, weight: .bold))
            Text("Bello Box is a little toolbox for whatever text you already have in front of you — in any app.")
                .font(.title3)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            VStack(alignment: .leading, spacing: 12) {
                bullet("cursorarrow.rays", "Select text anywhere", "A small Bello Box toolbar can appear next to your selection.")
                bullet("wand.and.stars", "Ask the AI", "Fix grammar, rewrite, summarize, or translate — then copy or replace in place.")
                bullet("qrcode", "Make a QR code", "Turn a link or any text into a scannable QR code you can edit on the fly.")
            }
            .padding(.top, 4)
        }
    }

    private var permissionStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            stepHeader("Grant Accessibility access", systemImage: "lock.shield")
            Text("Bello Box uses macOS Accessibility to read the text you select and paste replacements back. It only reads a selection when you ask it to. Nothing is sent anywhere except to the AI endpoint you configure.")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            HStack(spacing: 12) {
                Image(systemName: trusted ? "checkmark.circle.fill" : "exclamationmark.circle.fill")
                    .font(.system(size: 26))
                    .foregroundStyle(trusted ? .green : .orange)
                VStack(alignment: .leading, spacing: 2) {
                    Text(trusted ? "Accessibility access granted" : "Accessibility access needed")
                        .font(.headline)
                    Text(trusted
                        ? "Bello Box is ready to read selections."
                        : "Toggle Bello Box on under Privacy & Security → Accessibility.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }
            .padding(14)
            .background(RoundedRectangle(cornerRadius: 12).fill(.primary.opacity(0.05)))

            if !trusted {
                Button {
                    AccessibilityService.requestPermissionPrompt()
                    AccessibilityService.openAccessibilitySettings()
                } label: {
                    Label("Open Accessibility Settings", systemImage: "arrow.up.forward.app")
                }
                .controlSize(.large)
                Text("This window updates automatically once you grant access.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var behaviorStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            stepHeader("Choose how Bello Box appears", systemImage: "switch.2")
            Text("Keep the automatic hint on for quick mouse selections, or turn it off and use a keyboard shortcut only.")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            VStack(alignment: .leading, spacing: 14) {
                Toggle(isOn: $settings.floatingButtonEnabled) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Show auto hint after selecting text")
                            .font(.headline)
                        Text("Bello Box shows the tool board next to fresh text selections.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                Toggle(isOn: $settings.globalHotkeyEnabled) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Enable global shortcut \(settings.globalHotkey.displayString)")
                            .font(.headline)
                        Text("Press the shortcut to show the same tool board for the current selection.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                HotkeyRecorderView(settings: settings)
                    .disabled(!settings.globalHotkeyEnabled)
                    .padding(.leading, 44)
            }
            .toggleStyle(.switch)
            .padding(16)
            .background(RoundedRectangle(cornerRadius: 12).fill(.primary.opacity(0.05)))
        }
    }

    private var providerStep: some View {
        VStack(alignment: .leading, spacing: 14) {
            stepHeader("Connect your AI", systemImage: "antenna.radiowaves.left.and.right")
            Text("Bring your own AI: pick a format, add your details, optionally Load the model list, then run a quick hello to confirm it works. You can Skip and set this up later in Settings.")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            ProviderConfigView(settings: settings)
        }
    }

    private var doneStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            appBadge
            Text("You're all set")
                .font(.system(size: 32, weight: .bold))
            Text(doneSummary)
                .font(.title3)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            VStack(alignment: .leading, spacing: 12) {
                bullet("menubar.arrow.up.rectangle", "Find Bello Box in the menu bar", "The ✨ icon opens Settings and this guide anytime.")
                if settings.globalHotkeyEnabled {
                    bullet("keyboard", "Summon with a hotkey", "\(settings.globalHotkey.displayString) shows the Bello Box tool board for whatever you have selected.")
                }
                if settings.floatingButtonEnabled {
                    bullet("cursorarrow.rays", "Use auto hint", "Select text with the mouse and the Bello Box tool board appears nearby.")
                }
            }
            .padding(.top, 4)

            if !settings.isConfigured {
                Label("No AI provider is configured yet — you can add one anytime in Settings.", systemImage: "info.circle")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    // MARK: - Footer

    private var footer: some View {
        HStack {
            if step > 0 {
                Button("Back") { withAnimation { step -= 1 } }
            }
            if step < stepCount - 1 {
                Button("Skip") { onFinish() }
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            HStack(spacing: 6) {
                ForEach(0..<stepCount, id: \.self) { index in
                    Circle()
                        .fill(index == step ? BoxTheme.accent : Color.primary.opacity(0.18))
                        .frame(width: 7, height: 7)
                }
            }
            Spacer()
            if step < stepCount - 1 {
                Button("Continue") { withAnimation { step += 1 } }
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
                    .tint(BoxTheme.accent)
            } else {
                Button("Start Using Bello Box") { onFinish() }
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
                    .tint(BoxTheme.accent)
            }
        }
    }

    // MARK: - Pieces

    private var appBadge: some View {
        Group {
            if let icon = NSApp.applicationIconImage {
                Image(nsImage: icon).resizable().frame(width: 88, height: 88)
            } else {
                Image(systemName: "wand.and.stars")
                    .font(.system(size: 48))
                    .foregroundStyle(BoxTheme.accent)
            }
        }
    }

    private var doneSummary: String {
        switch (settings.floatingButtonEnabled, settings.globalHotkeyEnabled) {
        case (true, true):
            return "Select text in any app and use the Bello Box button that appears — or press \(settings.globalHotkey.displayString) to summon the tool board on the current selection."
        case (true, false):
            return "Select text in any app, then use the Bello Box button that appears next to your selection."
        case (false, true):
            return "Select text in any app, then press \(settings.globalHotkey.displayString) to summon the Bello Box tool board."
        case (false, false):
            return "Open Bello Box from the menu bar when you need it. You can re-enable auto hint or the shortcut in Settings anytime."
        }
    }

    private func stepHeader(_ title: String, systemImage: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: systemImage).font(.title2).foregroundStyle(BoxTheme.accent)
            Text(title).font(.system(size: 26, weight: .bold))
        }
    }

    private func bullet(_ symbol: String, _ title: String, _ detail: String) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: symbol)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(BoxTheme.accent)
                .frame(width: 26, height: 26)
                .background(Circle().fill(BoxTheme.accentSoft))
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.headline)
                Text(detail).font(.subheadline).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
        }
    }

}
