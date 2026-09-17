import AppKit
import SwiftUI
import XCTest
@testable import BelloBox

final class LauncherPreviewTests: XCTestCase {
    private let utc = TimeZone(secondsFromGMT: 0)!
    private let locale = Locale(identifier: "en_US_POSIX")
    private let now = Date(timeIntervalSince1970: 1_788_782_400) // 2026-09-07 12:00 UTC

    private func preview(_ text: String, _ command: LauncherCommand, context: LauncherPreviewContext = LauncherPreviewContext(zoneIDs: ["UTC"])) throws -> LauncherPreview {
        try LauncherPreview.make(text: text, command: command, context: context, now: now, localZone: utc, locale: locale)
    }

    func testBestInterpretationOutranksFavoriteAndRecentAlternatives() {
        let examples: [(String, LauncherCommand)] = [
            ("{}", .json), ("2026-09-07T12:00:00Z", .worldClock),
            ("HTTPS://example.com/path", .url), ("curl https://example.com", .http),
            ("0 9 * * MON-FRI", .cron), ("name,role\nBello,utility", .convert),
            ("Hello there", .textTools)
        ]
        for (input, expected) in examples {
            let others = LauncherCommand.allCases.filter { $0 != expected }.map(\.id)
            XCTAssertEqual(LauncherCommand.search("", input: input, favorites: Set(others), recents: others).first,
                           expected, input)
        }
        XCTAssertEqual(LauncherCommand.search("timestamp", input: "2026-09-07T12:00:00Z", favorites: [], recents: []).first, .time)
        XCTAssertEqual(LauncherCommand.search("", input: "", favorites: ["regex"], recents: []).first, .regex)
    }

    func testTitleWordsOutrankKeywordMatchesSuggestionsAndFavoritesButNotPrefixes() {
        let gif = LauncherCommand.search("gif", input: "", favorites: ["recording"], recents: ["recording"])
        XCTAssertEqual(gif.first, .videoToGIF, "A tool named after the query beats one that only mentions it")
        XCTAssertTrue(gif.contains(.recording), "The keyword match stays searchable")
        XCTAssertEqual(LauncherCommand.search("screen", input: "", favorites: [], recents: []).prefix(2).map { $0 }, [.screenshot, .recording],
                       "Two prefix matches keep the catalog order")
        XCTAssertEqual(LauncherCommand.search("clock", input: "", favorites: ["time"], recents: []).first, .worldClock)
        XCTAssertEqual(LauncherCommand.search("convert", input: "2026-09-07T12:00:00Z", favorites: [], recents: []).first, .convert,
                       "A title prefix beats a title word plus a suggestion bonus")
        XCTAssertEqual(LauncherCommand.search("time", input: "2026-09-07T12:00:00Z", favorites: [], recents: []).first, .time,
                       "Timestamp Converter has the prefix; World Clock only has the suggestion")
        XCTAssertEqual(LauncherCommand.search("", input: "2026-09-07T12:00:00Z", favorites: ["time"], recents: ["time"]).first, .worldClock,
                       "Without a query the best interpretation still leads")
    }

    func testClockPreviewUsesSelectedMomentAndDSTInEachSavedZone() throws {
        let value = try LauncherPreview.make(text: "2026-07-07T12:00:00Z", command: .worldClock,
            context: LauncherPreviewContext(zoneIDs: ["America/New_York", "Europe/London", "Asia/Singapore", "Asia/Tokyo", "UTC"]),
            now: now, localZone: utc, locale: locale)
        guard case .clocks(let clocks, let instant) = value.content else { return XCTFail("Expected clocks") }
        XCTAssertEqual(clocks.count, 4)
        XCTAssertEqual(instant, Date(timeIntervalSince1970: 1_783_425_600)) // 2026-07-07 12:00 UTC
        XCTAssertEqual(clocks.map(\.zone), ["UTC−04:00", "UTC+01:00", "UTC+08:00", "UTC+09:00"])
        XCTAssertTrue(clocks[0].time.hasPrefix("8:00:00"))
        XCTAssertTrue(clocks[0].date.contains("2026"))
        XCTAssertEqual(clocks[0].quality, .extended)
        let winter = try LauncherPreview.make(text: "2026-01-07T12:00:00Z", command: .worldClock,
            context: LauncherPreviewContext(zoneIDs: ["America/New_York"]), now: now, localZone: utc, locale: locale)
        guard case .clocks(let winterClocks, _) = winter.content else { return XCTFail("Expected clocks") }
        XCTAssertEqual(winterClocks[0].zone, "UTC−05:00")
        XCTAssertEqual(winterClocks.count, 2, "UTC fallback is deduplicated")
        XCTAssertEqual(WorldClockViewModel.previewZoneIDs(saved: ["Etc/UTC", "bad/zone"], localZone: utc), ["UTC"])
    }

