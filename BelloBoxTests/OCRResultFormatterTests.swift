import XCTest
@testable import BelloBox

final class OCRResultFormatterTests: XCTestCase {
    func testLinesSortTopToBottomAndLeftToRight() {
        let regions = [
            OCRTextRegion(kind: .line, text: "right", boundingBox: CGRectCodable(CGRect(x: 80, y: 20, width: 40, height: 10))),
            OCRTextRegion(kind: .line, text: "top", boundingBox: CGRectCodable(CGRect(x: 10, y: 20, width: 40, height: 10))),
            OCRTextRegion(kind: .line, text: "bottom", boundingBox: CGRectCodable(CGRect(x: 10, y: 80, width: 40, height: 10))),
        ]
        XCTAssertEqual(OCRResultFormatter.plainText(from: regions), "top\nright\n\nbottom")
    }

    func testMarkdownPrefersLLMMarkdown() {
        let result = OCRResult(id: UUID(), engine: .llm(provider: .openAI, model: "m"), target: .fullImage, plainText: "plain", markdownText: "**md**", regions: [], languageHints: [], imageDigest: "d", warnings: [], createdAt: Date())
        XCTAssertEqual(OCRResultFormatter.markdown(from: result), "**md**")
    }
}

