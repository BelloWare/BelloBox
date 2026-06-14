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
    @State private var isTesting = false
    @State private var testState: TestState = .idle

    private let stepCount = 4
    private let poll = Timer.publish(every: 1.0, on: .main, in: .common).autoconnect()

    enum TestState: Equatable {
        case idle
        case success(String)
        case failure(String)
    }

    var body: some View {
        VStack(spacing: 0) {
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .padding(28)

            Divider()
            footer
                .padding(.horizontal, 24)
                .padding(.vertical, 14)
        }
        .frame(width: 560, height: 600)
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
            Text("Welcome to BelloBox")
                .font(.system(size: 26, weight: .bold))
            Text("BelloBox is a little toolbox for whatever text you already have in front of you — in any app.")
                .font(.title3)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            VStack(alignment: .leading, spacing: 12) {
                bullet("cursorarrow.rays", "Select text anywhere", "A small BelloBox toolbar appears next to your selection.")
                bullet("wand.and.stars", "Ask the AI", "Fix grammar, rewrite, summarize, or translate — then copy or replace in place.")
                bullet("qrcode", "Make a QR code", "Turn a link or any text into a scannable QR code you can edit on the fly.")
            }
            .padding(.top, 4)
        }
    }

    private var permissionStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            stepHeader("Grant Accessibility access", systemImage: "lock.shield")
            Text("BelloBox uses macOS Accessibility to read the text you select and paste replacements back. It only reads a selection when you ask it to. Nothing is sent anywhere except to the AI endpoint you configure.")
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
                        ? "BelloBox is ready to read selections."
                        : "Toggle BelloBox on under Privacy & Security → Accessibility.")
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
        VStack(alignment: .leading, spacing: 16) {
            stepHeader("Connect your AI", systemImage: "antenna.radiowaves.left.and.right")
            Text("BelloBox brings your own AI. Choose an API format and paste your endpoint, key, and model. You can change this anytime in Settings.")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Picker("API format", selection: $settings.providerKind) {
                ForEach(ProviderKind.allCases) { Text($0.displayName).tag($0) }
            }
            .pickerStyle(.segmented)
            .onChange(of: settings.providerKind) { _ in testState = .idle }

            Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 8) {
                GridRow {
                    Text("Endpoint").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                    TextField("Base URL", text: baseURLBinding).textFieldStyle(.roundedBorder).autocorrectionDisabled()
                }
                GridRow {
                    Text("Model").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                    TextField("Model name", text: modelBinding).textFieldStyle(.roundedBorder).autocorrectionDisabled()
                }
                GridRow {
                    Text("API key").gridColumnAlignment(.trailing).foregroundStyle(.secondary)
                    SecureField("Paste your key", text: $settings.apiKey).textFieldStyle(.roundedBorder)
                }
            }

            HStack(spacing: 10) {
                Button {
                    runTest()
                } label: {
                    if isTesting { ProgressView().controlSize(.small) } else { Text("Test connection") }
                }
                .disabled(isTesting || !settings.isConfigured)

                switch testState {
                case .idle:
                    EmptyView()
                case let .success(message):
                    Label(message, systemImage: "checkmark.circle.fill").foregroundStyle(.green).font(.caption).lineLimit(1)
                case let .failure(message):
                    Label(message, systemImage: "xmark.octagon.fill").foregroundStyle(.red).font(.caption).lineLimit(2)
                }
                Spacer()
            }
            Text("Tip: this works with OpenAI, Anthropic, OpenRouter, Groq, and local servers like Ollama or LM Studio.")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
    }

    private var doneStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            appBadge
            Text("You're all set")
                .font(.system(size: 26, weight: .bold))
            Text("Select text in any app, then click the BelloBox button that appears — or press ⌃⌥⌘B to summon it on the current selection.")
                .font(.title3)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            VStack(alignment: .leading, spacing: 12) {
                bullet("menubar.arrow.up.rectangle", "Find BelloBox in the menu bar", "The ✨ icon opens Settings and this guide anytime.")
                bullet("keyboard", "Summon with a hotkey", "⌃⌥⌘B runs BelloBox on whatever you have selected.")
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
                Button("Start Using BelloBox") { onFinish() }
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
                Image(nsImage: icon).resizable().frame(width: 72, height: 72)
            } else {
                Image(systemName: "wand.and.stars")
                    .font(.system(size: 40))
                    .foregroundStyle(BoxTheme.accent)
            }
        }
    }

    private func stepHeader(_ title: String, systemImage: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: systemImage).font(.title2).foregroundStyle(BoxTheme.accent)
            Text(title).font(.system(size: 22, weight: .bold))
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

    // MARK: - Bindings & actions

    private var baseURLBinding: Binding<String> {
        switch settings.providerKind {
        case .openAI: return $settings.openAIBaseURL
        case .anthropic: return $settings.anthropicBaseURL
        }
    }

    private var modelBinding: Binding<String> {
        switch settings.providerKind {
        case .openAI: return $settings.openAIModel
        case .anthropic: return $settings.anthropicModel
        }
    }

    private func runTest() {
        isTesting = true
        testState = .idle
        let config = settings.currentConfig
        Task {
            do {
                let reply = try await AIClient().complete(config: config, userText: "Reply with the single word: OK")
                let trimmed = reply.trimmingCharacters(in: .whitespacesAndNewlines)
                testState = .success(trimmed.isEmpty ? "Connected" : "Connected: \(String(trimmed.prefix(40)))")
            } catch {
                testState = .failure((error as? AIError)?.errorDescription ?? error.localizedDescription)
            }
            isTesting = false
        }
    }
}
