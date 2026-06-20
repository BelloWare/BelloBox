enum OCRPromptTemplates {
    static func exactTranscription(localOCRHint: String?) -> String {
        base(localOCRHint: localOCRHint) + "\nReturn exact visible text. Preserve line breaks. Mark uncertain text as [unclear]."
    }

    static func layoutPreservingMarkdown(localOCRHint: String?) -> String {
        base(localOCRHint: localOCRHint) + "\nReturn JSON with plainText, markdownText, and warnings. Preserve columns, lists, and tables in Markdown when possible."
    }

    static func tableMarkdown(localOCRHint: String?) -> String {
        base(localOCRHint: localOCRHint) + "\nFocus on tables. Return JSON with plainText, markdownText, and warnings. Use Markdown tables when possible."
    }

    private static func base(localOCRHint: String?) -> String {
        var prompt = """
        Transcribe only the visible text in this screenshot. Do not infer hidden, blurred, cropped, or redacted content. Do not invent missing words.
        """
        if let hint = localOCRHint?.trimmingCharacters(in: .whitespacesAndNewlines), !hint.isEmpty {
            prompt += "\n\nLocal Mac OCR hint:\n\(hint)"
        }
        return prompt
    }
}