    func testJSONPreviewPreservesLargeIntegerAndBoundsLongUnicodeOutput() throws {
        let value = try preview("{\"id\":900719925474099312345}", .json)
        guard case .code(let output) = value.content else { return XCTFail("Expected JSON") }
        XCTAssertTrue(output.contains("900719925474099312345"))
        XCTAssertFalse(value.isWarning)
        let long = try preview("{\"text\":\"" + String(repeating: "🦊", count: 2_000) + "\"}", .json)
        guard case .code(let excerpt) = long.content else { return XCTFail("Expected excerpt") }
        XCTAssertLessThanOrEqual(excerpt.unicodeScalars.count, 902)
        XCTAssertTrue(long.subtitle.contains("excerpt"))
    }

    func testJWTPreviewMakesUnverifiedSignatureVisible() throws {
        func encode(_ value: String) -> String {
            Data(value.utf8).base64EncodedString().replacingOccurrences(of: "=", with: "")
                .replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_")
        }
        let token = encode("{\"alg\":\"HS256\"}") + "." + encode("{\"sub\":\"demo\"}") + ".c2ln"
        XCTAssertEqual(LauncherCommand.suggestions(for: token), [.jwt])
        let value = try preview(token, .jwt)
        XCTAssertTrue(value.isWarning)
        XCTAssertEqual(value.subtitle, "Signature not verified · excerpt")
        XCTAssertTrue(value.accessibilitySummary.contains("Signature not verified"))
    }

    func testURLPreviewKeepsDuplicateParametersAndFlags() throws {
        let value = try preview("https://example.com:8443/docs?a=one&a=two&flag#part", .url)
        guard case .fields(let fields) = value.content else { return XCTFail("Expected fields") }
        XCTAssertEqual(fields.map(\.value), ["example.com:8443", "/docs", "one", "two"])
        XCTAssertTrue(value.subtitle.contains("3 query parameters"))
        let flag = try preview("https://example.com/?flag", .url)
        guard case .fields(let flagFields) = flag.content else { return XCTFail("Expected flag") }
        XCTAssertEqual(flagFields.last?.value, "(flag)")
    }

    func testCurlPreviewSummarizesDraftWithoutExposingHeaderContents() throws {
        let value = try preview("curl -X POST https://example.com -H 'Authorization: Bearer example-secret' -d 'hello'", .http)
        guard case .fields(let fields) = value.content else { return XCTFail("Expected request") }
        XCTAssertEqual(fields.map(\.value), ["POST", "https://example.com", "2", "5 bytes"], "Includes the inferred Content-Type header")
        XCTAssertFalse(value.accessibilitySummary.contains("example-secret"))
        XCTAssertThrowsError(try preview("curl https://example.com -d @/etc/passwd", .http))
    }

    func testCronShowsThreeLocalUpcomingRuns() throws {
        let value = try preview("0 9 * * MON-FRI", .cron)
        guard case .fields(let fields) = value.content else { return XCTFail("Expected runs") }
        XCTAssertEqual(fields.map(\.label), ["Next 1", "Next 2", "Next 3"])
        XCTAssertEqual(Set(fields.map(\.value)).count, 3)
        XCTAssertTrue(value.subtitle.contains("5-field"))
    }

    func testDataDetectionAndPreviewHandleCSVURLsYAMLAndTSV() throws {
        let examples: [(String, DataFormat)] = [
            ("  {\"ready\":true}", .json), ("name,url\nBello,https://example.com", .csv),
            ("name: Bello\nready: true", .yaml), ("name\trole\nBello\tutility", .csv)
        ]
        for (input, format) in examples {
            XCTAssertEqual(DataConversion.detectFormat(input), format)
            let value = try preview(input, .convert)
            XCTAssertEqual(value.title, "\(format.rawValue) recognized")
            guard case .code = value.content else { return XCTFail("Expected converted JSON") }
        }
    }

    func testPlainTextCountsCRLFAsOneLineBreakAndCountsGraphemes() throws {
        let value = try preview("Hi 🦊\r\nBye", .textTools)
        guard case .statistics(let fields) = value.content else { return XCTFail("Expected statistics") }
        XCTAssertEqual(fields.first(where: { $0.label == "Words" })?.value, "3")
        XCTAssertEqual(fields.first(where: { $0.label == "Lines" })?.value, "2")
        XCTAssertEqual(fields.first(where: { $0.label == "Characters" })?.value, "8")
    }

    func testLargeInputsSkipParsingAndOverLimitInputsAreRejected() throws {
        let text = "{" + String(repeating: "x", count: LauncherPreview.parsingByteLimit)
        let value = try preview(text, .json)
        guard case .notice(let notice) = value.content else { return XCTFail("Expected large-input notice") }
        XCTAssertFalse(value.isWarning, "Do not parse or misreport a partial document")
        XCTAssertTrue(notice.contains("complete selection"))
        guard case .statistics = try preview(text, .textTools).content else { return XCTFail("Counting never needs the parse limit") }
        XCTAssertThrowsError(try preview(String(repeating: "x", count: UtilityLimits.inputBytes + 1), .textTools))
    }

