import XCTest
@testable import BelloBox

final class LLMOCRResponseParserTests: XCTestCase {
    func testParsesJSONResponse() throws {
        let parsed = try AIImageClient.parseLLMOCRResponse(##"{"plainText":"hello","markdownText":"# hello","warnings":["unclear"]}"##)
        XCTAssertEqual(parsed.plainText, "hello")
        XCTAssertEqual(parsed.markdownText, "# hello")
        XCTAssertEqual(parsed.warnings, ["unclear"])
    }

    func testFallsBackToPlainText() throws {
        let parsed = try AIImageClient.parseLLMOCRResponse("plain text")
        XCTAssertEqual(parsed.plainText, "plain text")
        XCTAssertEqual(parsed.warnings, ["Provider returned non-JSON OCR text."])
    }

    func testParsesFencedJSONResponse() throws {
        let parsed = try AIImageClient.parseLLMOCRResponse(
            """
            ```json
            {"plainText":"hello","markdownText":"**hello**"}
            ```
            """
        )

        XCTAssertEqual(parsed.plainText, "hello")
        XCTAssertEqual(parsed.markdownText, "**hello**")
        XCTAssertTrue(parsed.warnings.isEmpty)
    }

    func testExtractsOpenAIResponsesOutputArrayText() throws {
        let data = Data(
            """
            {"output":[{"content":[{"type":"output_text","text":"hello"},{"type":"output_text","text":"world"}]}]}
            """.utf8
        )
        let config = AIConfig(kind: .openAI, baseURL: "https://api.example.com/v1", model: "m", apiKey: "secret", systemPrompt: "")

        XCTAssertEqual(try AIImageClient.extractText(from: data, config: config), "hello\nworld")
    }

    func testEmptyResponseThrows() {
        XCTAssertThrowsError(try AIImageClient.parseLLMOCRResponse("   "))
    }

    func testVisionUploadCancellationStaysCancellationError() async throws {
        ImageClientMockURLProtocol.handler = { _ in throw URLError(.cancelled) }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ImageClientMockURLProtocol.self]
        let client = AIImageClient(session: URLSession(configuration: configuration))
        let config = AIConfig(kind: .openAI, baseURL: "https://api.example.com/v1", model: "m", apiKey: "secret", systemPrompt: "")
        let image = AIImagePayload(data: Data([0x89, 0x50, 0x4E, 0x47]), mimeType: "image/png", width: 1, height: 1, digest: "d")

        do {
            _ = try await client.completeVision(config: config, image: image, prompt: "ocr", responseFormat: .json)
            XCTFail("Expected cancellation.")
        } catch is CancellationError {
            // Expected.
        } catch {
            XCTFail("Expected CancellationError, got \(error).")
        }
    }
}

private final class ImageClientMockURLProtocol: URLProtocol {
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
