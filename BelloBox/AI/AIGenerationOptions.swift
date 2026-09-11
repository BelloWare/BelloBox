import CryptoKit
import Foundation

/// Optional API values. "Model default" means the field is absent, not "none".
enum AIReasoningEffort: String, CaseIterable, Codable, Identifiable {
    case providerDefault, none, minimal, low, medium, high, xhigh, max

    var id: String { rawValue }
    var label: String {
        switch self {
        case .providerDefault: return "Model default"
        case .xhigh: return "Extra high"
        default: return rawValue.capitalized
        }
    }

    static func choices(for provider: ProviderKind) -> [Self] {
        switch provider {
        case .openAI: return allCases
        case .anthropic: return [.providerDefault, .low, .medium, .high, .xhigh, .max]
        case .codexCLI: return [.low, .medium, .high, .xhigh]
        }
    }
}

enum AnthropicThinkingMode: String, CaseIterable, Codable, Identifiable {
    case providerDefault, disabled, adaptive, budgeted

    var id: String { rawValue }
    var label: String {
        switch self {
        case .providerDefault: return "Model default"
        case .disabled: return "Off"
        case .adaptive: return "Adaptive"
        case .budgeted: return "Token budget"
        }
    }
    var isEnabled: Bool { self == .adaptive || self == .budgeted }
}

/// A local preference, scoped to one provider, endpoint and model. No requests,
/// credentials, prompts or model-name capability guesses are stored here.
struct AIGenerationPreferences: Codable, Equatable {
    var temperatureMode: TemperatureMode = .providerDefault
    var temperature: Double = 1
    var reasoningEffort: AIReasoningEffort = .providerDefault
    var thinkingMode: AnthropicThinkingMode = .providerDefault
    var thinkingBudget: Int = 4096
    var outputTokenLimit: Int = 2048

    init() {}

    private enum CodingKeys: CodingKey {
        case temperatureMode, temperature, reasoningEffort, thinkingMode, thinkingBudget, outputTokenLimit
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        temperatureMode = (try? c.decode(TemperatureMode.self, forKey: .temperatureMode)) ?? .providerDefault
        temperature = (try? c.decode(Double.self, forKey: .temperature)) ?? 1
        reasoningEffort = (try? c.decode(AIReasoningEffort.self, forKey: .reasoningEffort)) ?? .providerDefault
        thinkingMode = (try? c.decode(AnthropicThinkingMode.self, forKey: .thinkingMode)) ?? .providerDefault
        thinkingBudget = (try? c.decode(Int.self, forKey: .thinkingBudget)) ?? 4096
        outputTokenLimit = (try? c.decode(Int.self, forKey: .outputTokenLimit)) ?? 2048
    }

    func normalized(for provider: ProviderKind) -> Self {
        var result = self
        let upper: Double = provider == .anthropic ? 1 : 2
        result.temperature = temperature.isFinite ? (min(max(temperature, 0), upper) * 100).rounded() / 100 : 1
        if reasoningEffort != .providerDefault && !AIReasoningEffort.choices(for: provider).contains(reasoningEffort) {
            result.reasoningEffort = .providerDefault
        }
        result.thinkingBudget = min(max(thinkingBudget, 1024), 32768)
        result.outputTokenLimit = min(max(outputTokenLimit, 1024), 65536)
        return result
    }

    var anthropicOutputTokenLimit: Int {
        let options = normalized(for: .anthropic)
        switch thinkingMode {
        case .adaptive: return max(options.outputTokenLimit, 8192)
        case .budgeted: return max(options.outputTokenLimit, options.thinkingBudget + 2048)
        case .providerDefault, .disabled: return options.outputTokenLimit
        }
    }

    static func key(provider: ProviderKind, endpoint: String, model: String) -> String {
        var endpoint = endpoint.trimmingCharacters(in: .whitespacesAndNewlines)
        if provider.isHTTP, var url = URLComponents(string: endpoint) {
            url.scheme = url.scheme?.lowercased()
            url.host = url.host?.lowercased()
            while url.path.hasSuffix("/") { url.path.removeLast() }
            endpoint = url.string ?? endpoint
        }
        // Length-delimited JSON prevents ambiguous endpoint/model boundaries.
        let data = try! JSONEncoder().encode([provider.rawValue, endpoint, model.trimmingCharacters(in: .whitespacesAndNewlines)])
        return SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }
}

struct AIModelPreferenceEntry: Codable {
    var options: AIGenerationPreferences
    var modifiedAt: Date
}

/// Shared by text, copilot and confirmed image requests so their options never
/// drift. The caller names the actual wire format, not the selected UI tab.
enum AIGenerationWireFormat { case openAIChat, openAIResponses, anthropic }

extension AIConfig {
    func applyGenerationOptions(to body: inout [String: Any], format: AIGenerationWireFormat) {
        let isAnthropic = format == .anthropic
        if let temperature, temperature.isFinite, !(isAnthropic && anthropicThinking.isEnabled) {
            body["temperature"] = min(max(temperature, 0), isAnthropic ? 1 : 2)
        }

        if reasoningEffort != .providerDefault,
           AIReasoningEffort.choices(for: isAnthropic ? .anthropic : .openAI).contains(reasoningEffort) {
            switch format {
            case .openAIChat: body["reasoning_effort"] = reasoningEffort.rawValue
            case .openAIResponses: body["reasoning"] = ["effort": reasoningEffort.rawValue]
            case .anthropic: body["output_config"] = ["effort": reasoningEffort.rawValue]
            }
        }

        guard isAnthropic else { return }
        switch anthropicThinking {
        case .providerDefault: break
        case .disabled: body["thinking"] = ["type": "disabled"]
        case .adaptive:
            body["thinking"] = ["type": "adaptive"]
            body["max_tokens"] = max(maxTokens, 8192)
        case .budgeted:
            let budget = min(max(anthropicThinkingBudget, 1024), 32768)
            body["thinking"] = ["type": "enabled", "budget_tokens": budget]
            body["max_tokens"] = max(maxTokens, budget + 2048)
        }
    }
}
