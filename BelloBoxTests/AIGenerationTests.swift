import XCTest
@testable import BelloBox

final class AIGenerationTests: XCTestCase {
    private let image = AIImagePayload(data: Data([1, 2, 3]), mimeType: "image/png", width: 1, height: 1, digest: "fixture")

    private func isolatedDefaults() -> UserDefaults {
        let suite = "BelloBoxTests.Generation.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        addTeardownBlock { defaults.removePersistentDomain(forName: suite) }
        return defaults
    }

    private func json(_ request: URLRequest) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: XCTUnwrap(request.httpBody)) as? [String: Any])
    }

    private func bodies(_ config: AIConfig) throws -> [(AIGenerationWireFormat, [String: Any])] {
        [
            (.openAIChat, try json(AIClient.openAIChatCompletionsRequest(config: config, userText: "fixture", stream: true))),
            (.openAIChat, AIImageClient.openAIChatVisionBody(config: config, image: image, prompt: "fixture", stream: false)),
            (.openAIResponses, try json(AIClient.openAIResponsesRequest(config: config, userText: "fixture", stream: false))),
            (.openAIResponses, AIImageClient.openAIResponsesVisionBody(config: config, image: image, prompt: "fixture")),
            (.anthropic, try json(AIClient.anthropicRequest(config: config, userText: "fixture", stream: true))),
            (.anthropic, AIImageClient.anthropicVisionBody(config: config, image: image, prompt: "fixture")),
        ]
    }

    func testDefaultsOmitEveryOptionalParameterInTextAndImageRequests() throws {
        let config = AppSettings(defaults: isolatedDefaults()).currentConfig
        for (_, body) in try bodies(config) {
            for field in ["temperature", "reasoning_effort", "reasoning", "output_config", "thinking"] {
                XCTAssertNil(body[field], "Default must omit \(field), not send a null or default sentinel")
            }
        }
    }

    func testStandardDefaultsGenerationSettingsStayInMemoryDuringTests() {
        let before = UserDefaults.standard.data(forKey: "aiModelGeneration.v1")
        let migration = UserDefaults.standard.bool(forKey: "aiModelGenerationMigrated.v1")
        let settings = AppSettings(defaults: .standard)
        settings.temperatureMode = .custom
        settings.temperature = 0.2
        settings.reasoningEffort = .high
        settings.resetModelGenerationPreferences()
        XCTAssertEqual(UserDefaults.standard.data(forKey: "aiModelGeneration.v1"), before)
        XCTAssertEqual(UserDefaults.standard.bool(forKey: "aiModelGenerationMigrated.v1"), migration)
    }

    func testEffortsUseEachActualWireFormatForTextAndImages() throws {
        var config = AppSettings(defaults: isolatedDefaults()).currentConfig
        // Builders are also called directly by OCR: their actual wire format
        // must win even if config.openAIAPIKind still says Chat.
        for effort in AIReasoningEffort.allCases where effort != .providerDefault {
            config.reasoningEffort = effort
            for (format, body) in try bodies(config) {
                switch format {
                case .openAIChat:
                    XCTAssertEqual(body["reasoning_effort"] as? String, effort.rawValue)
                    XCTAssertNil(body["reasoning"])
                    XCTAssertNil(body["output_config"])
                case .openAIResponses:
                    XCTAssertEqual((body["reasoning"] as? [String: String])?["effort"], effort.rawValue)
                    XCTAssertNil(body["reasoning_effort"])
                    XCTAssertNil(body["output_config"])
                case .anthropic:
                    let expected = AIReasoningEffort.choices(for: .anthropic).contains(effort) ? effort.rawValue : nil
                    XCTAssertEqual((body["output_config"] as? [String: String])?["effort"], expected)
                    XCTAssertNil(body["reasoning"])
                    XCTAssertNil(body["reasoning_effort"])
                }
            }
        }
    }

    func testNoneIsExplicitAndIndependentOfTemperature() throws {
        let settings = AppSettings(defaults: isolatedDefaults())
        settings.reasoningEffort = .none
        settings.temperatureMode = .custom
        settings.temperature = 0.4
        let body = try json(AIClient.openAIRequest(config: settings.currentConfig, userText: "fixture", stream: false))
        XCTAssertEqual(body["reasoning_effort"] as? String, "none")
        XCTAssertEqual(body["temperature"] as? Double, 0.4)
        settings.reasoningEffort = .providerDefault
        settings.temperatureMode = .providerDefault
        let defaults = try json(AIClient.openAIRequest(config: settings.currentConfig, userText: "fixture", stream: false))
        XCTAssertNil(defaults["reasoning_effort"])
        XCTAssertNil(defaults["temperature"])
    }

    func testAnthropicThinkingOmitsTemperatureAndLeavesAnswerHeadroom() throws {
        var config = AppSettings(defaults: isolatedDefaults()).currentConfig
        config.temperature = 0.4
        config.reasoningEffort = .high
        for mode in AnthropicThinkingMode.allCases {
            config.anthropicThinking = mode
            config.anthropicThinkingBudget = 4096
            for (format, body) in try bodies(config) where format == .anthropic {
                let thinking = body["thinking"] as? [String: Any]
                XCTAssertEqual(body["temperature"] as? Double, mode.isEnabled ? nil : 0.4)
                XCTAssertEqual((body["output_config"] as? [String: String])?["effort"], "high")
                switch mode {
                case .providerDefault: XCTAssertNil(thinking)
                case .disabled: XCTAssertEqual(thinking?["type"] as? String, "disabled")
                case .adaptive:
                    XCTAssertEqual(thinking?["type"] as? String, "adaptive")
                    XCTAssertNil(thinking?["budget_tokens"])
                    XCTAssertEqual(body["max_tokens"] as? Int, 8192)
                case .budgeted:
                    XCTAssertEqual(thinking?["type"] as? String, "enabled")
                    XCTAssertEqual(thinking?["budget_tokens"] as? Int, 4096)
                    XCTAssertEqual(body["max_tokens"] as? Int, 6144)
                }
            }
        }
    }

    func testThinkingBudgetsAreBoundedAndNeverConsumeTheWholeOutputLimit() throws {
        var config = AppSettings(defaults: isolatedDefaults()).currentConfig
        config.anthropicThinking = .budgeted
        config.temperature = .infinity
        for budget in [Int.min, 0, 1024, 32768, Int.max] {
            config.anthropicThinkingBudget = budget
            for (format, body) in try bodies(config) where format == .anthropic {
                let tokens = try XCTUnwrap((body["thinking"] as? [String: Any])?["budget_tokens"] as? Int)
                XCTAssertTrue((1024 ... 32768).contains(tokens))
                XCTAssertGreaterThanOrEqual(try XCTUnwrap(body["max_tokens"] as? Int), tokens + 2048)
                XCTAssertNil(body["temperature"])
            }
        }
        config.maxTokens = 60000
        let body = AIImageClient.anthropicVisionBody(config: config, image: image, prompt: "fixture")
        XCTAssertEqual(body["max_tokens"] as? Int, 60000, "A feature's larger output limit must survive")
    }

    func testSwitchingModelsAndEndpointsRestoresIndependentProfiles() throws {
        let defaults = isolatedDefaults()
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "creative-model"
        settings.temperatureMode = .custom
        settings.temperature = 1.5
        settings.reasoningEffort = .low
        let creative = settings.currentConfig
        settings.openAIModel = "reasoning-model"
        XCTAssertNil(settings.currentConfig.temperature)
        XCTAssertEqual(settings.reasoningEffort, .providerDefault)
        settings.reasoningEffort = .xhigh
        settings.openAIBaseURL = "https://other.example/v1"
        XCTAssertEqual(settings.reasoningEffort, .providerDefault)
        settings.openAIBaseURL = ProviderKind.openAI.defaultBaseURL
        XCTAssertEqual(settings.reasoningEffort, .xhigh)
        settings.openAIModel = "creative-model"
        XCTAssertEqual(settings.currentConfig, creative)
        XCTAssertEqual(AppSettings(defaults: defaults).currentConfig, creative)
        settings.providerKind = .anthropic
        XCTAssertNil(settings.currentConfig.temperature)
        XCTAssertEqual(settings.reasoningEffort, .providerDefault)
    }

    func testModelIdentityNormalizesWhitespaceAndSlashesWithoutMergingDifferentPaths() {
        func key(_ endpoint: String, _ model: String = "my-model", _ provider: ProviderKind = .openAI) -> String {
            AIGenerationPreferences.key(provider: provider, endpoint: endpoint, model: model)
        }
        XCTAssertEqual(key(" HTTPS://API.EXAMPLE/v1/// ", " my-model\n"), key("https://api.example/v1"))
        XCTAssertNotEqual(key("https://api.example/v1"), key("https://api.example/V1"))
        XCTAssertNotEqual(key("https://api.example/v1"), key("https://api.example/v2"))
        XCTAssertNotEqual(key("https://api.example/v1"), key("https://api.example/v1", "My-Model"))
        XCTAssertNotEqual(key("https://api.example/v1"), key("https://api.example/v1", "my-model", .anthropic))
    }

    func testLegacyTemperatureMigratesOnlyOnceToTheSelectedModel() {
        let defaults = isolatedDefaults()
        defaults.set("legacy", forKey: "openAIModel")
        defaults.set("custom", forKey: "temperatureMode")
        defaults.set(0.7, forKey: "temperature")
        let settings = AppSettings(defaults: defaults)
        XCTAssertEqual(settings.currentConfig.temperature, 0.7)
        settings.openAIModel = "new-model"
        XCTAssertNil(settings.currentConfig.temperature)
        settings.openAIModel = "legacy"
        settings.resetModelGenerationPreferences()
        XCTAssertNil(AppSettings(defaults: defaults).currentConfig.temperature, "Reset must not reimport the old global setting")
    }

    func testCodexEffortMigratesAndFollowsItsModelEvenWhenHTTPIsActive() {
        let defaults = isolatedDefaults()
        defaults.set("codex-a", forKey: "codexModel")
        defaults.set("xhigh", forKey: "codexReasoningEffort")
        let settings = AppSettings(defaults: defaults)
        XCTAssertEqual(settings.codexReasoningEffort, "xhigh")
        XCTAssertEqual(settings.reasoningEffort, .providerDefault)
        settings.providerKind = .codexCLI
        XCTAssertEqual(settings.currentConfig.codexReasoningEffort, "xhigh")
        settings.codexModel = "codex-b"
        XCTAssertEqual(settings.codexReasoningEffort, "medium")
        settings.codexReasoningEffort = "low"
        settings.codexModel = "codex-a"
        XCTAssertEqual(settings.codexReasoningEffort, "xhigh")
        settings.resetModelGenerationPreferences()
        XCTAssertEqual(AppSettings(defaults: defaults).codexReasoningEffort, "medium")
    }

    func testThinkingKeepsAnInactiveTemperatureChoiceAndConfigIsASnapshot() {
        let settings = AppSettings(defaults: isolatedDefaults())
        settings.providerKind = .anthropic
        settings.temperatureMode = .custom
        settings.temperature = 0.3
        let before = settings.currentConfig
        settings.generationPreferences.thinkingMode = .adaptive
        XCTAssertNil(settings.currentConfig.temperature)
        XCTAssertEqual(settings.temperature, 0.3)
        XCTAssertNotEqual(before, settings.currentConfig)
        XCTAssertEqual(before.temperature, 0.3)
        settings.generationPreferences.thinkingMode = .disabled
        XCTAssertEqual(settings.currentConfig.temperature, 0.3)
    }

    func testInvalidPreferencesNormalizeAndNewFieldsDecodeIndependently() throws {
        let data = Data(#"{"temperatureMode":"custom","temperature":99,"reasoningEffort":"future","thinkingMode":"budgeted","thinkingBudget":0,"outputTokenLimit":999999}"#.utf8)
        let options = try JSONDecoder().decode(AIGenerationPreferences.self, from: data).normalized(for: .anthropic)
        XCTAssertEqual(options.temperatureMode, .custom)
        XCTAssertEqual(options.temperature, 1)
        XCTAssertEqual(options.reasoningEffort, .providerDefault)
        XCTAssertEqual(options.thinkingBudget, 1024)
        XCTAssertEqual(options.outputTokenLimit, 65536)
        let settings = AppSettings(defaults: isolatedDefaults())
        settings.generationPreferences = options
        settings.temperature = .nan
        XCTAssertEqual(settings.temperature, 1)
    }

    func testBrowsingModelsDoesNotCreateProfilesAndExplicitProfilesAreBounded() throws {
        let defaults = isolatedDefaults()
        let settings = AppSettings(defaults: defaults)
        for i in 0 ..< 150 {
            settings.openAIModel = "model-\(i)"
            _ = settings.currentConfig
        }
        XCTAssertNil(defaults.data(forKey: "aiModelGeneration.v1"))
        for i in 0 ..< 130 {
            settings.openAIModel = "model-\(i)"
            settings.reasoningEffort = .high
        }
        let data = try XCTUnwrap(defaults.data(forKey: "aiModelGeneration.v1"))
        let entries = try JSONDecoder().decode([String: AIModelPreferenceEntry].self, from: data)
        XCTAssertEqual(entries.count, 128)
        XCTAssertEqual(AppSettings(defaults: defaults).reasoningEffort, .high)
        let stored = try XCTUnwrap(String(data: data, encoding: .utf8))
        XCTAssertFalse(stored.contains("model-"))
        XCTAssertFalse(stored.contains("api.openai.com"))
    }

    func testCopilotAndVisionCarryModelOptionsThroughHTTPAndIgnoreThinkingText() async throws {
        let settings = AppSettings(defaults: isolatedDefaults())
        let context = WorldClockCopilotContext(selectedInstant: Date(), referenceZoneID: "UTC", locations: [],
                                               now: Date(), localZoneID: "UTC", isFollowingNow: true)
        let question = WorldClockCopilotRequest(question: "What time is it?", context: context, history: [])
        let sessionConfig = URLSessionConfiguration.ephemeral
        sessionConfig.protocolClasses = [GenerationURLProtocol.self]
        let session = URLSession(configuration: sessionConfig)
        defer { session.invalidateAndCancel(); GenerationURLProtocol.handler = nil }
        let textClient = AIClient(session: session)
        let resolver = WorldClockAIResolver(client: textClient)
        let imageClient = AIImageClient(session: session)

        for (kind, api) in [(ProviderKind.openAI, OpenAIAPIKind.chatCompletions), (.openAI, .responses), (.anthropic, .chatCompletions)] {
            settings.providerKind = kind
            settings.openAIAPIKind = api
            settings.apiKey = "fixture-only"
            settings.reasoningEffort = .xhigh
            if kind == .anthropic { settings.generationPreferences.thinkingMode = .adaptive }
            let config = settings.currentConfig
            let received = expectation(description: "Copilot and image requests use \(kind)/\(api)")
            received.expectedFulfillmentCount = 2
            GenerationURLProtocol.handler = { request in
                let body = try GenerationURLProtocol.body(request)
                XCTAssertNil(body["temperature"])
                if kind == .anthropic {
                    XCTAssertEqual((body["output_config"] as? [String: String])?["effort"], "xhigh")
                    XCTAssertEqual((body["thinking"] as? [String: String])?["type"], "adaptive")
                    XCTAssertEqual(body["max_tokens"] as? Int, 8192)
                } else if api == .responses {
                    XCTAssertEqual((body["reasoning"] as? [String: String])?["effort"], "xhigh")
                } else { XCTAssertEqual(body["reasoning_effort"] as? String, "xhigh") }
                received.fulfill()
                let answer = #"{"answer":"It is noon UTC.","suggestion":null}"#
                let data: Data
                if body["stream"] as? Bool == true {
                    let events: [[String: Any]]
                    if kind == .anthropic {
                        events = [
                            ["type": "content_block_delta", "delta": ["type": "thinking_delta", "thinking": "private reasoning fixture"]],
                            ["type": "content_block_delta", "delta": ["type": "text_delta", "text": answer]],
                            ["type": "message_stop"],
                        ]
                    } else if api == .responses {
                        events = [["type": "response.output_text.delta", "delta": answer], ["type": "response.completed"]]
                    } else {
                        events = [["choices": [["delta": ["content": answer]]]]]
                    }
                    let lines = try events.map { "data: " + String(decoding: try JSONSerialization.data(withJSONObject: $0), as: UTF8.self) + "\n\n" }.joined()
                    data = Data((lines + (kind == .openAI && api == .chatCompletions ? "data: [DONE]\n\n" : "")).utf8)
                } else {
                    let response: [String: Any] = kind == .anthropic
                        ? ["content": [["type": "thinking", "thinking": "private reasoning fixture"], ["type": "text", "text": "visible OCR text"]]]
                        : api == .responses ? ["output_text": "visible OCR text"] : ["choices": [["message": ["content": "visible OCR text"]]]]
                    data = try JSONSerialization.data(withJSONObject: response)
                }
                return data
            }
            let reply = try await resolver.ask(question, config: config)
            XCTAssertEqual(reply.answer, "It is noon UTC.")
            let ocr = try await imageClient.completeVision(config: config, image: image, prompt: "Read this fixture", responseFormat: .plainText)
            XCTAssertEqual(ocr, "visible OCR text")
            await fulfillment(of: [received], timeout: 3)
        }
    }

    func testReasoningOnlyOutputLimitProvidesActionableError() async throws {
        let settings = AppSettings(defaults: isolatedDefaults())
        settings.providerKind = .anthropic
        settings.apiKey = "fixture-only"
        let config = settings.currentConfig
        let sessionConfig = URLSessionConfiguration.ephemeral
        sessionConfig.protocolClasses = [GenerationURLProtocol.self]
        let session = URLSession(configuration: sessionConfig)
        defer { session.invalidateAndCancel(); GenerationURLProtocol.handler = nil }
        GenerationURLProtocol.handler = { _ in
            Data("data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"fixture\"}}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"max_tokens\"}}\n\ndata: {\"type\":\"message_stop\"}\n\n".utf8)
        }
        do {
            _ = try await AIClient(session: session).complete(config: config, userText: "fixture")
            XCTFail("A thinking-only response must not succeed or report a generic empty response")
        } catch { XCTAssertEqual(error as? AIError, .reasoningOutputLimit) }
        let imageResponse = Data(#"{"content":[{"type":"thinking","thinking":"fixture"}],"stop_reason":"max_tokens"}"#.utf8)
        XCTAssertThrowsError(try AIImageClient.extractText(from: imageResponse, config: config)) {
            XCTAssertEqual($0 as? AIError, .reasoningOutputLimit)
        }
        XCTAssertTrue(AIError.reasoningOutputLimit.localizedDescription.contains("Thinking & token limits"))
    }
}

private final class GenerationURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> Data)?
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        do {
            let data = try XCTUnwrap(Self.handler)(request)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: nil)!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch { client?.urlProtocol(self, didFailWithError: error) }
    }
    override func stopLoading() {}

    static func body(_ request: URLRequest) throws -> [String: Any] {
        var data = request.httpBody ?? Data()
        if data.isEmpty, let stream = request.httpBodyStream {
            stream.open()
            defer { stream.close() }
            var buffer = [UInt8](repeating: 0, count: 4096)
            while stream.hasBytesAvailable {
                let count = stream.read(&buffer, maxLength: buffer.count)
                guard count > 0 else { break }
                data.append(contentsOf: buffer.prefix(count))
            }
        }
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }
}
