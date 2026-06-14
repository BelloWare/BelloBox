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
        case .codexCLI: return "Codex CLI"
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

/// A fully-resolved provider configuration used to issue one request.
struct AIConfig: Equatable {
    var kind: ProviderKind
    var baseURL: String
    var model: String
    var apiKey: String
    var systemPrompt: String
    var maxTokens: Int = 2048

    var isUsable: Bool {
        switch kind {
        case .openAI, .anthropic:
            return !apiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                && !model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                && URL(string: baseURL) != nil
        case .codexCLI:
            // Codex resolves `codex` from the user's shell, so it is always
            // attemptable; the connection test reports if it isn't installed.
            return true
        }
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
            return "No API key is set. Open BelloBox settings and add a key for the selected provider."
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
