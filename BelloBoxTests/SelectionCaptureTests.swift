import XCTest
@testable import BelloBox

final class SelectionTextResolverTests: XCTestCase {
    func testSelectedTextDoesNotReadTheEntireField() {
        let text = SelectionTextResolver.resolve(direct: "word", range: nil,
            stringForRange: { _ in XCTFail("Unneeded range read"); return nil },
            value: { XCTFail("Unneeded field read"); return nil })
        XCTAssertEqual(text, "word")
    }

    func testRangeOnlySelectionUsesTheParameterizedString() {
        let text = SelectionTextResolver.resolve(direct: "", range: NSRange(location: 5, length: 4),
            stringForRange: { range in XCTAssertEqual(range.location, 5); return "word" },
            value: { XCTFail("No full-field fallback needed"); return nil })
        XCTAssertEqual(text, "word")
    }

    func testValueFallbackUsesUTF16OffsetsAndPreservesUnicode() {
        let value = "👋🏼 Hello café 世界"
        let range = (value as NSString).range(of: "café 世界")
        XCTAssertEqual(SelectionTextResolver.resolve(direct: nil, range: range,
            stringForRange: { _ in nil }, value: { value }), "café 世界")
    }

    func testCaretAndInvalidRangesNeverBecomeFullFieldSelections() {
        for range in [NSRange(location: 2, length: 0), NSRange(location: NSNotFound, length: 4)] {
            XCTAssertNil(SelectionTextResolver.resolve(direct: nil, range: range,
                stringForRange: { _ in XCTFail("Invalid range must not be requested"); return nil },
                value: { XCTFail("A caret is not a selection"); return nil }))
        }
        XCTAssertNil(SelectionTextResolver.resolve(direct: "stale word", range: NSRange(location: 0, length: 0),
            stringForRange: { _ in nil }, value: { "full field" }))
        XCTAssertNil(SelectionTextResolver.resolve(direct: nil, range: NSRange(location: 3, length: Int.max),
            stringForRange: { _ in nil }, value: { "short" }))
        XCTAssertNil(SelectionTextResolver.resolve(direct: nil, range: nil,
            stringForRange: { _ in nil }, value: { XCTFail("No range, no value read"); return "full field" }))
    }
}

@MainActor
final class SelectionRequestTests: XCTestCase {
    private let selection = BelloBox.TextSelection(text: "doubleclicked", anchorRect: nil, appName: "Fixture", bundleID: nil, pid: 42)

    func testSettledSelectionOpensImmediatelyWithoutWaiting() {
        let request = SelectionRequest()
        var result: BelloBox.TextSelection?
        request.start(read: { self.selection }, isCurrent: { true },
            sleep: { _ in XCTFail("An available selection needs no delay") }, completion: { result = $0 })
        XCTAssertEqual(result, selection)
        XCTAssertFalse(request.isPending)
    }

    func testShortcutWaitsForDoubleClickSelectionBeforeOpening() async {
        let request = SelectionRequest()
        var reads = 0
        var delays: [UInt64] = []
        var result: BelloBox.TextSelection?
        request.start(read: { reads += 1; return reads < 3 ? nil : self.selection }, isCurrent: { true },
            sleep: { delays.append($0); await Task.yield() }, completion: { result = $0 })
        XCTAssertNil(result, "Do not steal host focus by opening an empty palette")
        for _ in 0..<30 where request.isPending { await Task.yield() }
        XCTAssertEqual(result, selection)
        XCTAssertEqual(delays, [35_000_000, 65_000_000])
        XCTAssertEqual(reads, 3)
    }

    func testEmptySelectionStillOpensAfterBoundedRetries() async {
        let request = SelectionRequest()
        var reads = 0, opens = 0
        request.start(read: { reads += 1; return nil }, isCurrent: { true },
            sleep: { _ in await Task.yield() }, completion: { XCTAssertNil($0); opens += 1 })
        for _ in 0..<30 where request.isPending { await Task.yield() }
        XCTAssertEqual(reads, 4)
        XCTAssertEqual(opens, 1)
    }

    func testSwitchingSourceAppOrWindowDiscardsThePendingShortcut() async {
        let request = SelectionRequest()
        var current = true, reads = 0, opens = 0
        request.start(read: { reads += 1; return reads == 1 ? nil : self.selection }, isCurrent: { current },
            sleep: { _ in current = false; await Task.yield() }, completion: { _ in opens += 1 })
        for _ in 0..<30 where request.isPending { await Task.yield() }
        XCTAssertEqual(reads, 1)
        XCTAssertEqual(opens, 0)
        XCTAssertFalse(request.isPending)
    }

    func testCancellationCannotOpenALatePalette() async {
        let request = SelectionRequest()
        var opens = 0
        request.start(read: { nil }, isCurrent: { true }, sleep: { _ in await Task.yield() }, completion: { _ in opens += 1 })
        request.cancel()
        for _ in 0..<10 { await Task.yield() }
        XCTAssertEqual(opens, 0)
        XCTAssertFalse(request.isPending)
    }
}
