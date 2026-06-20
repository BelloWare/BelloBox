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

    func testEmptyResponseThrows() {
        XCTAssertThrowsError(try AIImageClient.parseLLMOCRResponse("   "))
    }
}
