import XCTest
@testable import BelloBox

/// Compare's rows must not depend on the macOS that runs it. The inputs are the
/// Rust port's synthetic TextComparison oracle (115 cases, SHA-256 6af24b2d…fddfb);
/// the expected rows are Swift 6.3.3's alignment, cross-checked against the Rust
/// port of the same algorithm. The macOS 14 runtime differs on five of them.
final class ComparisonCorpusTests: XCTestCase {
    private struct Case: Decodable {
        let id: String
        let mode: String
        let ignoreWhitespace: Bool
        let left: String
        let right: String
        let rows: [[String]]?
        let copy: String?
        let error: String?
    }

    private func corpus() throws -> [Case] {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "comparison-corpus", withExtension: "json"))
        return try JSONDecoder().decode([Case].self, from: Data(contentsOf: url))
    }

    func testComparisonCorpusMatchesSwift633Rows() throws {
        let cases = try corpus()
        XCTAssertEqual(cases.count, 115)
        for item in cases {
            let mode = try XCTUnwrap(ComparisonMode(rawValue: item.mode), item.id)
            let result: BelloBox.ComparisonResult
            do {
                result = try TextComparison.compare(item.left, item.right, mode: mode, ignoreWhitespace: item.ignoreWhitespace)
            } catch {
                XCTAssertEqual(error.localizedDescription, item.error, item.id)
                continue
            }
            XCTAssertNil(item.error, "\(item.id) should fail")
            // Exact scalars: String == would accept a canonically equivalent spelling.
            let rows = result.rows.map { row -> [String] in
                let kind: String
                switch row.kind { case .same: kind = "same"; case .added: kind = "added"; case .removed: kind = "removed" }
                return [kind, row.text]
            }
            XCTAssertEqual(rows.map { $0.map { Array($0.unicodeScalars) } }, item.rows?.map { $0.map { Array($0.unicodeScalars) } }, item.id)
            XCTAssertEqual(Array(result.text.utf8), item.copy.map { Array($0.utf8) }, item.id)
            XCTAssertEqual(result.added, item.rows?.filter { $0[0] == "added" }.count, item.id)
            XCTAssertEqual(result.removed, item.rows?.filter { $0[0] == "removed" }.count, item.id)
        }
    }

    func testBlankRowsAlignLikeSwift633() throws {
        // macOS 14's runtime kept the second blank line unchanged and added the last.
        let result = try TextComparison.compare("a\n\nb\n", "a\nb\n\n", mode: .lines, ignoreWhitespace: false)
        XCTAssertEqual(result.text, "  a\n− \n  b\n+ \n  ")
    }

    func testDifferenceIsMinimalAndReconstructsBothSides() {
        // Exhaustive over short strings of a two-letter alphabet: every alignment
        // is a shortest edit script and rebuilds both inputs.
        func strings(_ length: Int) -> [[Character]] {
            length == 0 ? [[]] : strings(length - 1).flatMap { [$0 + ["a"], $0 + ["b"]] }
        }
        func lcs(_ a: [Character], _ b: [Character]) -> Int {
            var table = [[Int]](repeating: [Int](repeating: 0, count: b.count + 1), count: a.count + 1)
            for i in 0..<a.count { for j in 0..<b.count {
                table[i + 1][j + 1] = a[i] == b[j] ? table[i][j] + 1 : max(table[i][j + 1], table[i + 1][j])
            } }
            return table[a.count][b.count]
        }
        let all = (0...5).flatMap(strings)
        for a in all { for b in all {
            let (removed, inserted) = MyersDifference.changes(from: a, to: b, by: ==)
            XCTAssertEqual(removed.filter { $0 }.count + inserted.filter { $0 }.count, a.count + b.count - 2 * lcs(a, b), "\(a) → \(b)")
            let keptA = a.indices.filter { !removed[$0] }.map { a[$0] }
            let keptB = b.indices.filter { !inserted[$0] }.map { b[$0] }
            XCTAssertEqual(keptA, keptB, "\(a) → \(b)")
        } }
    }
}
