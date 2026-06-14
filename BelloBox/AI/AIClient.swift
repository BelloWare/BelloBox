import Foundation

/// Issues chat requests against an OpenAI-compatible or Anthropic-compatible
/// endpoint. Streaming is preferred so results appear live; `complete` offers a
/// buffered convenience built on top of the same stream.
final class AIClient {
    private let session: URLSession

    init(session: URLSession = AIClient.makeSession()) {
        self.session = session
    }

    static func makeSession() -> URLSession {
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 60
        config.timeoutIntervalForResource = 300
        config.waitsForConnectivity = true
        return URLSession(configuration: config)
    }

    // MARK: - Public API

    /// Streams the model response, invoking `onDelta` for each new text chunk.
    /// `onDelta` is called on an arbitrary queue; callers should marshal to the
    /// main actor as needed.
    func stream(
        config: AIConfig,
        userText: String,
        onDelta: @escaping (String) -> Void
    ) async throws {
        if config.kind == .codexCLI {
            try await CodexAppServerClient().stream(config: config, userText: userText, onDelta: onDelta)
            return
        }
        guard !config.apiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw AIError.missingAPIKey
        }
        let request: URLRequest
        switch config.kind {
        case .openAI:
            request = try Self.openAIRequest(config: config, userText: userText, stream: true)
        case .anthropic:
            request = try Self.anthropicRequest(config: config, userText: userText, stream: true)
        case .codexCLI:
            return
        }

        let (bytes, response): (URLSession.AsyncBytes, URLResponse)
        do {
            (bytes, response) = try await session.bytes(for: request)
        } catch {
            throw AIError.transport(error.localizedDescription)
        }

        guard let http = response as? HTTPURLResponse else {
            throw AIError.transport("The response was not an HTTP response.")
        }
        guard (200..<300).contains(http.statusCode) else {
            let body = try await Self.drain(bytes)
            throw AIError.http(status: http.statusCode, message: Self.extractErrorMessage(body))
        }

        var sawAny = false
        do {
            for try await line in bytes.lines {
                guard let payload = Self.ssePayload(line) else { continue }
                switch config.kind {
                case .openAI:
                    if payload == "[DONE]" { return }
                    switch config.openAIAPIKind {
                    case .chatCompletions:
                        if let text = Self.openAIDelta(payload), !text.isEmpty {
                            sawAny = true
                            onDelta(text)
                        }
                    case .responses:
                        switch Self.openAIResponsesEvent(payload) {
                        case let .delta(text):
                            if !text.isEmpty { sawAny = true; onDelta(text) }
                        case let .error(message):
                            throw AIError.http(status: 200, message: message)
                        case .stop:
                            return
                        case .ignore:
                            break
                        }
                    }
                case .anthropic:
                    switch Self.anthropicEvent(payload) {
                    case let .delta(text):
                        if !text.isEmpty { sawAny = true; onDelta(text) }
                    case let .error(message):
                        throw AIError.http(status: 200, message: message)
                    case .stop:
                        return
                    case .ignore:
                        break
                    }
                case .codexCLI:
                    break
                }
            }
        } catch let error as AIError {
            throw error
        } catch {
            throw AIError.transport(error.localizedDescription)
        }

