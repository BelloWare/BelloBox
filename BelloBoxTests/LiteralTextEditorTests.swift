import AppKit
import XCTest
@testable import BelloBox

@MainActor
final class LiteralTextEditorTests: XCTestCase {
    func testLiteralInputAndUndoPreserveCodeAndUnicode() {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 200),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let editor = LiteralTextView(frame: NSRect(x: 0, y: 0, width: 400, height: 200))
        editor.isRichText = false
        editor.allowsUndo = true
        editor.configureLiteralInput()
        window.contentView = editor
        window.makeFirstResponder(editor)
        XCTAssertFalse(editor.isAutomaticQuoteSubstitutionEnabled)
        XCTAssertFalse(editor.isAutomaticDashSubstitutionEnabled)
        XCTAssertFalse(editor.isAutomaticTextReplacementEnabled)
        XCTAssertFalse(editor.isAutomaticSpellingCorrectionEnabled)
        let text = #"{"message":"Hello 👋","flag":"--verbose","url":"https://example.com"}"#
        editor.insertText(text, replacementRange: NSRange(location: 0, length: 0))
        XCTAssertEqual(editor.string, text)
        editor.breakUndoCoalescing()
        XCTAssertTrue(editor.undoManager?.canUndo == true)
        editor.undoManager?.undo()
        XCTAssertEqual(editor.string, "")
        editor.undoManager?.redo()
        XCTAssertEqual(editor.string, text)
        window.close()
    }
}
