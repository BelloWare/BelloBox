import AppKit
import XCTest
@testable import BelloBox

final class LauncherPreviewTests: XCTestCase {
    private let utc = TimeZone(secondsFromGMT: 0)!
    private let locale = Locale(identifier: "en_US_POSIX")
    private let now = Date(timeIntervalSince1970: 1_788_782_400) // 2026-09-07 12:00 UTC

    private func preview(_ text: String, _ command: LauncherCommand) throws -> LauncherPreview {
        try LauncherPreview.make(text: text, command: command, zoneIDs: ["UTC"], now: now, localZone: utc, locale: locale)
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

    func testClockPreviewUsesSelectedMomentAndDSTInEachSavedZone() throws {
        let value = try LauncherPreview.make(text: "2026-07-07T12:00:00Z", command: .worldClock,
            zoneIDs: ["America/New_York", "Europe/London", "Asia/Singapore", "UTC"],
            now: now, localZone: utc, locale: locale)
        guard case .clocks(let clocks) = value.content else { return XCTFail("Expected clocks") }
        XCTAssertEqual(clocks.count, 3)
        XCTAssertEqual(clocks.map(\.zone), ["UTC−04:00", "UTC+01:00", "UTC+08:00"])
        XCTAssertTrue(clocks[0].time.hasPrefix("8:00:00"))
        XCTAssertTrue(clocks[0].date.contains("2026"))
        XCTAssertEqual(clocks[0].quality, .extended)
        let winter = try LauncherPreview.make(text: "2026-01-07T12:00:00Z", command: .worldClock,
            zoneIDs: ["America/New_York"], now: now, localZone: utc, locale: locale)
        guard case .clocks(let winterClocks) = winter.content else { return XCTFail("Expected clocks") }
        XCTAssertEqual(winterClocks[0].zone, "UTC−05:00")
        XCTAssertEqual(winterClocks.count, 2, "UTC fallback is deduplicated")
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
        XCTAssertThrowsError(try preview(String(repeating: "x", count: UtilityLimits.inputBytes + 1), .textTools))
    }
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
    private func model(_ text: String, builder: LauncherModel.PreviewBuilder? = nil) -> LauncherModel {
        let selection = TextSelection(text: text, anchorRect: nil, appName: "Editor", bundleID: nil, pid: 123)
        if let builder { return LauncherModel(selection: selection, snippets: SnippetStore(), defaults: defaults, previewBuilder: builder) }
        return LauncherModel(selection: selection, snippets: SnippetStore(), defaults: defaults)
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
        defer { model.cancelAll() }
        try await waitUntil { model.preview != nil }
        let preview = model.preview
        XCTAssertEqual(model.selectedCommand, .json)
        XCTAssertNil(model.workbench)
        XCTAssertNil(defaults.object(forKey: "launcherRecents"))
        let size = model.paletteSize
        model.move(1)
        XCTAssertEqual(model.paletteSize, size, "Navigation must not jump the expanded row")
        model.query = "regex"
        XCTAssertNil(model.featuredCommand)
        XCTAssertEqual(model.preview, preview)
        model.query = ""
        XCTAssertEqual(model.featuredCommand, .json)
        XCTAssertEqual(model.preview, preview)
        model.openSelected()
        XCTAssertEqual(model.workbench?.input, "{\"id\":42}")
        model.back()
        XCTAssertEqual(model.preview, preview)
        XCTAssertEqual(defaults.stringArray(forKey: "launcherRecents"), ["json"])
    }

    func testTimestampEnterPassesOriginalSelectionToDedicatedWindow() async throws {
        let text = "2026-09-07T12:00:00Z"
        let model = model(text)
        defer { model.cancelAll() }
        var opened: LauncherCommand?
        var input: String?
        model.onCommand = { opened = $0; input = $1.text }
        try await waitUntil { model.preview != nil }
        XCTAssertNil(opened)
        model.openSelected()
        XCTAssertEqual(opened, .worldClock)
        XCTAssertEqual(input, text)
        XCTAssertNil(model.workbench)
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
        try await waitUntil { model.preview?.title == "second" }
        gate.signal()
        await fulfillment(of: [finished], timeout: 2)
        await Task.yield()
        XCTAssertEqual(model.preview?.title, "second")
        model.clearSelection()
        XCTAssertNil(model.preview)
        XCTAssertNil(model.featuredCommand)
        XCTAssertNil(model.selection.pid)
    }

    func testInvalidJSONShowsNoticeButKeepsCompleteEditableInput() async throws {
        let model = model("{invalid}")
        defer { model.cancelAll() }
        try await waitUntil { model.preview != nil }
        XCTAssertTrue(model.preview?.isWarning == true)
        model.openSelected()
        XCTAssertEqual(model.workbench?.input, "{invalid}")
    }

    func testLongSelectionOpensCompleteDocumentAndOversizedSelectionHasNoPreview() async throws {
        let text = "{\"body\":\"" + String(repeating: "x", count: 300_000) + "\"}"
        let model = model(text)
        defer { model.cancelAll() }
        try await waitUntil { model.preview != nil }
        XCTAssertEqual(model.preview?.title, "Full selection ready")
        model.openSelected()
        XCTAssertEqual(model.workbench?.input, text)
        model.useClipboard(String(repeating: "x", count: UtilityLimits.inputBytes + 1))
        XCTAssertNil(model.preview)
        XCTAssertNil(model.featuredCommand)
        XCTAssertNil(model.workbench)
        XCTAssertTrue(model.selection.text.isEmpty)
    }
}
