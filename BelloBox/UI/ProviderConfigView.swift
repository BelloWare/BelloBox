import Foundation
import SwiftUI

/// Shared provider configuration UI used in Settings and onboarding: pick a
/// provider, fill its fields, load its model list, pick a model, and run a
/// "say hi" connection test. Handles OpenAI, Anthropic, and Codex app-server.
struct ProviderConfigView: View {
    @ObservedObject var settings: AppSettings

    @State private var models: [String] = []
    @State private var isLoadingModels = false
    @State private var loadError: String?
    @State private var isTesting = false
    @State private var testState: TestState = .idle
    @State private var modelLoadToken = UUID()
    @State private var testToken = UUID()

    enum TestState: Equatable {
        case idle
        case success(String)
        case failure(String)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ToolChoiceBar(selection: $settings.providerKind, choices: ProviderKind.allCases.map { ($0, $0.displayName) }, label: "API format")
            .onChange(of: settings.providerKind) { _ in
                resetTransientState(clearModels: true)
            }
            .onChange(of: settings.openAIBaseURL) { _ in resetIfActiveProvider(.openAI, clearModels: true) }
            .onChange(of: settings.anthropicBaseURL) { _ in resetIfActiveProvider(.anthropic, clearModels: true) }
            .onChange(of: settings.apiKey) { _ in resetIfActiveHTTPProvider(clearModels: true) }
            .onChange(of: settings.openAIAPIKind) { _ in resetIfActiveProvider(.openAI, clearModels: true) }
            .onChange(of: settings.codexPath) { _ in resetIfActiveProvider(.codexCLI, clearModels: false) }
            .onChange(of: settings.codexModel) { _ in resetIfActiveProvider(.codexCLI, clearModels: false) }
            .onChange(of: settings.codexReasoningEffort) { _ in resetIfActiveProvider(.codexCLI, clearModels: false) }
            .onChange(of: settings.codexApprovalPolicy) { _ in resetIfActiveProvider(.codexCLI, clearModels: false) }
            .onChange(of: settings.codexSandboxMode) { _ in resetIfActiveProvider(.codexCLI, clearModels: false) }

            if settings.providerKind == .codexCLI {
                codexFields
            } else {
                httpFields
            }
            if settings.providerKind == .openAI {
                openAIAPIKindRow
            }

            modelRow
            AIGenerationSettingsView(settings: settings)
            if settings.providerKind == .codexCLI { codexPolicyRows }
            Text(settings.providerKind.isHTTP
                 ? "Used by Ask AI, World Clock copilot, and AI screenshot text recognition."
                 : "Used by Ask AI and World Clock copilot.")
                .font(.system(size: 11)).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            testRow

            Text(hint)
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .buttonStyle(SecondaryButtonStyle())
        .accentColor(BoxTheme.accentFill)
        .onChange(of: settings.currentConfig) { _ in
            testToken = UUID()
            isTesting = false
            testState = .idle
        }
    }

    // MARK: - Fields

    private var httpFields: some View {
        VStack(alignment: .leading, spacing: 8) {
            labeledField("Endpoint") {
                HStack {
                    TextField("Base URL", text: endpointBinding)
                        .textFieldStyle(ToolTextFieldStyle())
                        .autocorrectionDisabled()
                    Button("Default") { resetEndpoint() }
                }
            }
            labeledField("API key") {
                SecureField(apiKeyPlaceholder, text: $settings.apiKey)
                    .textFieldStyle(ToolTextFieldStyle())
            }
        }
    }

    private var codexFields: some View {
        labeledField("Codex command (optional)") {
            HStack {
                TextField("codex (from your shell PATH)", text: $settings.codexPath)
                    .textFieldStyle(ToolTextFieldStyle())
                    .autocorrectionDisabled()
                Button("Detect") { Task { await settings.detectCodexPath() } }
                    .help("Fill in the full path to your codex binary")
            }
        }
    }

    private var openAIAPIKindRow: some View {
        labeledField("Request API") {
            ToolChoiceBar(selection: $settings.openAIAPIKind, choices: OpenAIAPIKind.allCases.map { ($0, $0.fullLabel) }, label: "Request API")
        }
    }

