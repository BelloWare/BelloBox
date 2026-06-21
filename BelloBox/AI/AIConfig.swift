import Foundation

/// Which wire protocol the configured endpoint speaks.
enum ProviderKind: String, CaseIterable, Codable, Identifiable {
    case openAI
    case anthropic
    case codexCLI

    var id: String { rawValue }

    /// Whether this provider talks over HTTP (vs. a local CLI).
    var isHTTP: Bool { self != .codexCLI }

    var displayName: String {
        switch self {
        case .openAI: return "OpenAI-compatible"
        case .anthropic: return "Anthropic-compatible"
        case .codexCLI: return "Codex app-server"
        }
    }

    /// Short label for compact pickers.
    var shortName: String {
        switch self {
        case .openAI: return "OpenAI"
        case .anthropic: return "Anthropic"
        case .codexCLI: return "Codex"
        }
    }

    var defaultBaseURL: String {
        switch self {
        case .openAI: return "https://api.openai.com/v1"
        case .anthropic: return "https://api.anthropic.com/v1"
        case .codexCLI: return ""
        }
    }

    var defaultModel: String {
        switch self {
        case .openAI: return "gpt-4o-mini"
        case .anthropic: return "claude-3-5-haiku-latest"
        case .codexCLI: return ""
        }
    }
}

enum OpenAIAPIKind: String, CaseIterable, Codable, Identifiable {
    case chatCompletions
    case responses

    var id: String { rawValue }

    var label: String {
        switch self {
        case .chatCompletions: return "Chat"
        case .responses: return "Responses"
        }
    }

    var fullLabel: String {
        switch self {
        case .chatCompletions: return "Chat Completions"
        case .responses: return "Responses API"
        }
    }
}

enum TemperatureMode: String, CaseIterable, Codable, Identifiable {
    case providerDefault
    case custom

    var id: String { rawValue }

    var label: String {
        switch self {
        case .providerDefault: return "Default"
        case .custom: return "Custom"
        }
    }
}

enum CodexApprovalPolicy: String, CaseIterable, Codable, Identifiable {
    case never
    case onFailure = "on-failure"
    case onRequest = "on-request"
    case untrusted

    var id: String { rawValue }

    var label: String {
        switch self {
        case .never: return "Never"
        case .onFailure: return "On failure"
        case .onRequest: return "On request"
        case .untrusted: return "Untrusted"
        }
    }
}

enum CodexSandboxMode: String, CaseIterable, Codable, Identifiable {
    case readOnly = "read-only"
    case workspaceWrite = "workspace-write"
    case dangerFullAccess = "danger-full-access"

    var id: String { rawValue }

    var label: String {
        switch self {
        case .readOnly: return "Read only"
        case .workspaceWrite: return "Workspace write"
        case .dangerFullAccess: return "Full access"
        }
    }
}

/// A fully-resolved provider configuration used to issue one request.
struct AIConfig: Equatable {
    var kind: ProviderKind
    var baseURL: String
    var model: String
    var apiKey: String
    var systemPrompt: String
    var maxTokens: Int = 2048
    var codexReasoningEffort: String = "medium"
    var codexApprovalPolicy: CodexApprovalPolicy = .never
    var codexSandboxMode: CodexSandboxMode = .readOnly
    var openAIAPIKind: OpenAIAPIKind = .chatCompletions
    var temperature: Double?

    var isUsable: Bool {
        switch kind {
        case .openAI:
            return !model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                && AIConfig.isHTTPEndpoint(baseURL)
        case .anthropic:
            return !apiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                && !model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                && AIConfig.isHTTPEndpoint(baseURL)
        case .codexCLI:
            // Codex resolves `codex` from the user's shell, so it is always
            // attemptable; the connection test reports if it isn't installed.
            return true
        }
    }

    static func isHTTPEndpoint(_ value: String) -> Bool {
        guard let components = URLComponents(string: value.trimmingCharacters(in: .whitespacesAndNewlines)),
              let scheme = components.scheme?.lowercased(),
              scheme == "http" || scheme == "https",
              let host = components.host,
              !host.isEmpty
        else { return false }
        return true
    }
}

struct ChatMessage: Equatable {
    let role: String
    let content: String
}

enum AIError: LocalizedError, Equatable {
    case missingAPIKey
    case invalidEndpoint(String)
    case http(status: Int, message: String)
    case emptyResponse
    case transport(String)

    var errorDescription: String? {
        switch self {
        case .missingAPIKey:
            return "No API key is set. Open Bello Box settings and add a key for the selected provider."
        case let .invalidEndpoint(value):
            return "The endpoint \"\(value)\" is not a valid URL."
        case let .http(status, message):
            let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
            return "The provider returned HTTP \(status).\(trimmed.isEmpty ? "" : " \(trimmed)")"
        case .emptyResponse:
            return "The provider returned an empty response."
        case let .transport(message):
            return message
        }
    }
}
