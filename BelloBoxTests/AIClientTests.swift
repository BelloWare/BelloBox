import XCTest
@testable import BelloBox

final class AIClientTests: XCTestCase {
    private func config(_ kind: ProviderKind, base: String) -> AIConfig {
        AIConfig(kind: kind, baseURL: base, model: "m-1", apiKey: "sk-test", systemPrompt: "be terse")
    }

    private func temporaryDefaults(_ name: String = UUID().uuidString) -> UserDefaults {
        let suiteName = "BelloBoxTests.\(name)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }

    // MARK: - Endpoint construction

    func testEndpointTrimsTrailingSlash() throws {
        let url = try AIClient.endpointURL(base: "https://api.example.com/v1/", path: "/chat/completions")
        XCTAssertEqual(url.absoluteString, "https://api.example.com/v1/chat/completions")
    }

    func testEndpointRejectsSchemelessBase() {
        XCTAssertThrowsError(try AIClient.endpointURL(base: "not a url", path: "/messages"))
    }

    func testSettingsResolveTemperature() throws {
        let settings = AppSettings(defaults: temporaryDefaults())
        XCTAssertNil(settings.currentConfig.temperature)

        settings.temperatureMode = .custom
        settings.temperature = 0.8
        XCTAssertEqual(try XCTUnwrap(settings.currentConfig.temperature), 0.8, accuracy: 0.0001)

        settings.providerKind = .anthropic
        settings.temperature = 1.7
        XCTAssertEqual(try XCTUnwrap(settings.currentConfig.temperature), 1.0, accuracy: 0.0001)
    }

    // MARK: - OpenAI request

    func testOpenAIRequestShape() throws {
        let request = try AIClient.openAIRequest(config: config(.openAI, base: "https://api.openai.com/v1"), userText: "hello", stream: true)
        XCTAssertEqual(request.url?.absoluteString, "https://api.openai.com/v1/chat/completions")
        XCTAssertEqual(request.httpMethod, "POST")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer sk-test")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")

        let body = try XCTUnwrap(request.httpBody)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
        XCTAssertEqual(json["model"] as? String, "m-1")
        XCTAssertEqual(json["stream"] as? Bool, true)
        XCTAssertNil(json["temperature"])
        let messages = try XCTUnwrap(json["messages"] as? [[String: Any]])
        XCTAssertEqual(messages.first?["role"] as? String, "system")
        XCTAssertEqual(messages.first?["content"] as? String, "be terse")
        XCTAssertEqual(messages.last?["role"] as? String, "user")
        XCTAssertEqual(messages.last?["content"] as? String, "hello")
    }

    func testOpenAIResponsesRequestShape() throws {
        var c = config(.openAI, base: "https://api.openai.com/v1")
        c.openAIAPIKind = .responses

        let request = try AIClient.openAIRequest(config: c, userText: "hello", stream: true)
        XCTAssertEqual(request.url?.absoluteString, "https://api.openai.com/v1/responses")
        XCTAssertEqual(request.httpMethod, "POST")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer sk-test")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")