    private var modelRow: some View {
        VStack(alignment: .leading, spacing: 6) {
            labeledField("Model") {
                HStack(spacing: 6) {
                    TextField(modelPlaceholder, text: modelBinding)
                        .textFieldStyle(ToolTextFieldStyle())
                        .autocorrectionDisabled()
                    Menu {
                        if !models.isEmpty {
                            ForEach(models, id: \.self) { name in
                                Button(name) { setModel(name) }
                          }
                      } else {
                            ForEach(fallbackModels, id: \.self) { name in
                                Button(name) { setModel(name) }
                          }
                      }
                  } label: {
                        Image(systemName: "chevron.down.circle.fill").foregroundStyle(BoxTheme.accent)
                  }
                    .menuStyle(.borderlessButton)
                    .fixedSize()
                    if settings.providerKind.isHTTP {
                        Button {
                            loadModels()
                      } label: {
                            if isLoadingModels { ProgressView().controlSize(.small) } else { Text("Load") }
                      }
                        .disabled(isLoadingModels || modelLoadRequiresAPIKey)
                        .help("Fetch the available models from the endpoint")
                  }
              }
            }
            if let loadError {
                Label(loadError, systemImage: "exclamationmark.circle")
                    .font(.system(size: 11)).foregroundStyle(BoxTheme.danger)
                    .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
            }
        }
    }