    func testEveryCommandHasAUsefulPreviewWithAndWithoutASelection() throws {
        for command in LauncherCommand.allCases {
            let empty = try preview("", command)
            let text: LauncherPreview
            do { text = try preview("Just some words", command) } catch { text = LauncherPreview.failure(error, command: command) }
            for value in [empty, text] {
                XCTAssertFalse(value.title.isEmpty, "\(command)")
                if case .actions(let actions) = value.content {
                    XCTAssertTrue((1...3).contains(actions.count), "\(command) keeps its actions concise")
                    XCTAssertTrue(actions.allSatisfy { !$0.text.isEmpty && !$0.symbol.isEmpty })
                }
            }
            XCTAssertFalse(empty.isWarning, "No selection is not an error for \(command)")
        }
        guard case .actions(let json) = try preview("", .json).content else { return XCTFail("Expected actions") }
        XCTAssertTrue(json.last?.text.contains("empty input") == true, "Developer tools explain that they open empty")
        guard case .actions(let screenshot) = try preview("Just some words", .screenshot).content else { return XCTFail("Expected actions") }
        XCTAssertFalse(screenshot.contains { $0.text.contains("words") }, "Capture tools never pretend to transform the text")
        XCTAssertEqual(try preview("", .worldClock).title, "Current time")
        XCTAssertEqual(try preview("Just some words", .worldClock).title, "Current time", "World Clock shows now instead of an error")
        guard case .clocks(let clocks, _) = try preview("Just some words", .worldClock).content else { return XCTFail("Expected clocks") }
        XCTAssertFalse(clocks.isEmpty)
        XCTAssertThrowsError(try preview("Just some words", .time), "The converter needs a timestamp")
    }

    func testTimestampConverterQRAndRequestPreviewsShowConcreteFacts() throws {
        let time = try preview("1788782400", .time)
        guard case .fields(let fields) = time.content else { return XCTFail("Expected fields") }
        XCTAssertEqual(fields.map(\.label), ["Local", "UTC", "Unix"])
        XCTAssertEqual(fields[1].value, "2026-09-07T12:00:00Z")
        XCTAssertTrue(fields[2].value.hasPrefix("1788782400 s"))

        let qr = try preview("https://example.com", .qr)
        XCTAssertFalse(qr.isWarning)
        guard case .fields(let qrFields) = qr.content else { return XCTFail("Expected fields") }
        XCTAssertEqual(qrFields.first?.value, "https://example.com")
        XCTAssertTrue(qrFields.last?.value.contains("19 bytes") == true)
        let tooLong = try preview(String(repeating: "a", count: QRCodeGenerator.maxByteCount + 1), .qr)
        XCTAssertTrue(tooLong.isWarning)

        let request = try preview("https://example.com/api", .http)
        guard case .fields(let requestFields) = request.content else { return XCTFail("Expected fields") }
        XCTAssertEqual(requestFields.map(\.value), ["GET", "https://example.com/api", "example.com"])
        XCTAssertTrue(request.subtitle.contains("Nothing is sent"))

        let snippet = try preview("Dear {{name}}, {{selection}}", .snippets, context: LauncherPreviewContext(snippetCount: 2))
        guard case .actions(let actions) = snippet.content else { return XCTFail("Expected actions") }
        XCTAssertTrue(actions[1].text.contains("2 fields"))
        XCTAssertTrue(actions[2].text.contains("2 snippets"))
        let ai = try preview("hi", .ai, context: LauncherPreviewContext(aiProviderName: "OpenAI"))
        XCTAssertEqual(ai.subtitle, "Provider: OpenAI")
        XCTAssertTrue(try preview("hi", .ai).subtitle.contains("No AI provider"))
    }
}

private final class SentInstants: @unchecked Sendable {
    private let lock = NSLock()
    private(set) var all: [Date] = []
    func append(_ date: Date) { lock.lock(); all.append(date); lock.unlock() }
}

