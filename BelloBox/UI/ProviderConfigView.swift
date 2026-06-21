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
            Picker("API format", selection: $settings.providerKind) {
                ForEach(ProviderKind.allCases) { kind in
                    Text(kind.displayName).tag(kind)
                }
            }
            .pickerStyle(.segmented)
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
            if settings.providerKind.isHTTP {
                temperatureRow
            }
            if settings.providerKind == .codexCLI {
                codexReasoningRow
                codexPolicyRows
            }
            testRow

            Text(hint)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    // MARK: - Fields

    private var httpFields: some View {
        VStack(alignment: .leading, spacing: 8) {
            labeledField("Endpoint") {
                HStack {
                    TextField("Base URL", text: endpointBinding)
                        .textFieldStyle(.roundedBorder)
                        .autocorrectionDisabled()
                    Button("Default") { resetEndpoint() }
                }
            }
            labeledField("API key") {
                SecureField(apiKeyPlaceholder, text: $settings.apiKey)
                    .textFieldStyle(.roundedBorder)
            }
        }
    }

    private var codexFields: some View {
        labeledField("Codex command (optional)") {
            HStack {
                TextField("codex (from your shell PATH)", text: $settings.codexPath)
                    .textFieldStyle(.roundedBorder)
                    .autocorrectionDisabled()
                Button("Detect") { Task { await settings.detectCodexPath() } }
                    .help("Fill in the full path to your codex binary")
            }
        }
    }

    private var openAIAPIKindRow: some View {
        labeledField("Request API") {
            Picker("Request API", selection: $settings.openAIAPIKind) {
                ForEach(OpenAIAPIKind.allCases) { kind in
                    Text(kind.fullLabel).tag(kind)
                }
            }
            .pickerStyle(.segmented)
        }
    }

    private var modelRow: some View {
        labeledField("Model") {
            HStack(spacing: 6) {
                TextField(modelPlaceholder, text: modelBinding)
                    .textFieldStyle(.roundedBorder)
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
        .overlay(alignment: .bottomLeading) {
            if let loadError {
                Text(loadError).font(.caption2).foregroundStyle(.red).lineLimit(1).offset(y: 16)
            }
        }
    }

    private var codexReasoningRow: some View {
        labeledField("Reasoning") {
            Picker("Reasoning", selection: $settings.codexReasoningEffort) {
                ForEach(CodexCLI.reasoningEfforts, id: \.self) { effort in
                    Text(effortLabel(effort)).tag(effort)
                }
            }
            .pickerStyle(.segmented)
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
                    .pickerStyle(.menu)
                    .frame(maxWidth: 190, alignment: .leading)
                }

                labeledField("Approvals") {
                    Picker("Approvals", selection: $settings.codexApprovalPolicy) {
                        ForEach(CodexCLI.approvalPolicies) { policy in
                            Text(policy.label).tag(policy)
                        }
                    }
                    .pickerStyle(.menu)
                    .frame(maxWidth: 190, alignment: .leading)
                }
            }

            Text(codexPolicyHelp)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var temperatureRow: some View {
        labeledField("Temperature") {
            VStack(alignment: .leading, spacing: 7) {
                Picker("Temperature", selection: $settings.temperatureMode) {
                    ForEach(TemperatureMode.allCases) { mode in
                        Text(mode.label).tag(mode)
                    }
                }
                .pickerStyle(.segmented)
                .frame(maxWidth: 220)

                if settings.temperatureMode == .custom {
                    HStack(spacing: 10) {
                        Slider(value: temperatureBinding, in: 0 ... temperatureMaximum, step: 0.1)
                        Stepper(value: temperatureBinding, in: 0 ... temperatureMaximum, step: 0.1) {
                            Text(temperatureText)
                                .font(.system(.caption, design: .monospaced).weight(.semibold))
                                .frame(width: 34, alignment: .trailing)
                        }
                        .fixedSize()
                    }
                }

                Text(temperatureHelp)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private var testRow: some View {
        HStack(spacing: 10) {
            Button {
                runTest()
            } label: {
                if isTesting {
                    HStack(spacing: 6) { ProgressView().controlSize(.small); Text("Saying hi…") }
                } else {
                    Text("Test connection")
                }
            }
            .disabled(isTesting || !settings.isConfigured)

            switch testState {
            case .idle:
                EmptyView()
            case let .success(message):
                Label(message, systemImage: "checkmark.circle.fill")
                    .foregroundStyle(.green).font(.caption).lineLimit(2)
            case let .failure(message):
                Label(message, systemImage: "xmark.octagon.fill")
                    .foregroundStyle(.red).font(.caption).lineLimit(2)
            }
            Spacer()
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

    private var temperatureMaximum: Double {
        settings.providerKind == .anthropic ? 1.0 : 2.0
    }

    private var apiKeyPlaceholder: String {
        settings.providerKind == .openAI ? "Optional for local endpoints" : "Paste your key"
    }

    private var modelLoadRequiresAPIKey: Bool {
        settings.providerKind == .anthropic && settings.apiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var temperatureBinding: Binding<Double> {
        Binding(
            get: { min(settings.temperature, temperatureMaximum) },
            set: { settings.setTemperature(min($0, temperatureMaximum)) }
        )
    }

    private var temperatureText: String {
        String(format: "%.1f", min(settings.temperature, temperatureMaximum))
    }

    private var temperatureHelp: String {
        switch settings.temperatureMode {
        case .providerDefault:
            return "Temperature is omitted so models that require their default sampling can run."
        case .custom:
            return "Temperature is sent with each request. Use 1.0 for endpoints that require the default value."
        }
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

    private func effortLabel(_ effort: String) -> String {
        switch effort {
        case "xhigh": return "XHigh"
        default: return effort.capitalized
        }
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
