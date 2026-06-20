import Foundation

struct AIImagePayload: Equatable {
    var data: Data
    var mimeType: String
    var width: Int
    var height: Int
    var digest: String

    var dataURL: String {
        "data:\(mimeType);base64,\(data.base64EncodedString())"
    }
}

enum AIImageResponseFormat: Equatable {
    case plainText
    case json
}

struct LLMOCRParsedResponse: Equatable {
    var plainText: String
    var markdownText: String?
    var warnings: [String]
}

final class AIImageClient {
    private let session: URLSession

    init(session: URLSession = AIClient.makeSession()) {
        self.session = session
    }

    func completeVision(
        config: AIConfig,
        image: AIImagePayload,
        prompt: String,
        responseFormat: AIImageResponseFormat
    ) async throws -> String {
#if DEBUG
        if let fixture = ProcessInfo.processInfo.environment["BELLOBOX_E2E_LLM_OCR_FIXTURE"],
           !fixture.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
           let text = try? String(contentsOfFile: fixture, encoding: .utf8) {
            return text
        }
#endif
        guard config.kind != .codexCLI else {
            throw OCRError.unsupportedProvider("Codex app-server does not support image OCR yet. Use Mac OCR or an image-capable HTTP provider.")
        }
        guard !config.apiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw AIError.missingAPIKey
        }

