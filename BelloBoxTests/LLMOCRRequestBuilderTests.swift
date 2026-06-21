import XCTest
@testable import BelloBox

final class LLMOCRRequestBuilderTests: XCTestCase {
    private let image = AIImagePayload(data: Data([1, 2, 3]), mimeType: "image/png", width: 1, height: 1, digest: "d")

    func testOpenAIResponsesPayloadContainsTextAndImage() {
        var config = AIConfig(kind: .openAI, baseURL: "https://api.example.com/v1", model: "m", apiKey: "secret", systemPrompt: "")
        config.openAIAPIKind = .responses
        let body = AIImageClient.openAIResponsesVisionBody(config: config, image: image, prompt: "read")
        let input = body["input"] as? [[String: Any]]
        let content = input?.first?["content"] as? [[String: Any]]
        XCTAssertEqual(content?.first?["type"] as? String, "input_text")
        XCTAssertEqual(content?.last?["type"] as? String, "input_image")
        XCTAssertTrue(((content?.last?["image_url"] as? String) ?? "").hasPrefix("data:image/png;base64,"))
        XCTAssertNil(body["temperature"])
    }

    func testOpenAIChatPayloadContainsImageURL() {
        let config = AIConfig(kind: .openAI, baseURL: "https://api.example.com/v1", model: "m", apiKey: "secret", systemPrompt: "")
        let body = AIImageClient.openAIChatVisionBody(config: config, image: image, prompt: "read", stream: false)
        let messages = body["messages"] as? [[String: Any]]
        let content = messages?.first?["content"] as? [[String: Any]]
        XCTAssertEqual(content?.last?["type"] as? String, "image_url")
        XCTAssertNil(body["temperature"])
    }

    func testAnthropicPayloadContainsImageBlock() {
        let config = AIConfig(kind: .anthropic, baseURL: "https://api.example.com/v1", model: "m", apiKey: "secret", systemPrompt: "")
        let body = AIImageClient.anthropicVisionBody(config: config, image: image, prompt: "read")
        let messages = body["messages"] as? [[String: Any]]
        let content = messages?.first?["content"] as? [[String: Any]]
        XCTAssertEqual(content?.first?["type"] as? String, "image")
        XCTAssertNil(body["temperature"])
    }

    func testVisionPayloadsIncludeCustomTemperatureOnlyWhenConfigured() {
        var openAI = AIConfig(kind: .openAI, baseURL: "https://api.example.com/v1", model: "m", apiKey: "secret", systemPrompt: "", temperature: 0.7)
        var body = AIImageClient.openAIChatVisionBody(config: openAI, image: image, prompt: "read", stream: false)
        XCTAssertEqual((body["temperature"] as? NSNumber)?.doubleValue, 0.7)

        openAI.openAIAPIKind = .responses
        body = AIImageClient.openAIResponsesVisionBody(config: openAI, image: image, prompt: "read")
        XCTAssertEqual((body["temperature"] as? NSNumber)?.doubleValue, 0.7)

        let anthropic = AIConfig(kind: .anthropic, baseURL: "https://api.example.com/v1", model: "m", apiKey: "secret", systemPrompt: "", temperature: 0.4)
        body = AIImageClient.anthropicVisionBody(config: anthropic, image: image, prompt: "read")
        XCTAssertEqual((body["temperature"] as? NSNumber)?.doubleValue, 0.4)
    }

    func testOpenAIVisionRequestsAllowBlankAPIKey() throws {
        var config = AIConfig(kind: .openAI, baseURL: "https://api.example.com/v1", model: "m", apiKey: " ", systemPrompt: "")
        var request = try AIImageClient.openAIVisionRequest(config: config, image: image, prompt: "read", responseFormat: .json)
        XCTAssertNil(request.value(forHTTPHeaderField: "Authorization"))

        config.openAIAPIKind = .responses
        request = try AIImageClient.openAIVisionRequest(config: config, image: image, prompt: "read", responseFormat: .json)
        XCTAssertNil(request.value(forHTTPHeaderField: "Authorization"))
    }

    func testOpenAICompleteVisionAllowsBlankAPIKey() async throws {
        var capturedAuthorization: String?
        LLMOCRMockURLProtocol.handler = { request in
            capturedAuthorization = request.value(forHTTPHeaderField: "Authorization")
            let response = HTTPURLResponse(
                url: request.url ?? URL(string: "https://api.example.com/v1/chat/completions")!,
                statusCode: 200,
                httpVersion: "HTTP/1.1",
                headerFields: ["Content-Type": "application/json"]
            )!
            let body = #"{"choices":[{"message":{"content":"ok"}}]}"#
            return (response, Data(body.utf8))
        }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [LLMOCRMockURLProtocol.self]
        let client = AIImageClient(session: URLSession(configuration: configuration))
        let config = AIConfig(kind: .openAI, baseURL: "https://api.example.com/v1", model: "m", apiKey: "", systemPrompt: "")

        let text = try await client.completeVision(config: config, image: image, prompt: "read", responseFormat: .json)

        XCTAssertEqual(text, "ok")
        XCTAssertNil(capturedAuthorization)
    }

    func testAnthropicCompleteVisionRequiresAPIKeyBeforeNetwork() async {
        let config = AIConfig(kind: .anthropic, baseURL: "https://api.example.com/v1", model: "m", apiKey: "", systemPrompt: "")
        do {
            _ = try await AIImageClient().completeVision(config: config, image: image, prompt: "read", responseFormat: .json)
            XCTFail("Expected missing API key")
        } catch let error as AIError {
            XCTAssertEqual(error, .missingAPIKey)
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }

    func testCodexProviderReturnsUnsupportedBeforeNetwork() async {
        let config = AIConfig(kind: .codexCLI, baseURL: "", model: "m", apiKey: "", systemPrompt: "")
        do {
            _ = try await AIImageClient().completeVision(config: config, image: image, prompt: "read", responseFormat: .json)
            XCTFail("Expected unsupported provider")
        } catch let error as OCRError {
            XCTAssertEqual(error, .unsupportedProvider("Codex app-server does not support image OCR yet. Use Mac OCR or an image-capable HTTP provider."))
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }
}

private final class LLMOCRMockURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(self, didFailWithError: AIError.transport("No mock response was configured."))
            return
        }

        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}
