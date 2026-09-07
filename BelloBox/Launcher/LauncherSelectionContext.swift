import Foundation

/// Prepare display metadata once. Search never scans the complete selection.
struct LauncherSelectionContext {
    let hasText: Bool
    let exceedsLimit: Bool
    /// Small enough for a row preview to parse or edit it (64 KB). Larger
    /// selections show a notice and open complete in the full tool.
    let fitsPreviewLimit: Bool
    let characterCount: Int
    let preview: String
    let suggestions: [LauncherCommand]
    /// Rejected selections never teach a preference for the empty/text bucket.
    var contentKind: LauncherContentKind? {
        exceedsLimit ? nil : LauncherContentKind(hasText: hasText, suggestions: suggestions)
    }

    init(text: String) {
        hasText = !text.isEmpty
        exceedsLimit = text.utf8.prefix(UtilityLimits.inputBytes + 1).count > UtilityLimits.inputBytes
        fitsPreviewLimit = !exceedsLimit && text.utf8.prefix(LauncherPreview.parsingByteLimit + 1).count <= LauncherPreview.parsingByteLimit
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