        let request: URLRequest
        switch config.kind {
        case .openAI:
            request = try Self.openAIVisionRequest(config: config, image: image, prompt: prompt, responseFormat: responseFormat)
        case .anthropic:
            request = try Self.anthropicVisionRequest(config: config, image: image, prompt: prompt)
        case .codexCLI:
            throw OCRError.unsupportedProvider("Codex app-server does not support image OCR yet.")
        }

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw AIError.transport(error.localizedDescription)
        }

        guard let http = response as? HTTPURLResponse else {
            throw AIError.transport("The response was not an HTTP response.")
        }
        guard (200..<300).contains(http.statusCode) else {
            throw AIError.http(status: http.statusCode, message: AIClient.extractErrorMessage(String(data: data, encoding: .utf8) ?? ""))
        }

        return try Self.extractText(from: data, config: config)
    }

    static func openAIVisionRequest(
        config: AIConfig,
        image: AIImagePayload,
        prompt: String,
        responseFormat: AIImageResponseFormat
    ) throws -> URLRequest {
        switch config.openAIAPIKind {
        case .responses:
            let url = try AIClient.endpointURL(base: config.baseURL, path: "/responses")
            var request = URLRequest(url: url)
            request.httpMethod = "POST"
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.setValue("Bearer \(config.apiKey)", forHTTPHeaderField: "Authorization")
            request.httpBody = try JSONSerialization.data(
                withJSONObject: openAIResponsesVisionBody(config: config, image: image, prompt: prompt, responseFormat: responseFormat),
                options: [.sortedKeys]
            )
            return request
        case .chatCompletions:
            let url = try AIClient.endpointURL(base: config.baseURL, path: "/chat/completions")
            var request = URLRequest(url: url)
            request.httpMethod = "POST"
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.setValue("Bearer \(config.apiKey)", forHTTPHeaderField: "Authorization")
            request.httpBody = try JSONSerialization.data(
                withJSONObject: openAIChatVisionBody(config: config, image: image, prompt: prompt, stream: false),
                options: [.sortedKeys]
            )
            return request
        }
    }

    static func openAIResponsesVisionBody(
        config: AIConfig,
        image: AIImagePayload,
        prompt: String,
        responseFormat: AIImageResponseFormat = .json
    ) -> [String: Any] {
        var body: [String: Any] = [
            "model": config.model,
            "input": [[
                "role": "user",
                "content": [
                    ["type": "input_text", "text": prompt],
                    ["type": "input_image", "image_url": image.dataURL],
                ],
            ]],
        ]
        if case .json = responseFormat {
            body["text"] = ["format": ["type": "json_object"]]
        }
        if let temperature = config.temperature {
            body["temperature"] = temperature
        }
        return body
    }

    static func openAIChatVisionBody(config: AIConfig, image: AIImagePayload, prompt: String, stream: Bool) -> [String: Any] {
        var body: [String: Any] = [
            "model": config.model,
            "stream": stream,
            "messages": [[
                "role": "user",
                "content": [
                    ["type": "text", "text": prompt],
                    ["type": "image_url", "image_url": ["url": image.dataURL]],
                ],
            ]],
        ]
        if let temperature = config.temperature {
            body["temperature"] = temperature
        }
        return body
    }

    static func anthropicVisionRequest(config: AIConfig, image: AIImagePayload, prompt: String) throws -> URLRequest {
        let url = try AIClient.endpointURL(base: config.baseURL, path: "/messages")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue(config.apiKey, forHTTPHeaderField: "x-api-key")
        request.setValue("2023-06-01", forHTTPHeaderField: "anthropic-version")
        request.httpBody = try JSONSerialization.data(
            withJSONObject: anthropicVisionBody(config: config, image: image, prompt: prompt),
            options: [.sortedKeys]
        )
        return request
    }

    static func anthropicVisionBody(config: AIConfig, image: AIImagePayload, prompt: String) -> [String: Any] {
        var body: [String: Any] = [
            "model": config.model,
            "max_tokens": config.maxTokens,
            "messages": [[
                "role": "user",
                "content": [
                    [
                        "type": "image",
                        "source": [
                            "type": "base64",
                            "media_type": image.mimeType,
                            "data": image.data.base64EncodedString(),
                        ],
                    ],
                    ["type": "text", "text": prompt],
                ],
            ]],
        ]
        if let temperature = config.temperature {
            body["temperature"] = temperature
        }
        return body
    }

    static func parseLLMOCRResponse(_ text: String) throws -> LLMOCRParsedResponse {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { throw OCRError.providerReturnedEmptyText }

        if let data = trimmed.data(using: .utf8),
           let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            let plain = (json["plainText"] as? String) ?? (json["text"] as? String) ?? ""
            let markdown = json["markdownText"] as? String
            let warnings = json["warnings"] as? [String] ?? []
            let finalPlain = plain.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? (markdown ?? "") : plain
            guard !finalPlain.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                throw OCRError.providerReturnedEmptyText
            }
            return LLMOCRParsedResponse(plainText: finalPlain, markdownText: markdown, warnings: warnings)
        }

        let cleaned = stripCodeFence(trimmed)
        if cleaned != trimmed { return try parseLLMOCRResponse(cleaned) }

        return LLMOCRParsedResponse(
            plainText: trimmed,
            markdownText: nil,
            warnings: ["Provider returned non-JSON OCR text."]
        )
    }

    private static func stripCodeFence(_ text: String) -> String {
        guard text.hasPrefix("```") else { return text }
        var lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        if !lines.isEmpty { lines.removeFirst() }
        if lines.last?.trimmingCharacters(in: .whitespacesAndNewlines) == "```" { lines.removeLast() }
        return lines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static func extractText(from data: Data, config: AIConfig) throws -> String {
        guard
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            let text = String(data: data, encoding: .utf8) ?? ""
            if text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { throw OCRError.providerReturnedEmptyText }
            return text
        }

        switch config.kind {
        case .openAI:
            if let text = object["output_text"] as? String { return text }
            if let choices = object["choices"] as? [[String: Any]],
               let message = choices.first?["message"] as? [String: Any],
               let content = message["content"] as? String {
                return content
            }
            if let output = object["output"] as? [[String: Any]] {
                let parts = output.flatMap { item -> [String] in
                    guard let content = item["content"] as? [[String: Any]] else { return [] }
                    return content.compactMap { ($0["text"] as? String) ?? ($0["content"] as? String) }
                }
                if !parts.isEmpty { return parts.joined(separator: "\n") }
            }
        case .anthropic:
            if let content = object["content"] as? [[String: Any]] {
                let parts = content.compactMap { $0["text"] as? String }
                if !parts.isEmpty { return parts.joined(separator: "\n") }
            }
        case .codexCLI:
            break
        }

        throw OCRError.providerReturnedEmptyText
    }
}