@MainActor
final class LauncherPreviewLifecycleTests: XCTestCase {
    private var defaults: UserDefaults!
    private var suite: String!
    override func setUp() {
        super.setUp()
        suite = "LauncherPreview.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)!
    }
    override func tearDown() {
        defaults.removePersistentDomain(forName: suite)
        super.tearDown()
    }
    private func model(_ text: String, settings: AppSettings? = nil, clockResponder: WorldClockCopilotSession.Responder? = nil,
                       builder: LauncherModel.PreviewBuilder? = nil) -> LauncherModel {
        let selection = TextSelection(text: text, anchorRect: nil, appName: "Editor", bundleID: nil, pid: 123)
        let settings = settings ?? AppSettings(defaults: defaults)
        if let builder {
            return LauncherModel(selection: selection, snippets: SnippetStore(), defaults: defaults, settings: settings,
                                 clockResponder: clockResponder, previewBuilder: builder)
        }
        return LauncherModel(selection: selection, snippets: SnippetStore(), defaults: defaults, settings: settings, clockResponder: clockResponder)
    }
    private func waitUntil(_ predicate: () -> Bool) async throws {
        for _ in 0..<200 {
            if predicate() { return }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
        XCTFail("Preview did not settle")
    }

    func testPreviewIsCachedDuringNavigationAndOnlyEnterOpensTool() async throws {
        let model = model("{\"id\":42}")
        var opened: [UtilityWorkbenchModel] = []
        model.onOpenWorkbench = { opened.append($0) }
        defer { opened.forEach { $0.cancel() } }
        defer { model.cancelAll() }
        try await waitUntil { model.expandedPreview != nil }
        let preview = model.expandedPreview
        XCTAssertEqual(model.selectedCommand, .json)
        XCTAssertEqual(model.bestMatch, .json)
        XCTAssertNil(opened.last)
        XCTAssertNil(defaults.object(forKey: "launcherRecents"))
        let size = model.paletteSize
        model.move(1)
        XCTAssertEqual(model.expandedCommand, .compare, "Focus expands the focused row")
        XCTAssertEqual(model.bestMatch, .json, "The best match label stays on the top interpretation")
        XCTAssertEqual(LauncherModel.previewHeight(for: .json), LauncherModel.previewHeight(for: .compare))
        XCTAssertEqual(model.paletteSize, size, "Rows of the same preview height never jump the palette")
        XCTAssertNil(model.expandedPreview, "The new row's preview is prepared, not faked from the previous one")
        try await waitUntil { model.expandedPreview != nil }
        XCTAssertEqual(model.expandedPreview?.title, "Ready to compare")
        model.move(-1)
        XCTAssertEqual(model.expandedPreview, preview, "Returning to a row shows its cached preview at once")
        model.query = "regex"
        XCTAssertNil(model.bestMatch)
        XCTAssertEqual(model.expandedCommand, .regex, "A search focuses its top result, which expands")
        XCTAssertEqual(model.preview(for: .json), preview)
        try await waitUntil { model.expandedPreview != nil }
        XCTAssertEqual(model.expandedPreview?.title, "Test text ready")
        model.query = ""
        XCTAssertEqual(model.bestMatch, .json)
        XCTAssertEqual(model.expandedPreview, preview)
        model.openSelected()
        XCTAssertEqual(opened.last?.input, "{\"id\":42}")
        XCTAssertEqual(model.expandedPreview, preview)
        XCTAssertEqual(defaults.stringArray(forKey: "launcherRecents"), ["json"], "Only Enter records a recent")
        XCTAssertEqual(model.previews.count, 3, "One cached preview per focused command")
    }

    func testFocusingWorldClockWithoutATimestampShowsALivePlannerThatOpensLiveUnlessScrubbed() async throws {
        let model = model("Hello there")
        defer { model.cancelAll() }
        var opened: LauncherCommandContext?
        model.onCommand = { _, _, context in opened = context }
        model.query = "world"
        XCTAssertEqual(model.expandedCommand, .worldClock)
        XCTAssertTrue(model.expandsClock, "The planner height is reserved for World Clock even without a timestamp")
        XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.clockPreviewHeight)
        try await waitUntil { model.expandedPreview != nil }
        XCTAssertEqual(model.expandedPreview?.title, "Current time")
        let clock = try XCTUnwrap(model.clockPreview, "A live planner is installed without a timestamp")
        XCTAssertTrue(clock.isFollowingNow)
        XCTAssertNil(clock.seedInstant)
        XCTAssertTrue(model.featuresClock, "Arrow keys can still scrub the live planner")
        XCTAssertNil(model.previewedInstant, "Following the current time is not a chosen instant")
        let live = try XCTUnwrap(model.worldClockHandoff, "A live preview hands over explicit live intent")
        XCTAssertTrue(live.followsNow)
        XCTAssertNil(live.instant)
        XCTAssertEqual(live.anchorZoneID, clock.anchorZoneID, "The shown reference travels")
        XCTAssertEqual(live.copilot?.isEmpty, true, "An empty transcript still replaces an older one")
        model.openSelected()
        XCTAssertEqual(opened?.worldClock?.followsNow, true, "Without a timestamp or a scrub the window follows now")
        clock.nudge(by: 2)
        XCTAssertFalse(clock.isFollowingNow)
        XCTAssertEqual(model.previewedInstant, clock.selectedInstant)
        model.openSelected()
        XCTAssertEqual(opened?.worldClock?.instant, clock.selectedInstant, "A scrubbed live planner opens at the scrubbed time")
        XCTAssertEqual(opened?.worldClock?.followsNow, false)
        clock.goToNow()
        XCTAssertTrue(clock.isFollowingNow)
        model.openSelected()
        XCTAssertEqual(opened?.worldClock?.followsNow, true, "Returning to now opens live again")
        XCTAssertNil(opened?.worldClock?.instant)
    }

