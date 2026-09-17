import AppKit
import SwiftUI
import XCTest
@testable import BelloBox

final class AdditionalUtilityTests: XCTestCase {
    private func run(_ kind: AdditionalUtilityKind, _ input: String, _ options: [String: String] = [:], second: String = "") throws -> WorkbenchResult {
        try AdditionalUtilityEngine.run(kind, input: input, second: second, options: options)
    }
    func testTwentyDistinctDiscoverableLocalToolsHaveWorkingExamples() throws {
        XCTAssertEqual(AdditionalUtilityKind.allCases.count, 40)
        XCTAssertEqual(LauncherCommand.allCases.count, 61)
        for kind in AdditionalUtilityKind.allCases {
            XCTAssertTrue(kind.command.isDeveloperTool)
            XCTAssertNotNil(NSImage(systemSymbolName: kind.symbol, accessibilityDescription: nil), kind.title)
            XCTAssertEqual(LauncherCommand.search(kind.title, input: "", favorites: [], recents: []).first, kind.command)
            let result = try run(kind, kind.example, second: kind.secondExample)
            XCTAssertFalse(result.text.isEmpty, kind.title)
        }
    }
    func testExplicitWordSearchBeatsIncidentalNewToolSubstrings() {
        XCTAssertEqual(LauncherCommand.search("ai", input: "email text", favorites: ["extract"], recents: ["extract"]).first, .ai)
        XCTAssertEqual(LauncherCommand.search("gif", input: "", favorites: [], recents: []).first, .videoToGIF)
    }
    func testArithmeticPrecedenceFunctionsDomainsAndLimits() throws {
        for (expression, expected) in [("2+3*4", "14"), ("(2+3)*4", "20"), ("2^3^2", "512"), ("-2^2", "-4"), ("2^-2", "0.25"), ("sqrt(81)+max(2,3)", "12"), ("log10(100)+1e2", "102"), ("sin(pi/2)", "1")] {
            XCTAssertEqual(try run(.calculator, expression).text, expected, expression)
        }
        for expression in ["1/0", "min(1/0, 2)", "min(sqrt(-1), 2)", "sqrt(-1)", "1..2", "2+", "random()", "2(3)", String(repeating: "(", count: 65) + "1" + String(repeating: ")", count: 65)] { XCTAssertThrowsError(try run(.calculator, expression), expression) }
    }
    func testUnitDimensionsAffineTemperatureAndBinaryData() throws {
        XCTAssertEqual(try run(.units, "32", ["from": "°F", "to": "°C"]).text, "0 °C")
        XCTAssertEqual(try run(.units, "1", ["from": "mi", "to": "m"]).text, "1609.344 m")
        XCTAssertEqual(try run(.units, "1024", ["from": "KiB", "to": "MiB"]).text, "1 MiB")
        XCTAssertEqual(try run(.units, "1", ["from": "MB", "to": "B"]).text, "1000000 B")
        XCTAssertThrowsError(try run(.units, "1", ["from": "kg", "to": "m"]))
        XCTAssertThrowsError(try run(.units, "-274", ["from": "°C", "to": "K"]))
    }
    func testRadixConversionBeyondDoubleAndUInt64() throws {
        let big = try run(.numberBase, "18446744073709551616").text
        XCTAssertTrue(big.contains("Hex      10000000000000000"))
        XCTAssertTrue(big.contains("Binary   1" + String(repeating: "0", count: 64)))
        XCTAssertTrue(try run(.numberBase, "-0xFF", ["base": "16"]).text.contains("Decimal  -255"))
        XCTAssertTrue(try run(.numberBase, "-000").text.contains("Decimal  0"))
        XCTAssertEqual(try run(.numberBase, "255", ["outputBase": "16"]).text, "ff")
        XCTAssertThrowsError(try run(.numberBase, "0b12", ["base": "2"]))
        XCTAssertThrowsError(try run(.numberBase, String(repeating: "1", count: 257)))
    }
    func testColorFormatsAndUnroundedContrast() throws {
        XCTAssertEqual(try UtilityColor.parse("#f80").hex, "#FF8800")
        XCTAssertEqual(try run(.color, "#f80", ["format": "HEX"]).text, "#FF8800")
        XCTAssertEqual(try UtilityColor.parse("hsl(120 100% 50%)").hex, "#00FF00")
        XCTAssertEqual(try UtilityColor.parse("rgb(100% 0% 0% / 50%)").hex, "#FF000080")
        XCTAssertEqual(try UtilityColor.parse("#1234").hex, "#11223344")
        for invalid in ["rgb(300 0 0)", "rgb(1 / 2 3)", "rgb()", "rgb(1,,2,3)", "hsl(deg120deg 100% 50%)"] { XCTAssertThrowsError(try UtilityColor.parse(invalid)) }
        XCTAssertThrowsError(try run(.contrast, "#0008"))
        let contrast = try run(.contrast, "black", ["background": "white"])
        guard case .contrast(_, _, let ratio) = contrast.visual else { return XCTFail("Missing visual") }
        XCTAssertEqual(ratio, 21, accuracy: 1e-10)
        XCTAssertTrue(contrast.text.contains("AAA normal text (7:1): Pass"))
        XCTAssertTrue(try run(.contrast, "#777777").text.contains("AA normal text (4.5:1): Fail"))
        XCTAssertTrue(try run(.gradient, "#000", ["end": "#fff", "angle": "-90"]).text.contains("270deg"))
    }
    func testMarkdownProducesOfflinePreviewAndEscapesExecutableMarkup() throws {
        let result = try run(.markdown, "# Title\n\n**Bold** and *italic*\n\n<script>alert(1)</script>\n\n[x](javascript:evil)\n\n```html\n<img src=x>\n```")
        XCTAssertTrue(result.text.contains("<h1>Title</h1>"))
        XCTAssertTrue(result.text.contains("<strong>Bold</strong>"))
        XCTAssertTrue(result.text.contains("&lt;script&gt;"))
        XCTAssertFalse(result.text.contains("<script>"))
        XCTAssertFalse(result.text.contains("href=\"javascript:"))
        XCTAssertTrue(result.text.contains("&lt;img src=x&gt;"))
        XCTAssertNotNil(result.visual)
    }
    func testMarkdownTreatsCRLFAsOneBreakIncludingFencedCode() throws {
        let lf = "# Heading\n\n```swift\nlet a = 1\nlet b = 2\n```"
        XCTAssertEqual(try run(.markdown, lf).text, try run(.markdown, lf.replacingOccurrences(of: "\n", with: "\r\n")).text)
        XCTAssertEqual(try run(.markdown, lf).text, try run(.markdown, lf.replacingOccurrences(of: "\n", with: "\r")).text)
    }
    func testRFC6901PointersEscapesFragmentsAndMissingValues() throws {
        let json = try DeveloperJSON.parse(#"{"foo":["bar","baz"],"":0,"a/b":1,"m~n":8,"~1":9,"large":9007199254740993}"#)
        let expected = ["/foo/0": "\"bar\"", "/": "0", "/a~1b": "1", "/m~0n": "8", "/~01": "9", "#/a%7E1b": "1", "/large": "9007199254740993"]
        for (pointer, value) in expected { XCTAssertEqual(try JSONPointerTool.resolve(json, pointer: pointer).formatted(pretty: false), value) }
        XCTAssertEqual(try JSONPointerTool.resolve(json, pointer: "").formatted(), json.formatted())
        for pointer in ["foo", "/foo/01", "/foo/-", "/foo/2", "/missing", "/m~2n", "#/%ZZ"] { XCTAssertThrowsError(try JSONPointerTool.resolve(json, pointer: pointer), pointer) }
    }
    func testTypedFlatteningRoundTripsContainersEscapedKeysAndExactNumbers() throws {
        for input in [#"{"0":[],"/":{"~":9007199254740993},"empty":{},"a":[null,true,"x"]}"#, "[]", "{}", "null", "42", #"{"a/b":{"":1}}"#] {
            let json = try DeveloperJSON.parse(input)
            let flat = try JSONPathEntries.flatten(json)
            XCTAssertEqual(try JSONPathEntries.unflatten(flat).formatted(), json.formatted())
        }
        for bad in [#"[{"path":"/a","type":"value","value":1}]"#, #"[{"path":"","type":"array"},{"path":"/1","type":"value","value":1}]"#, #"[{"path":"","type":"value","value":{}}]"#, #"[{"path":"","type":"object"},{"path":"","type":"array"}]"#] {
            XCTAssertThrowsError(try JSONPathEntries.unflatten(DeveloperJSON.parse(bad)))
        }
    }
    func testInferredModelsCoverEverySampleAndGenerateValidSwift() throws {
        let input = #"[{"id":1,"first-name":"Ada","class":true,"empty":{},"mixed":[1,"x"],"self":1},{"id":2,"newField":null}]"#
        let ts = try run(.jsonCode, input).text
        XCTAssertTrue(ts.contains("\"first-name\"?: string | null"))
        XCTAssertTrue(ts.contains("\"newField\"?: unknown | null"))
        XCTAssertTrue(ts.contains("\"mixed\"?: Array<unknown> | null"))
        let swift = try run(.jsonCode, input, ["language": "Swift"]).text
        XCTAssertTrue(swift.contains("Decimal"))
        XCTAssertTrue(swift.contains("JSONValue"))
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("BelloBox-Typecheck-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("Models.swift")
        try swift.write(to: file, atomically: true, encoding: .utf8)
        let process = Process(), pipe = Pipe()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/xcrun")
        process.arguments = ["swiftc", "-typecheck", file.path]
        process.standardError = pipe
        try process.run()
        let diagnostics = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        process.waitUntilExit()
        XCTAssertEqual(process.terminationStatus, 0, diagnostics)
        XCTAssertThrowsError(try run(.jsonCode, input, ["name": "Root; bad"]))
        XCTAssertThrowsError(try run(.jsonCode, input, ["name": "Root\n"]))
    }
    func testSQLQuotesIdentifiersValuesAndMissingFieldsWithoutExecution() throws {
        let input = #"[{"id":9007199254740993,"name":"O'Reilly","path":"C:\\test"},{"id":2,"name":null}]"#
        let sql = try run(.sqlInsert, input, ["table": "a\"b"]).text
        XCTAssertTrue(sql.hasPrefix("INSERT INTO \"a\"\"b\""))
        XCTAssertTrue(sql.contains("9007199254740993"))
        XCTAssertTrue(sql.contains("E'O''Reilly'"))
        XCTAssertTrue(sql.contains("NULL"))
        XCTAssertTrue(try run(.sqlInsert, #"{"active":true}"#, ["dialect": "SQLite"]).text.contains("(1)"))
        XCTAssertThrowsError(try run(.sqlInsert, "[1,2]"))
        XCTAssertThrowsError(try run(.sqlInsert, #"{"text":"\u0000"}"#))
    }
    func testXMLPreservesMixedContentRepeatedChildrenAndRejectsEntities() throws {
        let result = try run(.xmlJSON, #"<p id="1">Hello <b>Ada</b>!<b>Lin</b>&amp;</p>"#)
        let json = try DeveloperJSON.parse(result.text)
        XCTAssertEqual(try JSONPointerTool.resolve(json, pointer: "/children/0").scalarText, "Hello ")
        XCTAssertEqual(try JSONPointerTool.resolve(json, pointer: "/children/2").scalarText, "!")
        XCTAssertEqual(try JSONPointerTool.resolve(json, pointer: "/children/3/children/0").scalarText, "Lin")
        XCTAssertThrowsError(try run(.xmlJSON, #"<!DOCTYPE a [<!ENTITY x SYSTEM "file:///etc/passwd">]><a>&x;</a>"#))
        XCTAssertThrowsError(try run(.xmlJSON, "<a><b></a>"))
    }
    func testUnicodeNormalizationAndLiteralRoundTrip() throws {
        let decomposed = "e\u{301}"
        XCTAssertEqual(try run(.unicode, decomposed, ["mode": "NFC"]).text.unicodeScalars.map(\.value), [0xE9])
        XCTAssertEqual(try run(.unicode, "é", ["mode": "NFD"]).text.unicodeScalars.map(\.value), [0x65, 0x301])
        XCTAssertTrue(try run(.unicode, "👩🏽‍💻").text.contains("U+200D"))
        let input = "Hello \"a\"\n\\(danger)\0"
        let quoted = try run(.stringEscape, input).text
        XCTAssertEqual(try run(.stringEscape, quoted, ["mode": "JSON unquote"]).text, input)
        XCTAssertEqual(try run(.stringEscape, "O'Reilly", ["mode": "Shell quote"]).text, "'O'\"'\"'Reilly'")
        XCTAssertTrue(TextUtility.swiftLiteral(input).contains("\\\\(danger)"))
        XCTAssertThrowsError(try run(.stringEscape, "\0", ["mode": "Shell quote"]))
    }
    func testLinkEmailExtractionAndStableSetOperations() throws {
        let input = "https://example.com/a https://example.com/a hello@example.com"
        XCTAssertEqual(try run(.extract, input).text, "https://example.com/a")
        XCTAssertEqual(try run(.extract, input, ["mode": "Emails"]).text, "hello@example.com")
        let a = " Swift \nRust\nSwift\n", b = "swift\nGo\nRUST"
        XCTAssertEqual(try run(.listSet, a, ["mode": "Intersection", "matching": "Trim & ignore case"], second: b).text, "Swift\nRust")
        XCTAssertEqual(try run(.listSet, a, ["mode": "Symmetric difference", "matching": "Trim & ignore case"], second: b).text, "Go")
        XCTAssertEqual(try run(.listSet, "", second: "Go\nGo").text, "Go")
    }
    func testSemVerSpecificationSequenceAndUnboundedComponents() throws {
        let sequence = ["1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta", "1.0.0-beta", "1.0.0-beta.2", "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0"]
        XCTAssertEqual(try run(.semver, sequence.reversed().joined(separator: "\n")).text, sequence.joined(separator: "\n"))
        XCTAssertEqual(try SemanticVersion("1.0.0+a"), try SemanticVersion("1.0.0+b"))
        XCTAssertLessThan(try SemanticVersion("99999999999999999999.0.0"), try SemanticVersion("100000000000000000000.0.0"))
        for invalid in ["v1.2.3", "1.2", "01.0.0", "1.0.0-01", "1.0.0+", "1.2.3\n"] { XCTAssertThrowsError(try SemanticVersion(invalid)) }
    }
    func testIPv4BoundariesAndPermissionBits() throws {
        let subnet = try run(.subnet, "192.168.1.42/24").text
        XCTAssertTrue(subnet.contains("192.168.1.0/24")); XCTAssertTrue(subnet.contains("192.168.1.254")); XCTAssertTrue(subnet.contains("Usable hosts 254"))
        XCTAssertTrue(try run(.subnet, "10.0.0.0/31").text.contains("Usable hosts 2"))
        XCTAssertTrue(try run(.subnet, "10.0.0.1/32").text.contains("Usable hosts 1"))
        XCTAssertTrue(try run(.subnet, "0.0.0.0/0").text.contains("Addresses    4294967296"))
        for invalid in ["10.0.0.256/24", "10.00.0.1/24", "::1/128", "10.0.0.1/33"] { XCTAssertThrowsError(try run(.subnet, invalid)) }
        XCTAssertEqual(SecurityUtility.symbolic(0o4755), "rwsr-xr-x")
        XCTAssertEqual(try SecurityUtility.permissionBits("rwSr-Sr-T"), 0o7644)
        for bits in 0...0o7777 { XCTAssertEqual(try SecurityUtility.permissionBits(SecurityUtility.symbolic(bits)), bits) }
        XCTAssertThrowsError(try SecurityUtility.permissionBits("888"))
    }
    func testRFC4231HMACVectorsAndKeyValidation() throws {
        let message = "what do ya want for nothing?"
        XCTAssertEqual(try run(.hmac, message, second: "Jefe").text, "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843")
        XCTAssertEqual(try run(.hmac, message, ["algorithm": "SHA-512"], second: "Jefe").text, "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737")
        XCTAssertEqual(try run(.hmac, "Hi There", ["keyFormat": "Hex"], second: String(repeating: "0b", count: 20)).text, "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7")
        XCTAssertThrowsError(try run(.hmac, message, ["keyFormat": "Hex"], second: "abc"))
        XCTAssertThrowsError(try run(.hmac, message))
    }
    func testFlatteningBoundsRepeatedLongPathsBeforeExpansion() throws {
        let longKey = String(repeating: "a", count: 8_200)
        XCTAssertThrowsError(try JSONPathEntries.flatten(.object([longKey: .array([.number("1")])])))
    }
    func testAllEnginesRejectOversizedInputsAndOptions() {
        let big = String(repeating: "x", count: UtilityLimits.inputBytes + 1)
        for kind in AdditionalUtilityKind.allCases {
            XCTAssertThrowsError(try run(kind, big), kind.title)
            XCTAssertThrowsError(try run(kind, kind.example, ["name": big]), kind.title)
        }
    }
}

@MainActor
final class AdditionalUtilityWorkflowTests: XCTestCase {
    func testEveryNewToolKeepsPreviewDraftOptionsAndResultOnOpenAndBack() async throws {
        let suite = "AdditionalUtilities-\(UUID())", defaults: UserDefaults
        defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let settings = AppSettings(defaults: defaults)
        let launcher = LauncherModel(selection: TextSelection(text: "", anchorRect: nil, appName: "Tests", bundleID: nil, pid: nil), snippets: SnippetStore(), defaults: defaults, settings: settings)
        defer { launcher.cancelAll() }
        var opened: [UtilityWorkbenchModel] = []
        launcher.onOpenWorkbench = { opened.append($0) }
        defer { opened.forEach { $0.cancel() } }
        let pasteboardChange = NSPasteboard.general.changeCount
        for kind in AdditionalUtilityKind.allCases {
            launcher.selectedID = kind.id
            let tool = try XCTUnwrap(launcher.expandedSession?.workbench)
            tool.loadUtilityExample()
            for _ in 0..<300 where tool.busy { try await Task.sleep(nanoseconds: 10_000_000) }
            XCTAssertNil(tool.error, kind.title)
            XCTAssertNotNil(tool.result, kind.title)
            let input = tool.input, options = tool.utilityOptions, second = tool.secondInput, output = tool.output
            launcher.selectedID = LauncherCommand.settings.id
            launcher.selectedID = kind.id
            XCTAssertTrue(launcher.expandedSession?.workbench === tool)
            launcher.openSelected()
            XCTAssertTrue(opened.last === tool)
            XCTAssertEqual(tool.input, input); XCTAssertEqual(tool.utilityOptions, options); XCTAssertEqual(tool.secondInput, second); XCTAssertEqual(tool.output, output)
            XCTAssertNil(launcher.expandedSession, "Each tool transfers ownership to its window")
        }
        XCTAssertEqual(NSPasteboard.general.changeCount, pasteboardChange, "Edits, preview focus and opening never copy")
        XCTAssertFalse(defaults.dictionaryRepresentation().values.contains { String(describing: $0).contains("Jefe") }, "The HMAC key is never persisted")
    }
    func testFlattenUseAsInputSwitchesDirectionAndKeepsData() async throws {
        let tool = UtilityWorkbenchModel(command: .jsonFlatten, selection: TextSelection(text: "", anchorRect: nil, appName: "Tests", bundleID: nil, pid: nil), snippets: SnippetStore())
        defer { tool.cancel() }
        tool.loadUtilityExample()
        let original = try DeveloperJSON.parse(tool.input)
        for _ in 0..<100 where tool.busy { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertTrue(tool.canChain)
        tool.useOutputAsInput()
        XCTAssertEqual(tool.utilityOptions["mode"], "Unflatten")
        for _ in 0..<100 where tool.busy { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertEqual(try DeveloperJSON.parse(tool.output), original)
    }
    func testOversizedOptionStopsPreviewAndStaleResultThenRecovers() async throws {
        let tool = UtilityWorkbenchModel(command: .jsonPointer, selection: TextSelection(text: "", anchorRect: nil, appName: "Tests", bundleID: nil, pid: nil), snippets: SnippetStore())
        defer { tool.cancel() }
        tool.previewsOnly = true
        tool.loadUtilityExample()
        tool.utilityOptions["pointer"] = String(repeating: "x", count: LauncherPreview.parsingByteLimit + 1)
        XCTAssertTrue(tool.draftExceedsPreviewLimit)
        XCTAssertFalse(tool.busy)
        XCTAssertNil(tool.result)
        try await Task.sleep(nanoseconds: 300_000_000)
        XCTAssertNil(tool.result)
        tool.utilityOptions["pointer"] = "/users/0/name"
        for _ in 0..<100 where tool.busy { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertEqual(tool.output, "\"Ada\"")
    }
}
