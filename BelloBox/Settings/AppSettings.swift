import Foundation
import Combine

enum AppearancePreference: String, CaseIterable, Identifiable {
    case system
    case light
    case dark

    var id: String { rawValue }
    var label: String {
        switch self {
        case .system: return "System"
        case .light: return "Light"
        case .dark: return "Dark"
        }
    }
    var symbol: String {
        switch self {
        case .system: return "circle.lefthalf.filled"
        case .light: return "sun.max"
        case .dark: return "moon"
        }
    }
}

/// User-facing configuration, persisted to UserDefaults (API keys go to the
/// Keychain). Per-provider base URL / model are kept independently so switching
/// providers does not lose the other configuration.
/// Touched only on the main thread (settings UI + app lifecycle); not annotated
/// `@MainActor` so the shared instance can be referenced from `App.init`.
final class AppSettings: ObservableObject {
    static let shared = AppSettings()

    private enum Keys {
        static let provider = "provider"
        static let openAIBase = "openAIBaseURL"
        static let anthropicBase = "anthropicBaseURL"
        static let openAIModel = "openAIModel"
        static let anthropicModel = "anthropicModel"
        static let systemPrompt = "systemPrompt"
        static let floatingButtonEnabled = "floatingButtonEnabled"
        static let hasCompletedSetup = "hasCompletedSetup"
        static let appearance = "appearance"
    }

    static let defaultSystemPrompt = """
    You are a writing assistant embedded in macOS. The user selects text in any \
    app and asks you to transform it. Unless the instruction asks a question, \
    reply with ONLY the transformed text — no preamble, no explanations, and no \
    surrounding quotation marks.
    """

    private let defaults: UserDefaults

    @Published var providerKind: ProviderKind {
        didSet {
            defaults.set(providerKind.rawValue, forKey: Keys.provider)
            apiKey = KeychainStore.get(account: KeychainStore.account(for: providerKind)) ?? ""
        }
    }

    @Published var openAIBaseURL: String { didSet { defaults.set(openAIBaseURL, forKey: Keys.openAIBase) } }
    @Published var anthropicBaseURL: String { didSet { defaults.set(anthropicBaseURL, forKey: Keys.anthropicBase) } }
    @Published var openAIModel: String { didSet { defaults.set(openAIModel, forKey: Keys.openAIModel) } }
    @Published var anthropicModel: String { didSet { defaults.set(anthropicModel, forKey: Keys.anthropicModel) } }
    @Published var systemPrompt: String { didSet { defaults.set(systemPrompt, forKey: Keys.systemPrompt) } }
    @Published var floatingButtonEnabled: Bool { didSet { defaults.set(floatingButtonEnabled, forKey: Keys.floatingButtonEnabled) } }
    @Published var appearance: AppearancePreference { didSet { defaults.set(appearance.rawValue, forKey: Keys.appearance) } }

    /// API key for the currently-selected provider. Persisted to the Keychain.
    @Published var apiKey: String {
        didSet { KeychainStore.set(apiKey, account: KeychainStore.account(for: providerKind)) }
    }

    var hasCompletedSetup: Bool {
        get { defaults.bool(forKey: Keys.hasCompletedSetup) }
        set { defaults.set(newValue, forKey: Keys.hasCompletedSetup) }
    }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        let kind = ProviderKind(rawValue: defaults.string(forKey: Keys.provider) ?? "") ?? .openAI
        providerKind = kind
        openAIBaseURL = defaults.string(forKey: Keys.openAIBase) ?? ProviderKind.openAI.defaultBaseURL
        anthropicBaseURL = defaults.string(forKey: Keys.anthropicBase) ?? ProviderKind.anthropic.defaultBaseURL
        openAIModel = defaults.string(forKey: Keys.openAIModel) ?? ProviderKind.openAI.defaultModel
        anthropicModel = defaults.string(forKey: Keys.anthropicModel) ?? ProviderKind.anthropic.defaultModel
        systemPrompt = defaults.string(forKey: Keys.systemPrompt) ?? Self.defaultSystemPrompt
        floatingButtonEnabled = (defaults.object(forKey: Keys.floatingButtonEnabled) as? Bool) ?? true
        appearance = AppearancePreference(rawValue: defaults.string(forKey: Keys.appearance) ?? "") ?? .system
        apiKey = KeychainStore.get(account: KeychainStore.account(for: kind)) ?? ""
    }

    /// The resolved configuration for the active provider.
    var currentConfig: AIConfig {
        switch providerKind {
        case .openAI:
            return AIConfig(kind: .openAI, baseURL: openAIBaseURL, model: openAIModel, apiKey: apiKey, systemPrompt: systemPrompt)
        case .anthropic:
            return AIConfig(kind: .anthropic, baseURL: anthropicBaseURL, model: anthropicModel, apiKey: apiKey, systemPrompt: systemPrompt)
        }
    }

    var isConfigured: Bool { currentConfig.isUsable }

    func resetSystemPrompt() { systemPrompt = Self.defaultSystemPrompt }

    func baseURLBinding() -> (get: () -> String, set: (String) -> Void) {
        switch providerKind {
        case .openAI: return ({ self.openAIBaseURL }, { self.openAIBaseURL = $0 })
        case .anthropic: return ({ self.anthropicBaseURL }, { self.anthropicBaseURL = $0 })
        }
    }
}
