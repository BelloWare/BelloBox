import AppKit
import XCTest
@testable import BelloBox

final class DeveloperUtilityTests: XCTestCase {
    func testJSONPreservesLargeIDsAndExponents() throws {
        let value = try DeveloperJSON.parse(#"{"id":900719925474099312345678901234567890,"tiny":1.234567890123456789e-100}"#)
        let output = value.formatted(pretty: false)
        XCTAssertTrue(output.contains("900719925474099312345678901234567890"))
        XCTAssertTrue(output.contains("1.234567890123456789e-100"))
        XCTAssertEqual(try DeveloperJSON.parse(output), value)
    }
    func testJSONRejectsMalformedAmbiguousAndDeepInput() {
        for value in ["{\"x\":1,}", "[1,]", "01", "1.", "1e", "{\"x\":1,\"x\":2}", "{\"x\":\"\\q\"}", "true false", String(repeating: "[", count: 70) + "0" + String(repeating: "]", count: 70)] {
            XCTAssertThrowsError(try DeveloperJSON.parse(value), value)
        }
        XCTAssertThrowsError(try DeveloperJSON.parse("{\n\"x\": }")) { XCTAssertTrue($0.localizedDescription.contains("line 2")) }
    }
    func testJSONUnicodeAndNullRoundTrip() throws {
        let value = try DeveloperJSON.parse(#"{"emoji":"\ud83d\ude80","empty":null,"bool":false,"s":"\n\t\""}"#)
        XCTAssertEqual(try DeveloperJSON.parse(value.formatted()), value)
    }
    func testCompareInsertRemoveAndJSONPropertyOrdering() throws {
        let result = try TextComparison.compare("alpha\nbeta\ngamma", "alpha\nnew\ngamma\nend", mode: .lines, ignoreWhitespace: false)
        XCTAssertEqual(result.added, 2); XCTAssertEqual(result.removed, 1)
        XCTAssertEqual(result.rows.filter { $0.kind != .removed }.map(\.text), ["alpha", "new", "gamma", "end"])
        let json = try TextComparison.compare(#"{"b":2,"a":1}"#, #"{"a":1,"b":2}"#, mode: .json, ignoreWhitespace: false)
        XCTAssertEqual(json.added + json.removed, 0)
    }
    func testComparisonWhitespaceWordsAndFieldPaths() throws {
        XCTAssertEqual(try TextComparison.compare("a   b", "a b", mode: .lines, ignoreWhitespace: true).added, 0)
        let words = try TextComparison.compare("one two", "one three", mode: .words, ignoreWhitespace: false)
        XCTAssertEqual(words.added, 1); XCTAssertEqual(words.removed, 1)
        let fields = try TextComparison.compare(#"{"a.b":1}"#, #"{"a":{"b":1}}"#, mode: .json, ignoreWhitespace: false)
        XCTAssertEqual(fields.added, 1); XCTAssertEqual(fields.removed, 1)
    }
    func testJWTExpiryIsReadableAndNeverClaimsVerification() throws {
        func base64(_ text: String) -> String { Data(text.utf8).base64EncodedString().replacingOccurrences(of: "=", with: "").replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_") }
        let token = base64(#"{"alg":"HS256"}"#) + "." + base64(#"{"exp":1000,"sub":"test"}"#) + ".signature"
        let result = try JWTInspector.inspect("Bearer " + token, now: Date(timeIntervalSince1970: 2_000))
        XCTAssertTrue(result.contains("SIGNATURE NOT VERIFIED")); XCTAssertTrue(result.contains("expired")); XCTAssertTrue(result.contains("1970-01-01"))
        XCTAssertThrowsError(try JWTInspector.inspect("a.b.c.d.e"))
        XCTAssertThrowsError(try JWTInspector.inspect("a!.b.c"))
    }
    func testRegexGroupsExtractionAndReplacement() throws {
        let result = try RegexTester.inspect(text: "BOX-12 API-34", pattern: "([A-Z]+)-(\\d+)", replacement: "$2:$1", caseInsensitive: false, multiline: false)
        XCTAssertEqual(result.ranges.count, 2)
        XCTAssertEqual(result.extracted, "BOX-12\nAPI-34")
        XCTAssertEqual(result.replaced, "12:BOX 34:API")
        XCTAssertTrue(result.details.contains("$2: 34"))
    }
    func testRegexStopsCatastrophicBacktracking() {
        let start = Date()
        XCTAssertThrowsError(try RegexTester.inspect(text: String(repeating: "a", count: 20_000) + "!", pattern: "(a+)+$", replacement: "", caseInsensitive: false, multiline: false, timeout: 0.05))
        XCTAssertLessThan(Date().timeIntervalSince(start), 2)
    }
    func testRegexHandlesZeroWidthAndUnicodeRanges() throws {
        let result = try RegexTester.inspect(text: "🚀a", pattern: "(?=a)", replacement: "X", caseInsensitive: false, multiline: false)
        XCTAssertEqual(result.ranges, [NSRange(location: 2, length: 0)])
        XCTAssertEqual(result.replaced, "🚀Xa")
    }
    func testURLPreservesRepeatedParametersFlagsAndLiteralPlus() throws {
        var url = try URLInspection("https://example.com:8443/a?q=a+b&tag=one&tag=two&flag#hello")
        XCTAssertEqual(url.parameters[0].value, "a+b")
        XCTAssertEqual(url.parameters.filter { $0.name == "tag" }.count, 2)
        XCTAssertFalse(url.parameters.last!.hasValue)
        url.parameters[0].value = "a & b"
        let output = try url.rebuilt()
        let parsed = URLComponents(string: output)!
        XCTAssertEqual(parsed.port, 8443)
        XCTAssertEqual(parsed.queryItems?.first?.value, "a & b")
        XCTAssertTrue(output.contains("&flag#"))
    }
    func testCSVQuotedCommasNewlinesAndEscapedQuotesRoundTrip() throws {
        let text = "name,note\r\n\"Ada, A\",\"line1\nline2\"\r\nBob,\"said \"\"hi\"\"\"\r\n"
        let value = try CSVCodec.decode(text, delimiter: ",", inferTypes: false)
        let encoded = try CSVCodec.encode(value, delimiter: ",")
        XCTAssertEqual(try CSVCodec.decode(encoded, delimiter: ",", inferTypes: false), value)
    }
    func testCSVRejectsBadRowsAndPreservesNumericStringsByDefault() throws {
        for text in ["a,a\n1,2", "a,b\n1", "a\n\"bad", "a\n\"x\"q"] { XCTAssertThrowsError(try CSVCodec.decode(text, delimiter: ",", inferTypes: false)) }
        let text = "id,flag\n00123,true"
        XCTAssertTrue(try CSVCodec.decode(text, delimiter: ",", inferTypes: false).formatted().contains("\"true\""))
        let inferred = try CSVCodec.decode(text, delimiter: ",", inferTypes: true).formatted()
        XCTAssertTrue(inferred.contains("\"00123\"")); XCTAssertTrue(inferred.contains(": true"))
    }
    func testYAMLJSONRoundTripPreservesNumbersAndStringTypes() throws {
        let source = #"{"id":90071992547409931234567890,"yes":"yes","on":"on","active":true,"nothing":null,"items":["001",2.5]}"#
        let yaml = try DataConversion.convert(source, from: .json, to: .yaml).0
        let json = try DataConversion.convert(yaml, from: .yaml, to: .json).0
        XCTAssertEqual(try DeveloperJSON.parse(json), try DeveloperJSON.parse(source))
        let plain = try DataConversion.convert("yes: on\nflag: true\ndate: 2026-09-06", from: .yaml, to: .json).0
        XCTAssertTrue(plain.contains("\"yes\": \"on\""))
        XCTAssertTrue(plain.contains("\"2026-09-06\""))
    }
    func testYAMLAliasesAndDuplicateKeys() throws {
        let json = try DataConversion.convert("first: &x [1, 2]\nsecond: *x", from: .yaml, to: .json).0
        XCTAssertEqual(try DeveloperJSON.parse(json), .object(["first": .array([.number("1"), .number("2")]), "second": .array([.number("1"), .number("2")])]))
        XCTAssertThrowsError(try DataConversion.convert("a: 1\na: 2", from: .yaml, to: .json))
        XCTAssertThrowsError(try DataConversion.convert("a: &a [*a]", from: .yaml, to: .json))
    }
    func testTimestampUnitsZeroNegativeAndDifference() throws {
        XCTAssertEqual(try DeveloperTime.date("0", unit: .seconds), Date(timeIntervalSince1970: 0))
        XCTAssertEqual(try DeveloperTime.date("-1000", unit: .milliseconds), Date(timeIntervalSince1970: -1))
        XCTAssertEqual(try DeveloperTime.date("1700000000000", unit: .automatic), Date(timeIntervalSince1970: 1700000000))
        let summary = try DeveloperTime.summary("0", unit: .seconds, zone: TimeZone(secondsFromGMT: 0)!, comparedWith: "3600")
        XCTAssertTrue(summary.contains("1.000 hours"))
        XCTAssertThrowsError(try DeveloperTime.date("1e100"))
    }
    func testCronNamesRangesStepsAndSundayAlias() throws {
        let cron = try CronSchedule("*/15 9-17 * * MON-FRI")
        XCTAssertEqual(cron.fields[0].values, [0, 15, 30, 45])
        XCTAssertEqual(cron.fields[4].values, [1, 2, 3, 4, 5])
        XCTAssertEqual(try CronSchedule("0 0 * * 7").fields[4].values, [0])
        for input in ["* * * * * *", "*/0 * * * *", "60 * * * *", "0 0 * BAD *", "0 0 * * FRI-MON"] { XCTAssertThrowsError(try CronSchedule(input)) }
    }
    func testCronNextRunsAndDayOfMonthOrWeekdayRule() throws {
        let now = ISO8601DateFormatter().date(from: "2026-09-06T08:00:00Z")! // Sunday
        let cron = try CronSchedule("0 9 7 * SUN")
        let dates = try cron.next(after: now, zone: TimeZone(secondsFromGMT: 0)!, count: 2)
        XCTAssertEqual(dates.map { ISO8601DateFormatter().string(from: $0) }, ["2026-09-06T09:00:00Z", "2026-09-07T09:00:00Z"])
    }
    func testCronSkipsNonexistentDSTWallClockTime() throws {
        let now = ISO8601DateFormatter().date(from: "2026-03-08T00:00:00Z")!
        let dates = try CronSchedule("30 2 * * *").next(after: now, zone: TimeZone(identifier: "America/New_York")!, count: 1)
        XCTAssertEqual(ISO8601DateFormatter().string(from: dates[0]), "2026-03-09T06:30:00Z")
    }
    func testSnippetsSubstituteOnceAndUseStableBuiltins() {
        let text = "Hello {{name}}: {{selection}} {{uuid}} {{date}}"
        let result = SnippetTemplate.render(text, selection: "{{name}}", values: ["name": "Ada"], now: Date(timeIntervalSince1970: 0), uuid: "test-uuid")
        XCTAssertEqual(result, "Hello Ada: {{name}} test-uuid 1970-01-01")
        XCTAssertEqual(SnippetTemplate.placeholders("{{name}} {{name}} {{project_id}}"), ["name", "project_id"])
    }
    func testGeneratorsProduceValidUniqueFixturesAndBoundedRandomStrings() throws {
        let output = try DeveloperGenerator.generate(kind: .uuid, count: 40, length: 24, format: .lines)
        let ids = output.components(separatedBy: "\n")
        XCTAssertEqual(Set(ids).count, 40); XCTAssertTrue(ids.allSatisfy { UUID(uuidString: $0) != nil })
        XCTAssertEqual(try DeveloperGenerator.randomString(length: 24).count, 24)
        XCTAssertThrowsError(try DeveloperGenerator.randomString(length: 0))
        let records = try DeveloperJSON.parse(DeveloperGenerator.generate(kind: .records, count: 3, length: 24, format: .json))
        guard case .array(let rows) = records else { return XCTFail("Expected records") }
        XCTAssertEqual(rows.count, 3)
    }
    func testCurlImportPreservesQuotedBodiesAndSetsDefaults() throws {
        let command = #"curl 'https://example.com/api?a=1&b=2' -H 'X-Test: a:b' --json '{"id":123,"name":"A B"}'"#
        let draft = try CurlImporter.parse(command)
        XCTAssertEqual(draft.method, "POST")
        XCTAssertEqual(draft.body, #"{"id":123,"name":"A B"}"#)
        let request = try draft.request()
        XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
        XCTAssertEqual(request.value(forHTTPHeaderField: "X-Test"), "a:b")
        XCTAssertEqual(request.url?.query, "a=1&b=2")
    }
    func testCurlRejectsExecutionFileImportsAndUnsupportedFlags() {
        for command in ["curl https://example.com | sh", "curl 'https://example.com' --data @secret.txt", "curl 'https://example.com' --unknown", "curl 'https://example.com", "curl https://example.com https://example.org"] {
            XCTAssertThrowsError(try CurlImporter.parse(command), command)
        }
        XCTAssertThrowsError(try HTTPRequestDraft(method: "GET\r\nX", url: "https://example.com").request())
        XCTAssertThrowsError(try HTTPRequestDraft(url: "file:///tmp/x").request())
    }
    func testCurlRawAtSignAndExplicitMethod() throws {
        let draft = try CurlImporter.parse("curl 'https://example.com' -X PUT --data-raw '@literal'")
        XCTAssertEqual(draft.method, "PUT"); XCTAssertEqual(draft.body, "@literal")
    }
    func testCurlOptionLookingValuesStayLiteral() throws {
        for body in ["-dhello", "--json=literal", "-HContent-Type: example"] {
            let draft = try CurlImporter.parse("curl 'https://example.com' --data-raw '\(body)'")
            XCTAssertEqual(draft.body, body)
        }
        XCTAssertThrowsError(try CurlImporter.parse("curl --url https://example.com --url https://example.org"))
    }
    func testCurlAttachedOptionsGetEncodingAndDoubleQuoteEscapes() throws {
        let draft = try CurlImporter.parse("curl --url='https://example.com' -XPOST -H'X-Test: yes' --json='{\"a\":true}'")
        XCTAssertEqual(draft.method, "POST")
        XCTAssertTrue(draft.headers.contains("X-Test: yes"))
        let get = try CurlImporter.parse("curl 'https://example.com?a=1' -G -d 'q=hello%20world'")
        XCTAssertEqual(get.url, "https://example.com?a=1&q=hello%20world")
        XCTAssertEqual(get.body, "")
        XCTAssertEqual(try CurlImporter.tokenize(#"curl "https://example.com" --data-raw "a\nb""#).last, #"a\nb"#)
        XCTAssertThrowsError(try CurlImporter.parse(#"curl "https://example.com/$TOKEN""#))
    }
    func testEmptyComparisonHasNoPhantomAddedLine() throws {
        let result = try TextComparison.compare("text", "", mode: .lines, ignoreWhitespace: false)
        XCTAssertEqual(result.added, 0)
        XCTAssertEqual(result.removed, 1)
    }
    func testEditableURLPortIsValidated() throws {
        var url = try URLInspection("http://localhost:8080/test")
        XCTAssertEqual(url.port, "8080")
        url.port = "9090"
        XCTAssertTrue(try url.rebuilt().contains(":9090/"))
        url.port = "70000"
        XCTAssertThrowsError(try url.rebuilt())
    }
    func testHTTPResponseIsBoundedAndPrettyPrinted() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [UtilityHTTPProtocol.self]
        let response = try await HTTPRequestTool.send(HTTPRequestDraft(url: "https://example.test/fixture"), configuration: configuration)
        XCTAssertTrue(response.status.hasPrefix("HTTP 201"))
        XCTAssertTrue(response.body.contains("\"id\": 9007199254740993"))
        XCTAssertTrue(response.headers.lowercased().contains("x-fixture: yes"))
    }

    func testLauncherSuggestionsSearchAndFavorites() {
        XCTAssertEqual(LauncherCommand.suggestions(for: "{bad json").first, .json)
        XCTAssertEqual(LauncherCommand.suggestions(for: "https://example.com").first, .url)
        XCTAssertEqual(LauncherCommand.search("pretty json", input: "", favorites: [], recents: []).first, .json)
        XCTAssertEqual(LauncherCommand.search("generate", input: "", favorites: [], recents: []).first, .generate)
        XCTAssertEqual(LauncherCommand.search("", input: "", favorites: ["regex"], recents: []).first, .regex)
        XCTAssertTrue(LauncherCommand.search("doesnotexist", input: "", favorites: [], recents: []).isEmpty)
        XCTAssertEqual(Set(LauncherCommand.allCases.map(\.id)).count, LauncherCommand.allCases.count)
    }
}

@MainActor
final class DeveloperWorkflowTests: XCTestCase {
    func testSnippetsPersistUpdateDeleteAndProtectFilePermissions() throws {
        let base = ProcessInfo.processInfo.environment["BELLOBOX_TEST_TMP_DIR"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        let url = base.appendingPathComponent("snippets-\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: url) }
        let store = SnippetStore(url: url)
        var snippet = DeveloperSnippet(name: "Example", template: "Hello {{name}}")
        try store.save(snippet)
        XCTAssertEqual(SnippetStore(url: url).snippets, [snippet])
        snippet.template = "Updated"; try store.save(snippet)
        XCTAssertEqual(store.snippets.count, 1)
        let permissions = try FileManager.default.attributesOfItem(atPath: url.path)[.posixPermissions] as? NSNumber
        XCTAssertEqual(permissions?.intValue, 0o600)
        try store.remove(snippet.id)
        XCTAssertTrue(SnippetStore(url: url).snippets.isEmpty)
    }
    func testUnreadableSnippetLibraryIsNotOverwritten() throws {
        let base = ProcessInfo.processInfo.environment["BELLOBOX_TEST_TMP_DIR"].map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        let url = base.appendingPathComponent("bad-snippets-\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: url) }
        try Data("invalid original".utf8).write(to: url)
        let store = SnippetStore(url: url)
        XCTAssertThrowsError(try store.save(DeveloperSnippet(name: "New", template: "text")))
        XCTAssertEqual(try String(contentsOf: url), "invalid original")
    }
    func testConverterChainingUpdatesTheInputFormat() async throws {
        let selection = TextSelection(text: "[{\"name\":\"Ada\"}]", anchorRect: nil, appName: nil, bundleID: nil, pid: nil)
        let model = UtilityWorkbenchModel(command: .convert, selection: selection, snippets: SnippetStore())
        model.toFormat = .csv
        try await Task.sleep(nanoseconds: 450_000_000)
        model.useOutputAsInput()
        XCTAssertEqual(model.fromFormat, .csv)
        XCTAssertEqual(model.toFormat, .json)
        try await Task.sleep(nanoseconds: 450_000_000)
        XCTAssertNil(model.error)
        XCTAssertEqual(try DeveloperJSON.parse(model.output), try DeveloperJSON.parse(selection.text))
    }

    func testLauncherKeyboardNavigationAndContextArePreserved() {
        let suite = "LauncherTest.\(UUID().uuidString)", defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let selection = TextSelection(text: "{\"id\":1}", anchorRect: nil, appName: "Test", bundleID: nil, pid: nil)
        let model = LauncherModel(selection: selection, snippets: SnippetStore(), defaults: defaults)
        XCTAssertEqual(model.selectedID, "json")
        model.query = "regex"
        XCTAssertEqual(model.selectedID, "regex")
        model.openSelected()
        XCTAssertEqual(model.workbench?.input, selection.text)
        XCTAssertFalse(model.workbench!.canReplace)
        model.back()
        XCTAssertNil(model.workbench)
        model.query = ""
        model.move(-1)
        XCTAssertNotNil(model.selectedID)
    }
    func testClipboardContextCannotReplaceAnUnrelatedAppSelection() {
        let suite = "ClipboardLauncher.\(UUID().uuidString)", defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let selection = TextSelection(text: "original", anchorRect: nil, appName: "Editor", bundleID: nil, pid: 123)
        let model = LauncherModel(selection: selection, snippets: SnippetStore(), defaults: defaults)
        model.open(.json)
        model.back()
        model.useClipboard("{\"copied\":true}")
        model.open(.json)
        XCTAssertEqual(model.workbench?.input, "{\"copied\":true}")
        XCTAssertNil(model.workbench?.selection.pid)
        model.back()
    }

    func testStaleCalculationsCannotReplaceNewInputOrRestoreCancelledOutput() async throws {
        let model = UtilityWorkbenchModel(command: .json, selection: TextSelection(text: "{}", anchorRect: nil, appName: nil, bundleID: nil, pid: nil), snippets: SnippetStore())
        model.input = "{bad"
        model.input = "{\"new\":true}"
        try await Task.sleep(nanoseconds: 500_000_000)
        XCTAssertNil(model.error)
        XCTAssertTrue(model.output.contains("\"new\": true"))
        model.input = "{\"later\":false}"
        model.cancel()
        try await Task.sleep(nanoseconds: 300_000_000)
        XCTAssertTrue(model.output.isEmpty)
        XCTAssertFalse(model.busy)
    }
}

private final class UtilityHTTPProtocol: URLProtocol {
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let response = HTTPURLResponse(url: request.url!, statusCode: 201, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json", "X-Fixture": "yes"])!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(#"{"id":9007199254740993}"#.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}