    private var codexPolicyRows: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top, spacing: 10) {
                labeledField("Sandbox") {
                    Picker("Sandbox", selection: $settings.codexSandboxMode) {
                        ForEach(CodexCLI.sandboxModes) { mode in
                            Text(mode.label).tag(mode)
                      }
                  }
                    .labelsHidden().pickerStyle(.menu)
                    .frame(maxWidth: 190, alignment: .leading)
              }

                labeledField("Approvals") {
                    Picker("Approvals", selection: $settings.codexApprovalPolicy) {
                        ForEach(CodexCLI.approvalPolicies) { policy in
                            Text(policy.label).tag(policy)
                      }
                  }
                    .labelsHidden().pickerStyle(.menu)
                    .frame(maxWidth: 190, alignment: .leading)
              }
            }

            Text(codexPolicyHelp)
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var testRow: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Button { runTest() } label: {
                    if isTesting {
                        HStack(spacing: 6) { ProgressView().controlSize(.small); Text("Testing model…") }
                  } else {
                        Label("Test connection", systemImage: "arrow.up.right.circle")
                  }
              }
                .disabled(isTesting || !settings.isConfigured)
                Text("Sends a short hello with these settings.")
                    .font(.system(size: 11)).foregroundStyle(.secondary)
                Spacer(minLength: 0)
            }
            switch testState {
            case .idle: EmptyView()
            case let .success(message):
                Label(message, systemImage: "checkmark.circle.fill")
                    .foregroundStyle(BoxTheme.success).font(.caption)
                    .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
            case let .failure(message):
                // API compatibility errors often name the rejected parameter.
                // Keep that explanation readable instead of clipping it to two lines.
                Label(message, systemImage: "exclamationmark.circle.fill")
                    .foregroundStyle(BoxTheme.danger).font(.caption)
                    .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
            }
        }
    }

    // MARK: - Bindings & data

    private var endpointBinding: Binding<String> {
        switch settings.providerKind {
        case .openAI: return $settings.openAIBaseURL
        case .anthropic: return $settings.anthropicBaseURL
        case .codexCLI: return $settings.codexPath
        }
    }

    private var modelBinding: Binding<String> {
        switch settings.providerKind {
        case .openAI: return $settings.openAIModel
        case .anthropic: return $settings.anthropicModel
        case .codexCLI: return $settings.codexModel
        }
    }

    private var modelPlaceholder: String {
        settings.providerKind == .codexCLI ? CodexCLI.defaultModel : "Model name"
    }

    private var fallbackModels: [String] {
        switch settings.providerKind {
        case .openAI: return ["gpt-4o-mini", "gpt-4o", "gpt-4.1-mini", "gpt-4.1", "o3-mini", "gpt-3.5-turbo"]
        case .anthropic: return ["claude-3-5-haiku-latest", "claude-3-5-sonnet-latest", "claude-3-7-sonnet-latest", "claude-3-opus-latest"]
        case .codexCLI: return CodexCLI.presetModels
        }
    }

    private var apiKeyPlaceholder: String {
        settings.providerKind == .openAI ? "Optional for local endpoints" : "Paste your key"
    }

    private var modelLoadRequiresAPIKey: Bool {
        settings.providerKind == .anthropic && settings.apiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var hint: String {
        switch settings.providerKind {
        case .openAI:
            switch settings.openAIAPIKind {
            case .chatCompletions:
                return "POST {endpoint}/chat/completions with a Bearer token. Works with OpenAI, OpenRouter, Groq, Ollama, LM Studio, and other compatible servers. Use Load to fetch models."
            case .responses:
                return "POST {endpoint}/responses with a Bearer token and Responses API streaming. Use this for OpenAI or compatible endpoints that implement the Responses API."
            }
        case .anthropic:
            return "POST {endpoint}/messages with an x-api-key header. Use Load to fetch models from /models."
        case .codexCLI:
            return "Runs `codex app-server` through your login shell and uses your existing Codex login. Bello Box passes the selected model, reasoning effort, sandbox, and approval policy on each request."
        }
    }

    private var codexPolicyHelp: String {
        let sandboxHelp: String
        switch settings.codexSandboxMode {
        case .readOnly:
            sandboxHelp = "Read only is safest for text actions. Codex can read context but cannot write files."
        case .workspaceWrite:
            sandboxHelp = "Workspace write allows Codex to write only inside Bello Box's temporary action folder."
        case .dangerFullAccess:
            sandboxHelp = "Full access lets Codex run without filesystem sandboxing. Use only with trusted prompts."
        }
        guard settings.codexApprovalPolicy != .never else { return sandboxHelp }
        return "\(sandboxHelp) Bello Box does not show interactive Codex approval prompts; set approvals to Never for text actions."
    }

    // MARK: - Actions

    private func resetEndpoint() {
        switch settings.providerKind {
        case .openAI: settings.openAIBaseURL = ProviderKind.openAI.defaultBaseURL
        case .anthropic: settings.anthropicBaseURL = ProviderKind.anthropic.defaultBaseURL
        case .codexCLI: break
        }
    }

    private func setModel(_ name: String) {
        modelBinding.wrappedValue = name
    }

    private func loadModels() {
        let token = UUID()
        modelLoadToken = token
        isLoadingModels = true
        loadError = nil
        let config = settings.currentConfig
        Task { @MainActor in
            do {
                let list = try await AIClient().listModels(config: config)
                guard finishModelLoadIfCurrent(token: token) else { return }
                models = list
                if list.isEmpty { loadError = "No models returned." }
            } catch {
                guard finishModelLoadIfCurrent(token: token) else { return }
                loadError = (error as? AIError)?.errorDescription ?? error.localizedDescription
            }
            isLoadingModels = false
        }
    }

    private func runTest() {
        let token = UUID()
        testToken = token
        isTesting = true
        testState = .idle
        let config = settings.currentConfig
        Task { @MainActor in
            do {
                let reply = try await AIClient().complete(config: config, userText: "Reply with a short, friendly hello.")
                guard finishTestIfCurrent(token: token, config: config) else { return }
                let trimmed = reply.trimmingCharacters(in: .whitespacesAndNewlines)
                testState = .success(trimmed.isEmpty ? "Connected" : String(trimmed.prefix(60)))
            } catch {
                guard finishTestIfCurrent(token: token, config: config) else { return }
                testState = .failure((error as? AIError)?.errorDescription ?? error.localizedDescription)
            }
            isTesting = false
        }
    }

    private func resetTransientState(clearModels: Bool) {
        modelLoadToken = UUID()
        testToken = UUID()
        if clearModels { models = [] }
        loadError = nil
        isLoadingModels = false
        isTesting = false
        testState = .idle
    }

    private func resetIfActiveProvider(_ provider: ProviderKind, clearModels: Bool) {
        guard settings.providerKind == provider else { return }
        resetTransientState(clearModels: clearModels)
    }

    private func resetIfActiveHTTPProvider(clearModels: Bool) {
        guard settings.providerKind.isHTTP else { return }
        resetTransientState(clearModels: clearModels)
    }

    private func finishModelLoadIfCurrent(token: UUID) -> Bool {
        modelLoadToken == token
    }

    private func finishTestIfCurrent(token: UUID, config: AIConfig) -> Bool {
        guard testToken == token else { return false }
        guard settings.currentConfig == config else {
            isTesting = false
            return false
        }
        return true
    }

    @ViewBuilder
    private func labeledField<Content: View>(_ label: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label).font(.caption2.bold()).foregroundStyle(.secondary)
            content()
        }
    }
}
