import AppKit
import Foundation
import UniformTypeIdentifiers

enum OCRResultFormatter {
    static func plainText(from regions: [OCRTextRegion]) -> String {
        let sorted = regions.sortedByReadingOrder()
        guard !sorted.isEmpty else { return "" }

        let heights = sorted.compactMap { region -> CGFloat? in
            guard let rect = region.boundingBox?.rect, rect.height > 0 else { return nil }
            return rect.height
        }
        let medianHeight = heights.sorted().dropFirst(heights.count / 2).first ?? 14
        var lines: [String] = []
        var previousRect: CGRect?

        for region in sorted {
            let text = region.text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !text.isEmpty else { continue }
            if let previousRect, let rect = region.boundingBox?.rect {
                let gap = rect.minY - previousRect.maxY
                if gap > medianHeight * 1.35 {
                    lines.append("")
                }
            }
            lines.append(text)
            previousRect = region.boundingBox?.rect ?? previousRect
        }
        return lines.joined(separator: "\n")
    }

    static func plainText(from result: OCRResult) -> String {
        if !result.plainText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return result.plainText
        }
        return plainText(from: result.regions)
    }

    static func markdown(from result: OCRResult) -> String {
        if let markdown = result.markdownText, !markdown.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return markdown
        }
        return plainText(from: result)
    }

    static func copyPlainText(_ result: OCRResult) throws {
        let text = plainText(from: result)
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        guard pasteboard.setString(text, forType: .string) else {
            throw OCRError.failed("Could not copy OCR text to the pasteboard.")
        }
    }

    static func copyMarkdown(_ result: OCRResult) throws {
        let text = markdown(from: result)
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        guard pasteboard.setString(text, forType: .string) else {
            throw OCRError.failed("Could not copy OCR Markdown to the pasteboard.")
        }
    }

    @MainActor
    static func saveText(_ result: OCRResult, suggestedName: String) throws {
        try save(plainText(from: result), suggestedName: suggestedName, extension: "txt")
    }

    @MainActor
    static func saveMarkdown(_ result: OCRResult, suggestedName: String) throws {
        try save(markdown(from: result), suggestedName: suggestedName, extension: "md")
    }

    @MainActor
    private static func save(_ text: String, suggestedName: String, extension ext: String) throws {
        let panel = NSSavePanel()
        panel.allowedContentTypes = ext == "md" ? [UTType(filenameExtension: "md") ?? .plainText] : [.plainText]
        panel.canCreateDirectories = true
        panel.nameFieldStringValue = suggestedName.hasSuffix(".\(ext)") ? suggestedName : "\(suggestedName).\(ext)"
        guard panel.runModal() == .OK, let url = panel.url else { throw OCRError.cancelled }
        try text.data(using: .utf8)?.write(to: url, options: .atomic)
    }
}
