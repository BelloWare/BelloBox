import XCTest
@testable import BelloBox

final class LauncherUsageTests: XCTestCase {
    private var defaults: UserDefaults!
    private var suite: String!
    private let now = Date(timeIntervalSince1970: 1_800_000_000)

    override func setUp() {
        suite = "LauncherUsageTests.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)!
    }
    override func tearDown() { defaults.removePersistentDomain(forName: suite) }

    private func ranked(_ text: String, query: String = "", date: Date? = nil) -> [LauncherCommand] {
        let context = LauncherSelectionContext(text: text)
        return LauncherCommand.search(query, input: "", favorites: ["json", "worldClock", "compare"], recents: [],
            suggested: context.suggestions,
            learnedScores: LauncherUsageStore(defaults: defaults).scores(for: context.contentKind, now: date ?? now))
    }

    func testRepeatedChoicesBecomeTheDefaultOnlyForTheirContentKind() {
        let store = LauncherUsageStore(defaults: defaults)
        XCTAssertEqual(ranked("A short passage").first, .textTools)
        store.record(.ai, kind: .text, now: now)
        XCTAssertEqual(ranked("Another passage").first, .textTools, "One use should not overwhelm the initial interpretation")
        for _ in 0..<3 { store.record(.ai, kind: .text, now: now) }
        XCTAssertEqual(ranked("Completely different prose").first, .ai)
        XCTAssertEqual(ranked("{\"name\":\"example\"}").first, .json)
        XCTAssertEqual(ranked("https://example.com/path").first, .url)
        XCTAssertEqual(ranked("2026-09-08T12:00:00Z").first, .worldClock)
        XCTAssertNotEqual(ranked("").first, .ai)
    }

    func testTimestampPreferenceAndExplicitSearchStayIndependent() {
        let store = LauncherUsageStore(defaults: defaults)
        for _ in 0..<4 { store.record(.time, kind: .timestamp, now: now) }
        let text = "2026-09-08T12:00:00Z"
        XCTAssertEqual(ranked(text).first, .time)
        XCTAssertEqual(ranked(text, query: "clock").first, .worldClock)
        XCTAssertEqual(ranked(text, query: "time").first, .time)
        for _ in 0..<4 { store.record(.recording, kind: .empty, now: now) }
        XCTAssertEqual(ranked("", query: "gif").first, .videoToGIF, "Title match beats a learned keyword match")
        XCTAssertTrue(ranked(text, query: "unmatched-tool").isEmpty)
    }

    func testPreferencesPersistDecayAndCanBeResetWithoutClearingFavorites() {
        for _ in 0..<10 { LauncherUsageStore(defaults: defaults).record(.ai, kind: .text, now: now) }
        let store = LauncherUsageStore(defaults: defaults)
        XCTAssertEqual(store.scores(for: .text, now: now)["ai"], 1_400, "Counters are capped")
        XCTAssertEqual(store.scores(for: .text, now: now.addingTimeInterval(LauncherUsageStore.halfLife))["ai"], 700)
        XCTAssertEqual(ranked("A passage", date: now.addingTimeInterval(LauncherUsageStore.halfLife)).first, .textTools)
        defaults.set(["json"], forKey: "launcherFavorites")
        store.reset()
        XCTAssertTrue(store.scores(for: .text, now: now).isEmpty)
        XCTAssertNil(defaults.data(forKey: LauncherUsageStore.defaultsKey))
        XCTAssertEqual(defaults.stringArray(forKey: "launcherFavorites"), ["json"])
    }

    func testMalformedOrUnknownHistoryCannotPolluteRanking() throws {
        let store = LauncherUsageStore(defaults: defaults)
        defaults.set(Data("not json".utf8), forKey: LauncherUsageStore.defaultsKey)
        XCTAssertTrue(store.scores(for: .text, now: now).isEmpty)
        let entry: [String: Any] = ["weight": 9_000, "lastUsed": now.timeIntervalSince1970]
        defaults.set(try JSONSerialization.data(withJSONObject: ["text": ["ai": entry, "removedTool": entry, "settings": entry], "unknown": ["ai": entry]]), forKey: LauncherUsageStore.defaultsKey)
        XCTAssertEqual(store.scores(for: .text, now: now), ["ai": 1_400])
        store.record(.json, kind: .json, now: now)
        let data = try XCTUnwrap(defaults.data(forKey: LauncherUsageStore.defaultsKey))
        let saved = try XCTUnwrap(String(data: data, encoding: .utf8))
        XCTAssertFalse(saved.contains("removedTool"))
        XCTAssertFalse(saved.contains("unknown"))
        XCTAssertFalse(saved.contains("settings"))
        defaults.set(Data(repeating: 0, count: 32_769), forKey: LauncherUsageStore.defaultsKey)
        XCTAssertTrue(store.scores(for: .text, now: now).isEmpty)
    }

    func testCategoriesAreCoarseAndRejectedInputNeverTeaches() {
        let samples: [(String, LauncherContentKind)] = [("", .empty), ("hello", .text), ("{}", .json),
            ("https://example.com/private", .url), ("eyJabc.abc.def", .jwt), ("0 9 * * 1-5", .cron),
            ("curl https://example.com", .curl), ("name,value\na,1", .data), ("2026-09-08T12:00:00Z", .timestamp)]
        for (text, kind) in samples { XCTAssertEqual(LauncherSelectionContext(text: text).contentKind, kind) }
        let context = LauncherSelectionContext(text: String(repeating: "x", count: UtilityLimits.inputBytes + 1))
        XCTAssertNil(context.contentKind)
        let store = LauncherUsageStore(defaults: defaults)
        store.record(.ai, kind: context.contentKind, now: now)
        store.record(.settings, kind: .text, now: now)
        store.record(.home, kind: .empty, now: now)
        XCTAssertNil(defaults.data(forKey: LauncherUsageStore.defaultsKey))
    }

    @MainActor
    func testOnlyOpeningToolsTeachesAndTheNextPaletteSelectsTheLearnedPreview() throws {
        let secret = "private selection that must never enter usage storage"
        func model(_ text: String) -> LauncherModel {
            LauncherModel(selection: TextSelection(text: text, anchorRect: nil, appName: "Private app", bundleID: "private.bundle", pid: 123),
                          snippets: SnippetStore(), defaults: defaults)
        }
        for _ in 0..<4 {
            let palette = model(secret)
            palette.query = "ai"
            palette.move(1)
            palette.move(-1)
            XCTAssertEqual(palette.selectedCommand, .ai)
            palette.openSelected()
            palette.cancelAll()
        }
        let next = model("Another passage")
        defer { next.cancelAll() }
        XCTAssertEqual(next.selectedCommand, .ai)
        XCTAssertEqual(next.bestMatch, .ai)
        XCTAssertEqual(next.expandedCommand, .ai)
        next.useClipboard("{}")
        XCTAssertEqual(next.selectedCommand, .json, "Replacing input recalculates category preferences")
        let data = try XCTUnwrap(defaults.data(forKey: LauncherUsageStore.defaultsKey))
        let stored = try XCTUnwrap(String(data: data, encoding: .utf8))
        for forbidden in [secret, "Private app", "private.bundle", "Another passage"] { XCTAssertFalse(stored.contains(forbidden)) }
        LauncherUsageStore(defaults: defaults).reset()
        let browsing = model(secret)
        browsing.query = "ai"
        browsing.move(1)
        browsing.cancelAll()
        XCTAssertNil(defaults.data(forKey: LauncherUsageStore.defaultsKey), "Focus and previews do not count as use")
    }
}
