import SwiftUI

/// The Settings window: configure the AI provider, endpoint, key, model, prompt,
/// and the selection-button behavior.
struct SettingsView: View {
    @ObservedObject var settings: AppSettings

    @State private var isTesting = false
    @State private var testResult: TestResult?
    @State private var accessibilityTrusted = AccessibilityService.isTrusted

    private let permissionTimer = Timer.publish(every: 1.5, on: .main, in: .common).autoconnect()

    enum TestResult: Equatable {
        case success(String)
        case failure(String)
    }

    var body: some View {
        Form {
            providerSection
            promptSection
            behaviorSection
            permissionSection
        }
        .formStyle(.grouped)
        .frame(width: 520, height: 600)
        .onReceive(permissionTimer) { _ in
            accessibilityTrusted = AccessibilityService.isTrusted
        }
    }

    // MARK: - Provider

    private var providerSection: some View {
        Section("AI Provider") {
            Picker("API format", selection: $settings.providerKind) {
                ForEach(ProviderKind.allCases) { kind in
                    Text(kind.displayName).tag(kind)
                }
            }
            .pickerStyle(.segmented)

            LabeledContent("Endpoint") {
                HStack {
                    TextField("Base URL", text: baseURLBinding)
                        .textFieldStyle(.roundedBorder)
                        .autocorrectionDisabled()
                    Button("Default") { resetBaseURL() }
                        .help("Reset to the provider's default endpoint")
                }
            }

            LabeledContent("Model") {
                TextField("Model name", text: modelBinding)
                    .textFieldStyle(.roundedBorder)
                    .autocorrectionDisabled()
            }

            LabeledContent("API key") {
                SecureField("Paste your key", text: $settings.apiKey)
                    .textFieldStyle(.roundedBorder)
            }

            HStack {
                Button {
                    runConnectionTest()
                } label: {
                    if isTesting {
                        ProgressView().controlSize(.small)
                    } else {
                        Text("Test connection")
                    }
                }
                .disabled(isTesting || !settings.isConfigured)

                if let testResult {
                    switch testResult {
                    case let .success(message):
                        Label(message, systemImage: "checkmark.circle.fill")
                            .foregroundStyle(.green)
                            .font(.caption)
                    case let .failure(message):
                        Label(message, systemImage: "xmark.octagon.fill")
                            .foregroundStyle(.red)
                            .font(.caption)
                            .lineLimit(2)
                    }
                }
                Spacer()
            }

            Text(endpointHint)
                .font(.caption2)
                .foregroundStyle(.secondary)
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

    private var behaviorSection: some View {
        Section("Behavior") {
            Toggle("Show the floating button when I select text", isOn: $settings.floatingButtonEnabled)
            Text("You can also summon BelloBox on the current selection with ⌃⌥⌘B.")
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
                        ? "Granted — BelloBox can read your selection and paste replacements."
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

    // MARK: - Bindings

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

    private var endpointHint: String {
        switch settings.providerKind {
        case .openAI:
            return "Sends POST {endpoint}/chat/completions with a Bearer token. Works with OpenAI, OpenRouter, Groq, Ollama, LM Studio, and other OpenAI-compatible servers."
        case .anthropic:
            return "Sends POST {endpoint}/messages with an x-api-key header. Works with the Anthropic API and compatible gateways."
        }
    }

    // MARK: - Actions

    private func resetBaseURL() {
        switch settings.providerKind {
        case .openAI: settings.openAIBaseURL = ProviderKind.openAI.defaultBaseURL
        case .anthropic: settings.anthropicBaseURL = ProviderKind.anthropic.defaultBaseURL
        }
    }

    private func runConnectionTest() {
        isTesting = true
        testResult = nil
        let config = settings.currentConfig
        Task {
            let client = AIClient()
            do {
                let reply = try await client.complete(
                    config: config,
                    userText: "Reply with the single word: OK"
                )
                let trimmed = reply.trimmingCharacters(in: .whitespacesAndNewlines)
                testResult = .success(trimmed.isEmpty ? "Connected" : "Connected: \(String(trimmed.prefix(40)))")
            } catch {
                let message = (error as? AIError)?.errorDescription ?? error.localizedDescription
                testResult = .failure(message)
            }
            isTesting = false
        }
    }
}
