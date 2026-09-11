import XCTest
@testable import BelloBox

final class AISelectionLayoutTests: XCTestCase {
    func testShortSelectionsLeaveRoomForPromptAndWritingActions() {
        let short = ActionPopupView.selectionPreviewHeight(text: "Selected words", width: 696)
        let passage = ActionPopupView.selectionPreviewHeight(text: String(repeating: "A longer passage wraps onto another line. ", count: 20), width: 696)
        XCTAssertLessThan(short, passage)
        XCTAssertLessThanOrEqual(short, 40)
        XCTAssertLessThanOrEqual(passage, 120)
    }

    func testLongAndMultilineSelectionsStayBoundedOnNarrowWindows() {
        for text in [String(repeating: "Long selection ", count: 50_000), String(repeating: "line\n", count: 500), String(repeating: "🌏你好世界", count: 1_000)] {
            let wide = ActionPopupView.selectionPreviewHeight(text: text, width: 696)
            let narrow = ActionPopupView.selectionPreviewHeight(text: text, width: 300)
            XCTAssertGreaterThanOrEqual(narrow, wide)
            XCTAssertLessThanOrEqual(narrow, 120)
            XCTAssertGreaterThanOrEqual(narrow, 32)
        }
    }
}
