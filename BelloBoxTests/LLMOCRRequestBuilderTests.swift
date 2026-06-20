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
    }

    func testOpenAIChatPayloadContainsImageURL() {
        let config = AIConfig(kind: .openAI, baseURL: "https://api.example.com/v1", model: "m", apiKey: "secret", systemPrompt: "")
        let body = AIImageClient.openAIChatVisionBody(config: config, image: image, prompt: "read", stream: false)
        let messages = body["messages"] as? [[String: Any]]
        let content = messages?.first?["content"] as? [[String: Any]]
        XCTAssertEqual(content?.last?["type"] as? String, "image_url")
    }

    func testAnthropicPayloadContainsImageBlock() {
        let config = AIConfig(kind: .anthropic, baseURL: "https://api.example.com/v1", model: "m", apiKey: "secret", systemPrompt: "")
        let body = AIImageClient.anthropicVisionBody(config: config, image: image, prompt: "read")
        let messages = body["messages"] as? [[String: Any]]
        let content = messages?.first?["content"] as? [[String: Any]]
        XCTAssertEqual(content?.first?["type"] as? String, "image")
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

