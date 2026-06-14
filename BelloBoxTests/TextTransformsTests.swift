import XCTest
@testable import BelloBox

final class TextTransformsTests: XCTestCase {
    // MARK: - Case

    func testCaseConversions() {
        XCTAssertEqual(CaseConverter.convert("hello world", to: .upper), "HELLO WORLD")
        XCTAssertEqual(CaseConverter.convert("Hello World", to: .lower), "hello world")
        XCTAssertEqual(CaseConverter.convert("hello world", to: .title), "Hello World")
        XCTAssertEqual(CaseConverter.convert("hello world", to: .camel), "helloWorld")
        XCTAssertEqual(CaseConverter.convert("hello world", to: .pascal), "HelloWorld")
        XCTAssertEqual(CaseConverter.convert("hello world", to: .snake), "hello_world")
        XCTAssertEqual(CaseConverter.convert("hello world", to: .kebab), "hello-world")
        XCTAssertEqual(CaseConverter.convert("hello world", to: .constant), "HELLO_WORLD")
    }

    func testWordSplittingHandlesCamelAndDelimiters() {
        XCTAssertEqual(CaseConverter.words(in: "fooBarBaz"), ["foo", "Bar", "Baz"])
        XCTAssertEqual(CaseConverter.words(in: "foo_bar-baz qux"), ["foo", "bar", "baz", "qux"])
    }

    // MARK: - Encoding

    func testEncoders() {
        XCTAssertEqual(TextEncoder.encode("Hi", .base64), "SGk=")
        XCTAssertEqual(TextEncoder.encode("Hi", .hex), "4869")
        XCTAssertEqual(TextEncoder.encode("a b&c", .url), "a%20b%26c")
        XCTAssertEqual(TextEncoder.encode("<a>&'\"", .html), "&lt;a&gt;&amp;&#39;&quot;")
    }

    // MARK: - Decoding

    func testExplicitDecoders() {
        XCTAssertEqual(TextDecoder.base64Decode("SGVsbG8="), "Hello")
        XCTAssertEqual(TextDecoder.hexDecode("4869"), "Hi")
        XCTAssertEqual(TextDecoder.urlDecode("a%20b%26c"), "a b&c")
        XCTAssertEqual(TextDecoder.htmlUnescape("&lt;a&gt; &#65; &#x42;"), "<a> A B")
    }

    func testAutoDetectDecoding() {
        XCTAssertEqual(TextDecoder.autoDecode("SGVsbG8gd29ybGQ="), TextDecoder.Decoded(format: "Base64", output: "Hello world"))
        XCTAssertEqual(TextDecoder.autoDecode("%41%42")?.format, "URL")
        XCTAssertEqual(TextDecoder.autoDecode("%41%42")?.output, "AB")
        XCTAssertEqual(TextDecoder.autoDecode("&lt;tag&gt;")?.format, "HTML entities")
        XCTAssertNil(TextDecoder.autoDecode("just plain words here"))
    }

    func testBase64Heuristic() {
        XCTAssertTrue(TextDecoder.looksLikeBase64("SGVsbG8="))
        XCTAssertFalse(TextDecoder.looksLikeBase64("not base64!"))
    }

    // MARK: - Hash

    func testHashes() {
        XCTAssertEqual(HashTool.hash("abc", .md5), "900150983cd24fb0d6963f7d28e17f72")
        XCTAssertEqual(HashTool.hash("abc", .sha1), "a9993e364706816aba3e25717850c26c9cd0d89d")
        XCTAssertEqual(HashTool.hash("abc", .sha256), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    }

    // MARK: - Lines

    func testLineOps() {
        XCTAssertEqual(LineTool.apply("b\na\nb", .dedupe), "b\na")
        XCTAssertEqual(LineTool.apply("b\nA\nc", .sortAscending), "A\nb\nc")
        XCTAssertEqual(LineTool.apply("a\nb", .reverse), "b\na")
        XCTAssertEqual(LineTool.apply("a\n\nb", .removeEmpty), "a\nb")
    }

    // MARK: - Stats

    func testStats() {
        XCTAssertEqual(TextStats.characters("abc"), 3)
        XCTAssertEqual(TextStats.words("a b  c"), 3)
        XCTAssertEqual(TextStats.lines("a\nb\nc"), 3)
        XCTAssertEqual(TextStats.charactersNoSpaces("a b c"), 3)
    }

    // MARK: - Pretty print

    func testPrettyPrintJSON() {
        let result = PrettyPrinter.prettyPrint("{\"a\":1,\"b\":[2,3]}")
        XCTAssertEqual(result?.language, "JSON")
        XCTAssertTrue(result?.output.contains("\n") ?? false)
        XCTAssertTrue(result?.output.contains("  ") ?? false)
    }

    func testPrettyPrintRejectsPlainText() {
        XCTAssertNil(PrettyPrinter.prettyPrint("just some words"))
    }

    // MARK: - Token estimate

    func testTokenFamilyDetection() {
        XCTAssertEqual(TokenEstimator.family(model: "gpt-4o-mini", provider: .openAI), .openAIO200K)
        XCTAssertEqual(TokenEstimator.family(model: "gpt-3.5-turbo", provider: .openAI), .openAICL100K)
        XCTAssertEqual(TokenEstimator.family(model: "claude-3-5-haiku-latest", provider: .anthropic), .anthropic)
    }

    func testTokenEstimateVariesByFamily() {
        let text = String(repeating: "The quick brown fox jumps over the lazy dog. ", count: 5)
        let anthropic = TokenEstimator.estimate(text, family: .anthropic)
        let o200k = TokenEstimator.estimate(text, family: .openAIO200K)
        XCTAssertGreaterThan(anthropic, 0)
        XCTAssertGreaterThan(anthropic, o200k)
    }
}