    func testTimestampSelectionReservesThePlannerHeightBeforeItArrivesAndFocusMovesReportWithoutRefocus() async throws {
        let model = model("2026-09-07T12:00:00Z")
        defer { model.cancelAll() }
        XCTAssertTrue(model.expandsClock)
        XCTAssertNil(model.clockPreview)
        let tall = model.paletteSize.height
        XCTAssertEqual(tall, 64 + 48 + 26 + (5 * 42 + 12 + LauncherModel.clockPreviewHeight) + 42, "The clock height is known synchronously")
        try await waitUntil { model.clockPreview != nil }
        XCTAssertEqual(model.paletteSize.height, tall, "The planner arriving never resizes")
        var resizes: [CGFloat] = []
        var presentations = 0
        model.onPreviewResize = { resizes.append(model.paletteSize.height) }
        model.onPresentationChange = { presentations += 1 }
        model.move(1)
        XCTAssertEqual(model.expandedCommand, .time)
        XCTAssertFalse(model.featuresClock, "Arrow keys edit nothing while the planner is collapsed")
        XCTAssertEqual(resizes, [tall - LauncherModel.clockPreviewHeight + LauncherModel.previewHeight(for: .time)])
        XCTAssertEqual(presentations, 0, "Focus changes use the no-refocus resize path")
        XCTAssertNotNil(model.clockPreview, "The planner survives while another row is focused")
        try await waitUntil { model.expandedPreview != nil }
        XCTAssertEqual(model.expandedPreview?.title, "Timestamp recognized")
        model.move(-1)
        XCTAssertTrue(model.featuresClock)
        XCTAssertEqual(resizes.last, tall)
        XCTAssertEqual(resizes.count, 2)
        model.move(1); model.move(-1)
        XCTAssertEqual(resizes.count, 4, "Each real height change reports exactly once")
        XCTAssertEqual(presentations, 0)
    }

    func testPreviewForARowThatLostFocusIsDiscardedAndRebuiltOnReturn() async throws {
        let entered = expectation(description: "Compare preview started")
        entered.assertForOverFulfill = false
        let gate = DispatchSemaphore(value: 0)
        let model = model("first") { text, command, _ in
            if command == .compare {
                entered.fulfill()
                _ = gate.wait(timeout: .now() + 3) // ignores cancellation on purpose
            }
            return LauncherPreview(title: "\(command.id):\(text)", subtitle: "", content: .notice(text))
        }
        defer { gate.signal(); model.cancelAll() }
        try await waitUntil { model.expandedPreview != nil }
        model.selectedID = LauncherCommand.compare.id
        await fulfillment(of: [entered], timeout: 2)
        model.selectedID = LauncherCommand.snippets.id
        try await waitUntil { model.expandedPreview != nil }
        XCTAssertEqual(model.expandedPreview?.title, "snippets:first")
        gate.signal()
        for _ in 0..<20 { await Task.yield() }
        XCTAssertNil(model.preview(for: .compare), "A result for a row that lost focus is dropped")
        model.selectedID = LauncherCommand.compare.id
        gate.signal()
        try await waitUntil { model.preview(for: .compare) != nil }
        XCTAssertEqual(model.expandedPreview?.title, "compare:first")
    }

    func testTimestampEnterPassesSelectionAndPreviewedInstantToDedicatedWindow() async throws {
        let text = "2026-09-07T12:00:00Z"
        let model = model(text)
        var openedTools: [UtilityWorkbenchModel] = []
        model.onOpenWorkbench = { openedTools.append($0) }
        defer { openedTools.forEach { $0.cancel() } }
        defer { model.cancelAll() }
        var opened: LauncherCommand?
        var input: String?
        var instant: Date?
        model.onCommand = { opened = $0; input = $1.text; instant = $2.worldClock?.instant }
        try await waitUntil { model.clockPreview != nil }
        XCTAssertNil(opened)
        let clock = try XCTUnwrap(model.clockPreview)
        XCTAssertEqual(clock.mode, .preview)
        XCTAssertEqual(clock.selectedInstant, Date(timeIntervalSince1970: 1_788_782_400))
        clock.nudge(by: 2)
        model.openSelected()
        XCTAssertEqual(opened, .worldClock)
        XCTAssertEqual(input, text)
        XCTAssertEqual(instant, Date(timeIntervalSince1970: 1_788_782_400 + 1_800), "Enter opens at the scrubbed time")
        XCTAssertNil(openedTools.last)
    }

