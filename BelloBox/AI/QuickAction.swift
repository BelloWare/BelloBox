import Foundation

/// A one-click transformation the user can apply to the selected text.
struct QuickAction: Identifiable, Hashable {
    let id: String
    let title: String
    let symbol: String
    let instruction: String
    /// Whether the result is meant to replace the original selection (true) or
    /// is informational like an explanation or answer (false).
    let replacesSelection: Bool

    static let library: [QuickAction] = [
        QuickAction(
            id: "fix",
            title: "Fix Spelling & Grammar",
            symbol: "checkmark.circle",
            instruction: "Fix spelling, grammar, and punctuation mistakes. Preserve the original meaning, tone, and formatting. Return only the corrected text.",
            replacesSelection: true
        ),
        QuickAction(
            id: "improve",
            title: "Improve Writing",
            symbol: "wand.and.stars",
            instruction: "Rewrite the text so it reads more clearly and naturally while preserving its meaning and tone. Return only the rewritten text.",
            replacesSelection: true
        ),
        QuickAction(
            id: "shorter",
            title: "Make Shorter",
            symbol: "arrow.down.right.and.arrow.up.left",
            instruction: "Make the text more concise without losing essential meaning. Return only the shortened text.",
            replacesSelection: true
        ),
        QuickAction(
            id: "professional",
            title: "Professional Tone",
            symbol: "briefcase",
            instruction: "Rewrite the text in a clear, professional tone suitable for work communication. Return only the rewritten text.",
            replacesSelection: true
        ),
        QuickAction(
            id: "friendly",
            title: "Friendly Tone",
            symbol: "face.smiling",
            instruction: "Rewrite the text in a warm, friendly, approachable tone. Return only the rewritten text.",
            replacesSelection: true
        ),
        QuickAction(
            id: "summarize",
            title: "Summarize",
            symbol: "text.append",
            instruction: "Summarize the key points of the text in a few sentences. Return only the summary.",
            replacesSelection: false
        ),
        QuickAction(
            id: "explain",
            title: "Explain",
            symbol: "questionmark.circle",
            instruction: "Explain what the text means in plain language. Return only the explanation.",
            replacesSelection: false
        ),
        QuickAction(
            id: "translate-en",
            title: "Translate to English",
            symbol: "globe",
            instruction: "Translate the text into natural English. If it is already English, leave it unchanged. Return only the translation.",
            replacesSelection: true
        ),
    ]

    /// Builds the user-message payload combining an instruction with the text.
    static func userMessage(instruction: String, selectedText: String) -> String {
        """
        \(instruction)

        Text:
        \"\"\"
        \(selectedText)
        \"\"\"
        """
    }
}