        let body = try XCTUnwrap(request.httpBody)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
        XCTAssertEqual(json["model"] as? String, "m-1")
        XCTAssertEqual(json["stream"] as? Bool, true)
        XCTAssertEqual(json["instructions"] as? String, "be terse")
        XCTAssertNil(json["temperature"])
        let input = try XCTUnwrap(json["input"] as? [[String: Any]])
        XCTAssertEqual(input.first?["role"] as? String, "user")
        XCTAssertEqual(input.first?["content"] as? String, "hello")
    }

    // MARK: - Anthropic request

    func testAnthropicRequestShape() throws {
        let request = try AIClient.anthropicRequest(config: config(.anthropic, base: "https://api.anthropic.com/v1"), userText: "hello", stream: true)
        XCTAssertEqual(request.url?.absoluteString, "https://api.anthropic.com/v1/messages")
        XCTAssertEqual(request.value(forHTTPHeaderField: "x-api-key"), "sk-test")
        XCTAssertEqual(request.value(forHTTPHeaderField: "anthropic-version"), "2023-06-01")
        XCTAssertNil(request.value(forHTTPHeaderField: "Authorization"))

        let body = try XCTUnwrap(request.httpBody)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
        XCTAssertEqual(json["model"] as? String, "m-1")
        XCTAssertEqual(json["system"] as? String, "be terse")
        XCTAssertNotNil(json["max_tokens"])
        XCTAssertNil(json["temperature"])
        let messages = try XCTUnwrap(json["messages"] as? [[String: Any]])
        XCTAssertEqual(messages.count, 1)
        XCTAssertEqual(messages.first?["role"] as? String, "user")
    }

    func testCustomTemperatureInOpenAIChatRequest() throws {
        var c = config(.openAI, base: "https://api.openai.com/v1")
        c.temperature = 0.7

        let request = try AIClient.openAIRequest(config: c, userText: "hello", stream: true)
        let body = try XCTUnwrap(request.httpBody)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
        let temperature = try XCTUnwrap((json["temperature"] as? NSNumber)?.doubleValue)
        XCTAssertEqual(temperature, 0.7, accuracy: 0.0001)
    }

    func testCustomTemperatureInOpenAIResponsesRequest() throws {
        var c = config(.openAI, base: "https://api.openai.com/v1")
        c.openAIAPIKind = .responses
        c.temperature = 1.0

        let request = try AIClient.openAIRequest(config: c, userText: "hello", stream: true)
        let body = try XCTUnwrap(request.httpBody)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
        let temperature = try XCTUnwrap((json["temperature"] as? NSNumber)?.doubleValue)
        XCTAssertEqual(temperature, 1.0, accuracy: 0.0001)
    }

    func testCustomTemperatureInAnthropicRequest() throws {
        var c = config(.anthropic, base: "https://api.anthropic.com/v1")
        c.temperature = 0.4

        let request = try AIClient.anthropicRequest(config: c, userText: "hello", stream: true)
        let body = try XCTUnwrap(request.httpBody)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
        let temperature = try XCTUnwrap((json["temperature"] as? NSNumber)?.doubleValue)
        XCTAssertEqual(temperature, 0.4, accuracy: 0.0001)
    }

    // MARK: - SSE parsing

    func testSSEPayloadExtraction() {
        XCTAssertEqual(AIClient.ssePayload("data: hello"), "hello")
        XCTAssertEqual(AIClient.ssePayload("data:[DONE]"), "[DONE]")
        XCTAssertNil(AIClient.ssePayload("event: ping"))
        XCTAssertNil(AIClient.ssePayload(""))
    }

    func testOpenAIDeltaParsing() {
        let payload = #"{"choices":[{"delta":{"content":"Hi"}}]}"#
        XCTAssertEqual(AIClient.openAIDelta(payload), "Hi")
        XCTAssertNil(AIClient.openAIDelta(#"{"choices":[{"delta":{}}]}"#))
        XCTAssertNil(AIClient.openAIDelta("garbage"))
    }

    func testOpenAIResponsesEventParsing() {
        XCTAssertEqual(AIClient.openAIResponsesEvent(#"{"type":"response.output_text.delta","delta":"Hi"}"#), .delta("Hi"))
        XCTAssertEqual(AIClient.openAIResponsesEvent(#"{"type":"response.completed"}"#), .stop)
        XCTAssertEqual(
            AIClient.openAIResponsesEvent(#"{"type":"response.failed","response":{"error":{"message":"boom"}}}"#),
            .error("boom")
        )
        XCTAssertEqual(AIClient.openAIResponsesEvent(#"{"type":"response.created"}"#), .ignore)
    }

    func testAnthropicEventParsing() {
        let delta = #"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hi"}}"#
        XCTAssertEqual(AIClient.anthropicEvent(delta), .delta("Hi"))
        XCTAssertEqual(AIClient.anthropicEvent(#"{"type":"message_stop"}"#), .stop)
        XCTAssertEqual(AIClient.anthropicEvent(#"{"type":"error","error":{"message":"boom"}}"#), .error("boom"))
        XCTAssertEqual(AIClient.anthropicEvent(#"{"type":"ping"}"#), .ignore)
    }

    // MARK: - Error extraction

    func testErrorMessageExtraction() {
        XCTAssertEqual(AIClient.extractErrorMessage(#"{"error":{"message":"bad key"}}"#), "bad key")
        XCTAssertEqual(AIClient.extractErrorMessage(#"{"message":"nope"}"#), "nope")
        XCTAssertEqual(AIClient.extractErrorMessage("plain text"), "plain text")
    }

    // MARK: - Prompt building

    func testUserMessageEmbedsSelection() {
        let message = QuickAction.userMessage(instruction: "Fix it", selectedText: "teh cat")
        XCTAssertTrue(message.contains("Fix it"))
        XCTAssertTrue(message.contains("teh cat"))
    }

    func testConfigUsability() {
        var c = config(.openAI, base: "https://api.openai.com/v1")
        XCTAssertTrue(c.isUsable)
        c.apiKey = "  "
        XCTAssertFalse(c.isUsable)
    }

    // MARK: - Codex + model listing

    func testProviderKindHasCodex() {
        XCTAssertEqual(ProviderKind.allCases.count, 3)
        XCTAssertFalse(ProviderKind.codexCLI.isHTTP)
        XCTAssertTrue(ProviderKind.openAI.isHTTP)
        XCTAssertEqual(ProviderKind.codexCLI.shortName, "Codex")
    }

    func testCodexConfigAlwaysUsable() {
        var c = AIConfig(kind: .codexCLI, baseURL: "", model: "", apiKey: "", systemPrompt: "")
        XCTAssertTrue(c.isUsable) // Codex app-server resolves `codex` from the shell.
        c.baseURL = "/usr/local/bin/codex"
        XCTAssertTrue(c.isUsable)
    }

    func testCodexInvocation() {
        XCTAssertEqual(AIClient.codexInvocation(""), "codex")
        XCTAssertEqual(AIClient.codexInvocation("codex"), "codex")
        XCTAssertEqual(AIClient.codexInvocation("/Users/me/bin/codex"), "'/Users/me/bin/codex'")
    }

    func testShellQuote() {
        XCTAssertEqual(AIClient.shellQuote("/a/b"), "'/a/b'")
        XCTAssertEqual(AIClient.shellQuote("a'b"), "'a'\\''b'")
    }

    func testCodexAppServerCommand() {
        XCTAssertEqual(CodexAppServerClient.appServerCommand(""), "codex app-server --stdio")
        XCTAssertEqual(CodexAppServerClient.appServerCommand("/Users/me/bin/codex"), "'/Users/me/bin/codex' app-server --stdio")
    }

    func testCodexDeveloperInstructions() {
        let instructions = CodexAppServerClient.developerInstructions(system: "be terse")
        XCTAssertTrue(instructions.hasPrefix("be terse\n\n"))
        XCTAssertTrue(instructions.contains("Output only"))
    }

    func testCodexThreadStartParamsPassModelAndInstructions() throws {
        var c = AIConfig(kind: .codexCLI, baseURL: "", model: "gpt-5-codex", apiKey: "", systemPrompt: "be terse")
        c.codexReasoningEffort = "high"
        let params = CodexAppServerClient.threadStartParams(config: c, cwd: "/tmp")
        XCTAssertEqual(params["model"] as? String, "gpt-5-codex")
        XCTAssertEqual(params["cwd"] as? String, "/tmp")
        XCTAssertEqual(params["approvalPolicy"] as? String, "never")
        XCTAssertEqual(params["sandbox"] as? String, "read-only")
        XCTAssertEqual(params["ephemeral"] as? Bool, true)
        XCTAssertTrue((params["developerInstructions"] as? String)?.contains("be terse") == true)
    }

    func testCodexTurnStartParamsPassEffortAndInput() throws {
        var c = AIConfig(kind: .codexCLI, baseURL: "", model: "gpt-5-codex", apiKey: "", systemPrompt: "")
        c.codexReasoningEffort = "xhigh"
        let params = CodexAppServerClient.turnStartParams(threadId: "t-1", config: c, userText: "hello", cwd: "/tmp")
        XCTAssertEqual(params["threadId"] as? String, "t-1")
        XCTAssertEqual(params["model"] as? String, "gpt-5-codex")
        XCTAssertEqual(params["effort"] as? String, "xhigh")
        let input = try XCTUnwrap(params["input"] as? [[String: Any]])
        XCTAssertEqual(input.first?["type"] as? String, "text")
        XCTAssertEqual(input.first?["text"] as? String, "hello")
        let sandbox = try XCTUnwrap(params["sandboxPolicy"] as? [String: Any])
        XCTAssertEqual(sandbox["type"] as? String, "readOnly")
        XCTAssertEqual(sandbox["networkAccess"] as? Bool, true)
    }

    func testCodexAppServerParsesStreamingNotifications() throws {
        let delta = try XCTUnwrap(CodexAppServerClient.jsonObject(from: #"{"method":"item/agentMessage/delta","params":{"delta":"Hi"}}"#))
        XCTAssertEqual(CodexAppServerClient.agentMessageDelta(from: delta), "Hi")

        let completed = try XCTUnwrap(CodexAppServerClient.jsonObject(from: #"{"method":"item/completed","params":{"item":{"type":"agentMessage","text":"Hi there"}}}"#))
        XCTAssertEqual(CodexAppServerClient.completedAgentMessage(from: completed), "Hi there")

        let turn = try XCTUnwrap(CodexAppServerClient.jsonObject(from: #"{"method":"turn/completed","params":{"turn":{"status":"completed"}}}"#))
        XCTAssertEqual(CodexAppServerClient.turnCompletion(from: turn)?.status, "completed")
    }

    func testCodexAppServerParsesThreadAndErrors() throws {
        let response = try XCTUnwrap(CodexAppServerClient.jsonObject(
            from: #"{"id":2,"result":{"thread":{"id":"thread-1"}}}"#
        ))
        XCTAssertEqual(CodexAppServerClient.threadID(from: response), "thread-1")

        let error = try XCTUnwrap(CodexAppServerClient.jsonObject(
            from: #"{"id":1,"error":{"message":"boom"}}"#
        ))
        XCTAssertEqual(CodexAppServerClient.jsonRPCErrorMessage(error), "boom")
    }

    func testCodexDefaultsFillBlankModelAndEffort() {
        let c = AIConfig(kind: .codexCLI, baseURL: "", model: " ", apiKey: "", systemPrompt: "", codexReasoningEffort: "")
        XCTAssertEqual(CodexAppServerClient.resolvedModel(c), CodexCLI.defaultModel)
        XCTAssertEqual(CodexAppServerClient.resolvedReasoningEffort(c), CodexCLI.defaultReasoningEffort)
    }

    func testParseModelList() {
        let json = #"{"data":[{"id":"gpt-4o"},{"id":"gpt-3.5-turbo"},{"id":"gpt-4o"}]}"#
        XCTAssertEqual(AIClient.parseModelList(Data(json.utf8)), ["gpt-3.5-turbo", "gpt-4o"])
    }

    func testParseModelListEmptyOnGarbage() {
        XCTAssertEqual(AIClient.parseModelList(Data("not json".utf8)), [])
        XCTAssertEqual(AIClient.parseModelList(Data(#"{"foo":1}"#.utf8)), [])
    }

    func testCodexPresetsNonEmpty() {
        XCTAssertFalse(CodexCLI.presetModels.isEmpty)
    }
}