        if !sawAny { throw AIError.emptyResponse }
    }

    /// Buffered variant returning the full text.
    func complete(config: AIConfig, userText: String) async throws -> String {
        var buffer = ""
        try await stream(config: config, userText: userText) { buffer += $0 }
        return buffer
    }

    // MARK: - Codex app-server

    static func shellQuote(_ string: String) -> String {
        "'" + string.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    /// The codex command to run: `codex` by name (resolved by the user's shell,
    /// so it matches their terminal), or an explicit path if they set one.
    static func codexInvocation(_ pathOrCommand: String) -> String {
        let trimmed = pathOrCommand.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty || trimmed == "codex" { return "codex" }
        return shellQuote(trimmed)
    }

    // MARK: - Model listing

    func listModels(config: AIConfig) async throws -> [String] {
        switch config.kind {
        case .codexCLI:
            return CodexCLI.presetModels
        case .openAI:
            return try await listHTTPModels(base: config.baseURL, headers: ["Authorization": "Bearer \(config.apiKey)"])
        case .anthropic:
            return try await listHTTPModels(base: config.baseURL, headers: [
                "x-api-key": config.apiKey,
                "anthropic-version": "2023-06-01",
            ])
        }
    }

    private func listHTTPModels(base: String, headers: [String: String]) async throws -> [String] {
        let url = try Self.endpointURL(base: base, path: "/models")
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        for (key, value) in headers { request.setValue(value, forHTTPHeaderField: key) }

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
            throw AIError.http(status: http.statusCode, message: Self.extractErrorMessage(String(data: data, encoding: .utf8) ?? ""))
        }
        return Self.parseModelList(data)
    }

    /// Extracts and sorts model ids from an OpenAI/Anthropic `/models` response.
    static func parseModelList(_ data: Data) -> [String] {
        guard
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let entries = object["data"] as? [[String: Any]]
        else { return [] }
        let ids = entries.compactMap { $0["id"] as? String }
        return Array(Set(ids)).sorted()
    }

    // MARK: - Request building (pure, unit-tested)

    static func endpointURL(base: String, path: String) throws -> URL {
        var trimmed = base.trimmingCharacters(in: .whitespacesAndNewlines)
        while trimmed.hasSuffix("/") { trimmed.removeLast() }
        guard let url = URL(string: trimmed + path), url.scheme != nil else {
            throw AIError.invalidEndpoint(base)
        }
        return url
    }

    static func openAIRequest(config: AIConfig, userText: String, stream: Bool) throws -> URLRequest {
        switch config.openAIAPIKind {
        case .chatCompletions:
            return try openAIChatCompletionsRequest(config: config, userText: userText, stream: stream)
        case .responses:
            return try openAIResponsesRequest(config: config, userText: userText, stream: stream)
        }
    }

    static func openAIChatCompletionsRequest(config: AIConfig, userText: String, stream: Bool) throws -> URLRequest {
        let url = try endpointURL(base: config.baseURL, path: "/chat/completions")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(config.apiKey)", forHTTPHeaderField: "Authorization")

        var messages: [[String: Any]] = []
        let system = config.systemPrompt.trimmingCharacters(in: .whitespacesAndNewlines)
        if !system.isEmpty {
            messages.append(["role": "system", "content": system])
        }
        messages.append(["role": "user", "content": userText])

        var body: [String: Any] = [
            "model": config.model,
            "messages": messages,
            "stream": stream,
        ]
        body["temperature"] = 0.3
        request.httpBody = try JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])
        return request
    }

    static func openAIResponsesRequest(config: AIConfig, userText: String, stream: Bool) throws -> URLRequest {
        let url = try endpointURL(base: config.baseURL, path: "/responses")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(config.apiKey)", forHTTPHeaderField: "Authorization")

        var body: [String: Any] = [
            "model": config.model,
            "input": [
                [
                    "role": "user",
                    "content": userText,
                ],
            ],
            "stream": stream,
            "temperature": 0.3,
        ]
        let system = config.systemPrompt.trimmingCharacters(in: .whitespacesAndNewlines)
        if !system.isEmpty {
            body["instructions"] = system
        }
        request.httpBody = try JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])
        return request
    }

    static func anthropicRequest(config: AIConfig, userText: String, stream: Bool) throws -> URLRequest {
        let url = try endpointURL(base: config.baseURL, path: "/messages")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue(config.apiKey, forHTTPHeaderField: "x-api-key")
        request.setValue("2023-06-01", forHTTPHeaderField: "anthropic-version")

        var body: [String: Any] = [
            "model": config.model,
            "max_tokens": config.maxTokens,
            "messages": [["role": "user", "content": userText]],
            "stream": stream,
        ]
        let system = config.systemPrompt.trimmingCharacters(in: .whitespacesAndNewlines)
        if !system.isEmpty {
            body["system"] = system
        }
        request.httpBody = try JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])
        return request
    }

    // MARK: - SSE parsing (pure, unit-tested)

    /// Returns the payload after a leading `data:` field, or nil for other lines.
    static func ssePayload(_ line: String) -> String? {
        guard line.hasPrefix("data:") else { return nil }
        return String(line.dropFirst("data:".count)).trimmingCharacters(in: .whitespaces)
    }

    static func openAIDelta(_ payload: String) -> String? {
        guard
            let data = payload.data(using: .utf8),
            let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let choices = obj["choices"] as? [[String: Any]],
            let delta = choices.first?["delta"] as? [String: Any],
            let content = delta["content"] as? String
        else { return nil }
        return content
    }

    enum OpenAIResponsesChunk: Equatable {
        case delta(String)
        case error(String)
        case stop
        case ignore
    }

    static func openAIResponsesEvent(_ payload: String) -> OpenAIResponsesChunk {
        guard
            let data = payload.data(using: .utf8),
            let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let type = obj["type"] as? String
        else { return .ignore }

        switch type {
        case "response.output_text.delta":
            return .delta(obj["delta"] as? String ?? "")
        case "response.completed":
            return .stop
        case "response.failed", "response.incomplete":
            if let response = obj["response"] as? [String: Any],
               let error = response["error"] as? [String: Any],
               let message = error["message"] as? String {
                return .error(message)
            }
            return .error("The provider reported a Responses API failure.")
        case "error":
            if let error = obj["error"] as? [String: Any], let message = error["message"] as? String {
                return .error(message)
            }
            if let message = obj["message"] as? String { return .error(message) }
            return .error("The provider reported a Responses API stream error.")
        default:
            return .ignore
        }
    }

    enum AnthropicChunk: Equatable {
        case delta(String)
        case error(String)
        case stop
        case ignore
    }

    static func anthropicEvent(_ payload: String) -> AnthropicChunk {
        guard
            let data = payload.data(using: .utf8),
            let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let type = obj["type"] as? String
        else { return .ignore }

        switch type {
        case "content_block_delta":
            if let delta = obj["delta"] as? [String: Any], let text = delta["text"] as? String {
                return .delta(text)
            }
            return .ignore
        case "message_stop":
            return .stop
        case "error":
            let message = (obj["error"] as? [String: Any])?["message"] as? String
            return .error(message ?? "The provider reported a stream error.")
        default:
            return .ignore
        }
    }

    // MARK: - Helpers

    private static func drain(_ bytes: URLSession.AsyncBytes) async throws -> String {
        var collected = ""
        for try await line in bytes.lines {
            collected += line + "\n"
            if collected.count > 8000 { break }
        }
        return collected
    }

    /// Best-effort extraction of a human message from a JSON error body.
    static func extractErrorMessage(_ body: String) -> String {
        let trimmed = body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard
            let data = trimmed.data(using: .utf8),
            let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return String(trimmed.prefix(400)) }

        if let error = obj["error"] as? [String: Any], let message = error["message"] as? String {
            return message
        }
        if let message = obj["message"] as? String { return message }
        if let error = obj["error"] as? String { return error }
        return String(trimmed.prefix(400))
    }
}