    func testClockPreviewIsInteractiveWithoutTouchingSavedLocationsOrSendingOnItsOwn() async throws {
        let store = WorldClockPreferencesStore(defaults: defaults)
        store.save(zoneIDs: ["Asia/Tokyo", "Europe/Berlin"], anchorZoneID: "Europe/Berlin")
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        let sent = SentInstants()
        let model = model("2026-09-07T12:00:00Z", settings: settings, clockResponder: { request, _ in
            sent.append(request.context.selectedInstant)
            return WorldClockCopilotReply(answer: "An hour later works, and Kolkata would be 5:30 PM.",
                suggestion: WorldClockCopilotSuggestion(instant: request.context.selectedInstant.addingTimeInterval(3_600),
                    zoneIDs: ["Asia/Kolkata"], replacesLocations: false, anchorZoneID: "Asia/Kolkata"),
                suggestionIssue: nil)
        })
        defer { model.cancelAll() }
        try await waitUntil { model.clockPreview != nil }
        let clock = try XCTUnwrap(model.clockPreview)
        XCTAssertTrue(clock.zoneIDs.starts(with: ["Asia/Tokyo", "Europe/Berlin"]), "Saved locations come first, in order")
        XCTAssertLessThanOrEqual(clock.zoneIDs.count, 4)
        XCTAssertEqual(clock.anchorZoneID, "Europe/Berlin")
        XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.clockPreviewHeight)
        XCTAssertTrue(model.featuresClock)
        let sizeBefore = model.paletteSize
        var presentationChanges = 0
        var resizeHeights: [CGFloat] = []
        model.onPresentationChange = { presentationChanges += 1 }
        // The height must already be settled when the resize callback fires.
        model.onPreviewResize = { resizeHeights.append(model.paletteSize.height) }

        clock.setAnchorZone("Asia/Tokyo")
        clock.nudge(by: 4)
        clock.moveDay(by: 1)
        model.nudgeClock(by: -1, step: 3_600)
        XCTAssertEqual(store.loadZoneIDs(), ["Asia/Tokyo", "Europe/Berlin"])
        XCTAssertEqual(store.loadAnchorZoneID(validZoneIDs: ["Asia/Tokyo", "Europe/Berlin"]), "Europe/Berlin", "Preview never saves")
        XCTAssertEqual(presentationChanges, 0, "Scrubbing never resizes the palette")
        XCTAssertTrue(resizeHeights.isEmpty)
        XCTAssertEqual(model.paletteSize, sizeBefore)
        XCTAssertEqual(sent.all.count, 0, "Loading and scrubbing never call the provider")
        XCTAssertEqual(model.previewedInstant, clock.selectedInstant)

