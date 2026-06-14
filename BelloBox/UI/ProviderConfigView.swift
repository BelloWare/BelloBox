import SwiftUI

/// Shared provider configuration UI used in Settings and onboarding: pick a
/// provider, fill its fields, load its model list, pick a model, and run a
/// "say hi" connection test. Handles OpenAI, Anthropic, and the Codex CLI.
struct ProviderConfigView: View {
    @ObservedObject var settings: AppSettings

    @State private var models: [String] = []
    @State private var isLoadingModels = false
    @State private var loadError: String?
    @State private var isTesting = false
    @State private var testState: TestState = .idle

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
                models = []
                loadError = nil
                testState = .idle
                autoDetectCodexIfNeeded()
            }

            if settings.providerKind == .codexCLI {
                codexFields
            } else {
                httpFields
            }

            modelRow
            testRow

            Text(hint)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .onAppear(perform: autoDetectCodexIfNeeded)
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
                SecureField("Paste your key", text: $settings.apiKey)
                    .textFieldStyle(.roundedBorder)
            }
        }
    }

    private var codexFields: some View {
        labeledField("Codex path") {
            HStack {
                TextField("/path/to/codex", text: $settings.codexPath)
                    .textFieldStyle(.roundedBorder)
                    .autocorrectionDisabled()
                Button("Detect") { Task { await settings.detectCodexPath() } }
            }
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
                    .disabled(isLoadingModels || settings.apiKey.trimmingCharacters(in: .whitespaces).isEmpty)
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
        settings.providerKind == .codexCLI ? "default (from codex config)" : "Model name"
    }

    private var fallbackModels: [String] {
        switch settings.providerKind {
        case .openAI: return ["gpt-4o-mini", "gpt-4o", "gpt-4.1-mini", "gpt-4.1", "o3-mini", "gpt-3.5-turbo"]
        case .anthropic: return ["claude-3-5-haiku-latest", "claude-3-5-sonnet-latest", "claude-3-7-sonnet-latest", "claude-3-opus-latest"]
        case .codexCLI: return CodexCLI.presetModels
        }
    }

    private var hint: String {
        switch settings.providerKind {
        case .openAI:
            return "POST {endpoint}/chat/completions with a Bearer token. Works with OpenAI, OpenRouter, Groq, Ollama, LM Studio, and other compatible servers. Use Load to fetch models."
        case .anthropic:
            return "POST {endpoint}/messages with an x-api-key header. Use Load to fetch models from /models."
        case .codexCLI:
            return "Runs the local `codex exec` CLI using your existing Codex login — no API key needed. Leave the model blank to use your codex config default."
        }
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

    private func autoDetectCodexIfNeeded() {
        guard settings.providerKind == .codexCLI,
              settings.codexPath.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        Task { await settings.detectCodexPath() }
    }

    private func loadModels() {
        isLoadingModels = true
        loadError = nil
        let config = settings.currentConfig
        Task {
            do {
                let list = try await AIClient().listModels(config: config)
                models = list
                if list.isEmpty { loadError = "No models returned." }
            } catch {
                loadError = (error as? AIError)?.errorDescription ?? error.localizedDescription
            }
            isLoadingModels = false
        }
    }

    private func runTest() {
        isTesting = true
        testState = .idle
        let config = settings.currentConfig
        Task {
            do {
                let reply = try await AIClient().complete(config: config, userText: "Reply with a short, friendly hello.")
                let trimmed = reply.trimmingCharacters(in: .whitespacesAndNewlines)
                testState = .success(trimmed.isEmpty ? "Connected" : String(trimmed.prefix(60)))
            } catch {
                testState = .failure((error as? AIError)?.errorDescription ?? error.localizedDescription)
            }
            isTesting = false
        }
    }

    @ViewBuilder
    private func labeledField<Content: View>(_ label: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label).font(.caption2.bold()).foregroundStyle(.secondary)
            content()
        }
    }
}
