import Foundation

/// A best-effort, model-aware token estimate. This is NOT an exact tokenizer
/// (those require bundling large BPE vocab files); it is a word-aware heuristic
/// whose per-family multiplier reflects how model families differ in density.
enum TokenEstimator {
    enum Family: String {
        case openAICL100K = "cl100k_base"   // gpt-4, gpt-3.5
        case openAIO200K = "o200k_base"     // gpt-4o, o-series, gpt-4.1+
        case anthropic = "Claude"
        case generic = "generic"
    }

    static func family(model: String, provider: ProviderKind) -> Family {
        let m = model.lowercased()
        if provider == .anthropic || m.contains("claude") { return .anthropic }
        if m.contains("gpt-4o") || m.contains("gpt-4.1") || m.contains("gpt-5")
            || m.contains("o1") || m.contains("o3") || m.contains("o4") || m.contains("omni") {
            return .openAIO200K
        }
        if m.contains("gpt-4") || m.contains("gpt-3.5") || m.contains("davinci") || m.contains("text-embedding") {
            return .openAICL100K
        }
        return .generic
    }

    static func estimate(_ text: String, family: Family) -> Int {
        guard !text.isEmpty else { return 0 }

        let multiplier: Double
        switch family {
        case .openAIO200K: multiplier = 0.95
        case .openAICL100K: multiplier = 1.0
        case .anthropic: multiplier = 1.08
        case .generic: multiplier = 1.0
        }

        var count = 0.0
        let words = text.split { !$0.isLetter && !$0.isNumber }
        for word in words {
            count += max(1, (Double(word.count) / 4.0).rounded(.up))
        }
        let punctuation = text.unicodeScalars.filter {
            !CharacterSet.alphanumerics.contains($0) && !CharacterSet.whitespacesAndNewlines.contains($0)
        }.count
        count += Double(punctuation)

        return max(1, Int((count * multiplier).rounded()))
    }

    static func estimate(_ text: String, model: String, provider: ProviderKind) -> Int {
        estimate(text, family: family(model: model, provider: provider))
    }

    static func familyLabel(model: String, provider: ProviderKind) -> String {
        family(model: model, provider: provider).rawValue
    }
}