        clock.copilot.draft = "Best time for everyone?"
        clock.copilot.send()
        XCTAssertEqual(resizeHeights, [sizeBefore.height + LauncherModel.copilotTranscriptHeight],
                       "The palette grows once, and the callback sees the grown height")
        XCTAssertEqual(presentationChanges, 0, "Transcript growth uses the no-refocus resize path")
        XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.clockPreviewHeight + LauncherModel.copilotTranscriptHeight)
        try await waitUntil { !clock.copilot.isBusy }
        XCTAssertEqual(sent.all, [clock.selectedInstant], "The question carries the previewed instant")
        XCTAssertEqual(resizeHeights.count, 1, "The answer arriving does not resize again")
        let reply = try XCTUnwrap(clock.copilot.messages.last)
        let plan = try XCTUnwrap(clock.copilotPlan(for: reply))
        XCTAssertNil(plan.zoneIDs, "The palette applies time only; locations stay as saved")
        XCTAssertNil(plan.anchorZoneID)
        XCTAssertTrue(plan.summary.hasPrefix("Set time to"))
        let deferred = try XCTUnwrap(clock.deferredCopilotPlan(for: reply), "Location parts are handed to the window instead")
        XCTAssertNil(deferred.instant)
        XCTAssertEqual(deferred.zoneIDs?.last, "Asia/Kolkata")
        XCTAssertEqual(deferred.anchorZoneID, "Asia/Kolkata")
        XCTAssertEqual(deferred.summary, "Add India · Reference: India")
        let before = clock.selectedInstant
        clock.applyCopilotPlan(plan, from: reply)
        XCTAssertEqual(clock.selectedInstant, before.addingTimeInterval(3_600))
        XCTAssertFalse(clock.zoneIDs.contains("Asia/Kolkata"))
        XCTAssertEqual(store.loadZoneIDs(), ["Asia/Tokyo", "Europe/Berlin"])
        XCTAssertEqual(model.previewedInstant, before.addingTimeInterval(3_600))
        XCTAssertTrue(clock.copilot.isApplied(reply))
        XCTAssertEqual(resizeHeights.count, 1)

        // Enter hands the window everything it needs to apply the rest.
        var handoff: WorldClockHandoff?
        model.onCommand = { _, _, context in handoff = context.worldClock }
        clock.copilot.draft = "and Berlin?"
        model.openSelected()
        let carried = try XCTUnwrap(handoff)
        XCTAssertEqual(carried.instant, clock.selectedInstant)
        XCTAssertEqual(carried.anchorZoneID, "Asia/Tokyo", "The previewed reference travels without being saved")
        XCTAssertEqual(carried.copilot?.messages, clock.copilot.messages)
        XCTAssertEqual(carried.copilot?.draft, "and Berlin?")
        XCTAssertEqual(carried.copilot?.appliedParts, [reply.id: .time], "Only the time part was applied in the palette")
        XCTAssertEqual(store.loadAnchorZoneID(validZoneIDs: ["Asia/Tokyo", "Europe/Berlin"]), "Europe/Berlin")

        clock.copilot.clear()
        XCTAssertEqual(resizeHeights.last, sizeBefore.height, "Clearing shrinks the palette back")
        model.query = "regex"
        XCTAssertFalse(model.featuresClock)
        XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.previewHeight(for: .regex))
        model.query = ""
        XCTAssertTrue(model.clockPreview === clock, "Searching keeps the planner and its transcript")
        XCTAssertTrue(model.featuresClock)
        model.cancelAll()
        XCTAssertNil(model.clockPreview)
    }

    func testResizeCallbackSeesSettledStateForErrorsAndConversationsNeverLeakIntoANewSelection() async throws {
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        let model = model("2026-09-07T12:00:00Z", settings: settings, clockResponder: { request, _ in
            if request.question == "fail" { throw AIError.http(status: 500, message: "down") }
            return WorldClockCopilotReply(answer: "ok", suggestion: nil, suggestionIssue: nil)
        })
        defer { model.cancelAll() }
        try await waitUntil { model.clockPreview != nil }
        let clock = try XCTUnwrap(model.clockPreview)
        let base = model.paletteSize.height
        var resizeHeights: [CGFloat] = []
        model.onPreviewResize = { resizeHeights.append(model.paletteSize.height) }

        clock.copilot.draft = "fail"
        clock.copilot.send()
        XCTAssertEqual(resizeHeights, [base + LauncherModel.copilotTranscriptHeight])
        try await waitUntil { !clock.copilot.isBusy }
        XCTAssertNotNil(clock.copilot.errorMessage)
        XCTAssertEqual(resizeHeights.count, 1, "An error keeps the transcript visible, so no further resize")
        clock.copilot.clear()
        XCTAssertEqual(resizeHeights, [base + LauncherModel.copilotTranscriptHeight, base])

        clock.copilot.draft = "works"
        clock.copilot.send()
        try await waitUntil { !clock.copilot.isBusy }
        XCTAssertEqual(clock.copilot.messages.count, 2)
        XCTAssertEqual(model.worldClockHandoff?.copilot?.messages.count, 2)

        model.useClipboard("1788782400")
        try await waitUntil { model.clockPreview != nil && model.clockPreview !== clock }
        let replacement = try XCTUnwrap(model.clockPreview)
        XCTAssertTrue(replacement.copilot.messages.isEmpty, "A new selection starts a fresh conversation")
        XCTAssertEqual(model.worldClockHandoff?.copilot?.isEmpty, true)
        XCTAssertEqual(model.paletteSize.height, base, "The replaced transcript no longer counts toward the size")
        XCTAssertTrue(clock.copilot.messages.count == 2, "The old planner is detached, not mutated")
        var handoff: WorldClockHandoff?
        model.onCommand = { _, _, context in handoff = context.worldClock }
        model.openSelected()
        XCTAssertEqual(handoff?.instant, Date(timeIntervalSince1970: 1_788_782_400))
        XCTAssertEqual(handoff?.copilot?.isEmpty, true)
    }

    func testDismissingThePaletteCancelsTheCopilotAndOtherPreviewsStayStatic() async throws {
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        let json = model("{\"id\":1}", settings: settings)
        defer { json.cancelAll() }
        try await waitUntil { json.expandedPreview != nil }
        XCTAssertNil(json.clockPreview)
        XCTAssertNil(json.previewedInstant)
        XCTAssertFalse(json.featuresClock)
        XCTAssertEqual(json.expandedPreviewHeight, LauncherModel.previewHeight(for: .json))

        let started = expectation(description: "Copilot request started")
        let cancelled = expectation(description: "Copilot request cancelled")
        let model = model("1788782400", settings: settings, clockResponder: { _, _ in
            started.fulfill()
            do {
                try await Task.sleep(nanoseconds: 10_000_000_000)
            } catch {
                cancelled.fulfill()
                throw error
            }
            return WorldClockCopilotReply(answer: "late", suggestion: nil, suggestionIssue: nil)
        })
        try await waitUntil { model.clockPreview != nil }
        let clock = try XCTUnwrap(model.clockPreview)
        XCTAssertEqual(clock.selectedInstant, Date(timeIntervalSince1970: 1_788_782_400))
        clock.copilot.draft = "still there?"
        clock.copilot.send()
        await fulfillment(of: [started], timeout: 2)
        model.useClipboard("{\"replaced\":true}")
        await fulfillment(of: [cancelled], timeout: 2)
        XCTAssertNil(model.clockPreview, "A new selection discards the planner and its request")
        XCTAssertEqual(model.suggestions.first, .json)
    }

    func testStaleWorkerCannotOverwriteReplacementOrResurrectClearedPreview() async throws {
        let entered = expectation(description: "First worker started")
        let finished = expectation(description: "First worker finished")
        let gate = DispatchSemaphore(value: 0)
        let model = model("first") { text, _, _ in
            if text == "first" {
                entered.fulfill()
                _ = gate.wait(timeout: .now() + 3)
                finished.fulfill() // Deliberately ignore cancellation to exercise the generation guard.
            }
            return LauncherPreview(title: text, subtitle: "", content: .notice(text))
        }
        defer { gate.signal(); model.cancelAll() }
        await fulfillment(of: [entered], timeout: 2)
        model.useClipboard("second")
        try await waitUntil { model.expandedPreview?.title == "second" }
        gate.signal()
        await fulfillment(of: [finished], timeout: 2)
        await Task.yield()
        XCTAssertEqual(model.expandedPreview?.title, "second")
        model.clearSelection()
        XCTAssertNil(model.expandedPreview)
        XCTAssertNil(model.bestMatch)
        XCTAssertNil(model.selection.pid)
    }

    func testInvalidJSONShowsNoticeButKeepsCompleteEditableInput() async throws {
        let model = model("{invalid}")
        var opened: [UtilityWorkbenchModel] = []
        model.onOpenWorkbench = { opened.append($0) }
        defer { opened.forEach { $0.cancel() } }
        defer { model.cancelAll() }
        try await waitUntil { model.expandedPreview != nil }
        XCTAssertTrue(model.expandedPreview?.isWarning == true)
        XCTAssertEqual(model.expandedPreview?.title, "Not recognized for JSON Tools")
        model.openSelected()
        XCTAssertEqual(opened.last?.input, "{invalid}")
    }

    func testClockPreviewLayoutFitsTheHeightThePaletteReserves() throws {
        let store = WorldClockPreferencesStore(defaults: defaults)
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        let seed = Date(timeIntervalSince1970: 1_788_782_400)
        let clock = WorldClockViewModel(settings: settings, seedDate: seed, preferences: store, mode: .preview,
            zoneIDs: ["America/Los_Angeles", "Europe/London", "Asia/Kolkata", "Australia/Sydney"], anchorZoneID: "Europe/London",
            askCopilot: { _, _ in WorldClockCopilotReply(answer: "ok", suggestion: nil, suggestionIssue: nil) })
        clock.nudge(by: 1) // shows the "Selected time" chip as well
        let preview = LauncherPreview(title: "Timestamp recognized", subtitle: "in 2 days", content: .clocks([], instant: seed))
        func measure() -> CGFloat {
            let view = LauncherClockPreviewView(clock: clock, preview: preview, height: nil, onOpenSettings: {},
                onEscapeCopilot: {}, onCopilotFieldReady: { _ in }).frame(width: 644)
            return NSHostingView(rootView: view).fittingSize.height
        }
        XCTAssertLessThanOrEqual(measure(), LauncherModel.clockPreviewHeight, "The planner must fit without clipping")
        XCTAssertGreaterThan(measure(), LauncherModel.clockPreviewHeight - 40, "The reserved height should not leave a large gap")
        clock.copilot.draft = "q"
        clock.copilot.send()
        XCTAssertLessThanOrEqual(measure(), LauncherModel.clockPreviewHeight + LauncherModel.copilotTranscriptHeight)
        clock.cancelAI()
    }

    func testLongSelectionOpensCompleteDocumentAndOversizedSelectionHasNoPreview() async throws {
        let text = "{\"body\":\"" + String(repeating: "x", count: 300_000) + "\"}"
        let model = model(text)
        var opened: [UtilityWorkbenchModel] = []
        model.onOpenWorkbench = { opened.append($0) }
        defer { opened.forEach { $0.cancel() } }
        defer { model.cancelAll() }
        try await waitUntil { model.expandedPreview != nil }
        XCTAssertEqual(model.expandedPreview?.title, "Full selection ready")
        XCTAssertNil(model.expandedSession, "A selection over 64 KB is not parsed or edited in the row")
        XCTAssertEqual(model.expandedPreviewHeight, LauncherModel.previewHeight, "The notice keeps the compact height")
        model.openSelected()
        XCTAssertEqual(opened.last?.input, text)
        model.useClipboard(String(repeating: "x", count: UtilityLimits.inputBytes + 1))
        XCTAssertNil(model.expandedPreview)
        XCTAssertNil(model.bestMatch)
        XCTAssertEqual(opened.last?.input, text, "Changing the palette selection cannot clear an open window")
        XCTAssertTrue(model.selection.text.isEmpty)
    }
}
