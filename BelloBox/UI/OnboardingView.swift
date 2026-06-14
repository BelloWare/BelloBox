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

    private let stepCount = 4
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
        case 2: providerStep
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
                bullet("cursorarrow.rays", "Select text anywhere", "A small Bello Box toolbar appears next to your selection.")
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
            Text("Select text in any app, then click the Bello Box button that appears — or press ⌃⌥⌘B to summon it on the current selection.")
                .font(.title3)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            VStack(alignment: .leading, spacing: 12) {
                bullet("menubar.arrow.up.rectangle", "Find Bello Box in the menu bar", "The ✨ icon opens Settings and this guide anytime.")
                bullet("keyboard", "Summon with a hotkey", "⌃⌥⌘B runs Bello Box on whatever you have selected.")
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
