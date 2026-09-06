import Foundation

/// Prepare display metadata once. Search never scans the complete selection.
struct LauncherSelectionContext {
    let hasText: Bool
    let exceedsLimit: Bool
    let characterCount: Int
    let preview: String
    let suggestions: [LauncherCommand]

    init(text: String) {
        hasText = !text.isEmpty
        exceedsLimit = text.utf8.prefix(UtilityLimits.inputBytes + 1).count > UtilityLimits.inputBytes
        characterCount = exceedsLimit ? 0 : text.count
        let sample = String(String.UnicodeScalarView(text.unicodeScalars.prefix(160)))
        preview = sample.split(whereSeparator: \.isWhitespace).joined(separator: " ")
        suggestions = exceedsLimit ? [] : LauncherCommand.suggestions(for: text)
    }

    static let limitNotice = "This selection exceeds 500 KB. Select a smaller passage or paste a smaller input below. No text was truncated."

    func usableSelection(_ original: TextSelection) -> TextSelection {
        guard exceedsLimit else { return original }
        // Never pass a partial document to a formatter, AI action, or replacement.
        return TextSelection(text: "", anchorRect: original.anchorRect, appName: original.appName, bundleID: original.bundleID, pid: nil)
    }
}
