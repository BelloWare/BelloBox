import XCTest
@testable import BelloBox

final class AIClientTests: XCTestCase {
    private func config(_ kind: ProviderKind, base: String) -> AIConfig {
        AIConfig(kind: kind, baseURL: base, model: "m-1", apiKey: "sk-test", systemPrompt: "be terse")
    }

    // MARK: - Endpoint construction

    func testEndpointTrimsTrailingSlash() throws {
        let url = try AIClient.endpointURL(base: "https://api.example.com/v1/", path: "/chat/completions")
        XCTAssertEqual(url.absoluteString, "https://api.example.com/v1/chat/completions")
    }

    func testEndpointRejectsSchemelessBase() {
        XCTAssertThrowsError(try AIClient.endpointURL(base: "not a url", path: "/messages"))
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
        let messages = try XCTUnwrap(json["messages"] as? [[String: Any]])
        XCTAssertEqual(messages.first?["role"] as? String, "system")
        XCTAssertEqual(messages.first?["content"] as? String, "be terse")
        XCTAssertEqual(messages.last?["role"] as? String, "user")
        XCTAssertEqual(messages.last?["content"] as? String, "hello")
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
        let messages = try XCTUnwrap(json["messages"] as? [[String: Any]])
        XCTAssertEqual(messages.count, 1)
        XCTAssertEqual(messages.first?["role"] as? String, "user")
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
        XCTAssertTrue(c.isUsable) // Codex resolves `codex` from the shell
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

    func testIsCodexConfigError() {
        XCTAssertTrue(AIClient.isCodexConfigError("Error loading config.toml: unknown variant `priority`, expected `fast` or `flex`"))
        XCTAssertTrue(AIClient.isCodexConfigError("unknown field `foo`"))
        XCTAssertFalse(AIClient.isCodexConfigError("command not found: codex"))
        XCTAssertFalse(AIClient.isCodexConfigError("401 Unauthorized"))
    }

    func testCodexPromptCombinesSystemAndUser() {
        XCTAssertTrue(AIClient.codexPrompt(system: "be terse", user: "hi").hasPrefix("be terse\n\nhi"))
        XCTAssertTrue(AIClient.codexPrompt(system: "   ", user: "hi").hasPrefix("hi"))
        XCTAssertTrue(AIClient.codexPrompt(system: "", user: "hi").contains("Output only"))
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
