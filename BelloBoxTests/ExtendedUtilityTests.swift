import AppKit
import CryptoKit
import XCTest
@testable import BelloBox

final class ExtendedUtilityTests: XCTestCase {
    private func run(_ kind: AdditionalUtilityKind, _ input: String, _ options: [String: String] = [:], second: String = "") throws -> WorkbenchResult {
        try AdditionalUtilityEngine.run(kind, input: input, second: second, options: options)
    }
    func testExactlyTwentyMoreToolsAreDiscoverableAndGroupedWithoutHidingOldTools() {
        XCTAssertEqual(AdditionalUtilityKind.allCases.filter { $0.extendedDefinition != nil }.count, 20)
        XCTAssertEqual(DeveloperToolBrowser.commands(query: "", group: nil, newOnly: false).count, 51)
        XCTAssertEqual(DeveloperToolBrowser.commands(query: "", group: nil, newOnly: true).count, 20)
        XCTAssertEqual(DeveloperToolBrowser.commands(query: "bezier", group: nil, newOnly: true), [.bezier])
        XCTAssertTrue(DeveloperToolBrowser.commands(query: "JSON", group: .design, newOnly: true).isEmpty)
        let grouped = AdditionalUtilityKind.Group.allCases.flatMap { DeveloperToolBrowser.commands(query: "", group: $0, newOnly: false) }
        XCTAssertEqual(Set(grouped).count, 51)
        for kind in AdditionalUtilityKind.allCases where kind.extendedDefinition != nil {
            XCTAssertEqual(LauncherCommand.search(kind.title, input: "", favorites: [], recents: []).first, kind.command)
        }
    }
    func testSelectionSuggestionsRecognizeNewFormatsWithoutDisplacingJSONOrTime() {
        XCTAssertEqual(LauncherCommand.suggestions(for: AdditionalUtilityKind.certificate.example).first, .certificate)
        XCTAssertEqual(LauncherCommand.suggestions(for: AdditionalUtilityKind.sshKey.example).first, .sshKey)
        XCTAssertEqual(LauncherCommand.suggestions(for: AdditionalUtilityKind.uuidInspect.example).first, .uuidInspect)
        XCTAssertEqual(LauncherCommand.suggestions(for: "Cookie: a=1").first, .cookies)
        XCTAssertEqual(LauncherCommand.suggestions(for: AdditionalUtilityKind.plist.example).first, .plist)
        XCTAssertEqual(LauncherCommand.suggestions(for: AdditionalUtilityKind.jsonLines.example).first, .jsonLines)
        XCTAssertEqual(LauncherCommand.suggestions(for: "{\n\"x\": 1\n}").first, .json)
        XCTAssertEqual(LauncherCommand.suggestions(for: "2026-09-12T12:00:00Z").first, .worldClock)
    }
    func testSchemaValidatesTypesRequiredPathsAndAdditionalProperties() throws {
        let schema = ##"{"type":"object","required":["name"],"properties":{"a/b":{"type":"array","items":{"type":"integer","minimum":0}}},"additionalProperties":false}"##
        let result = try run(.jsonSchema, ##"{"a/b":[-1,"bad"]}"##, second: schema)
        XCTAssertTrue(result.warning)
        XCTAssertTrue(result.text.contains("missing required field"))
        XCTAssertTrue(result.text.contains("/a~1b/0")); XCTAssertTrue(result.text.contains("/a~1b/1"))
        let kind = AdditionalUtilityKind.jsonSchema
        XCTAssertFalse(try run(kind, kind.example, second: kind.secondExample).warning)
        XCTAssertTrue(try run(.jsonSchema, "null", second: "false").warning)
        XCTAssertFalse(try run(.jsonSchema, "null", second: "true").warning)
    }
    func testSchemaComparesNumbersExactlyAndCountsUnicodeCodePoints() throws {
        XCTAssertTrue(try run(.jsonSchema, "9007199254740993", second: ##"{"maximum":9007199254740992}"##).warning)
        XCTAssertFalse(try run(.jsonSchema, "1e3", second: ##"{"type":"integer","const":1000.00}"##).warning)
        XCTAssertTrue(try run(.jsonSchema, "1.01", second: ##"{"type":"integer"}"##).warning)
        XCTAssertTrue(try run(.jsonSchema, #""e\u0301""#, second: ##"{"maxLength":1}"##).warning)
        XCTAssertFalse(try run(.jsonSchema, #""é""#, second: ##"{"maxLength":1.0}"##).warning)
        XCTAssertTrue(try run(.jsonSchema, #""é""#, second: ##"{"const":"e\u0301"}"##).warning)
        XCTAssertTrue(try run(.jsonSchema, "[1,1.0]", second: ##"{"uniqueItems":true}"##).warning)
        XCTAssertFalse(try run(.jsonSchema, #"["é","e\u0301"]"#, second: ##"{"uniqueItems":true}"##).warning)
        XCTAssertThrowsError(try run(.jsonSchema, "1", second: ##"{"minimum":1e-9223372036854775808}"##))
        for (a,b,expected) in [("-1e20","-99999999999999999999",-1),("0","-0.001",1),("0.0100","1e-2",0),("1e1000000","9e999999",1)] {
            XCTAssertEqual(try JSONExactNumber(a).compare(JSONExactNumber(b)), expected)
        }
    }
    func testSchemaLocalReferencesBranchingAndUnknownConstraints() throws {
        let schema = ##"{"$defs":{"count":{"type":"integer","minimum":1}},"type":"array","items":{"$ref":"#/$defs/count"}}"##
        XCTAssertFalse(try run(.jsonSchema, "[1,2,3]", second: schema).warning)
        XCTAssertTrue(try run(.jsonSchema, "[0]", second: schema).warning)
        XCTAssertTrue(try run(.jsonSchema, "1", second: ##"{"oneOf":[{"type":"number"},{"type":"integer"}]}"##).warning)
        XCTAssertFalse(try run(.jsonSchema, #""x""#, second: ##"{"anyOf":[{"type":"number"},{"type":"string"}],"not":{"const":"y"}}"##).warning)
        XCTAssertTrue(try run(.jsonSchema, "3", second: ##"{"allOf":[{"minimum":1},{"maximum":2}]}"##).warning)
        XCTAssertTrue(try run(.jsonSchema, "[1,2]", second: ##"{"prefixItems":[{"type":"integer"}],"items":false}"##).warning)
        for schema in [##"{"$ref":"https://example.com/schema"}"##, ##"{"properties":{"unused":{"pattern":"evil"}}}"##, ##"{"$ref":"#"}"##, ##"{"items":{},"unexpected":true}"##, ##"{"$defs":{"n":{"type":"number"}},"$ref":"#/$defs/n","minimum":"0"}"##] {
            XCTAssertThrowsError(try run(.jsonSchema, "{}", second: schema), schema)
        }
        XCTAssertThrowsError(try run(.jsonSchema, "{}", second: ##"{"default":{"pattern":"x"},"$ref":"#/default"}"##), "References into annotations must be linted")
    }
    func testMergePatchRFCExamplesAndExactNumbers() throws {
        let examples = [
            (##"{"a":"b"}"##, ##"{"a":"c"}"##, ##"{"a":"c"}"##),
            (##"{"a":"b"}"##, ##"{"b":"c"}"##, ##"{"a":"b","b":"c"}"##),
            (##"{"a":"b"}"##, ##"{"a":null}"##, ##"{}"##),
            (##"{"a":"b","b":"c"}"##, ##"{"a":null}"##, ##"{"b":"c"}"##),
            (##"{"a":["b"]}"##, ##"{"a":"c"}"##, ##"{"a":"c"}"##),
            (##"{"a":"c"}"##, ##"{"a":["b"]}"##, ##"{"a":["b"]}"##),
            (##"{"a":{"b":"c"}}"##, ##"{"a":{"b":"d","c":null}}"##, ##"{"a":{"b":"d"}}"##),
            (##"{"a":"b"}"##, #"["c"]"#, #"["c"]"#),
            (##"{"a":"foo"}"##, "null", "null"),
            (##"{"a":9007199254740993}"##, ##"{"b":1e1000}"##, ##"{"a":9007199254740993,"b":1e1000}"##)
        ]
        for (original, patch, expected) in examples {
            XCTAssertEqual(try DeveloperJSON.parse(run(.jsonMerge, original, second: patch).text), try DeveloperJSON.parse(expected))
        }
        XCTAssertThrowsError(try run(.jsonMerge, "{}"))
    }
    func testRedactorOnlyReplacesRequestedKeysThroughoutArrays() throws {
        let result = try run(.jsonRedact, ##"{"user":{"TOKEN":"keep-private","name":"Ada"},"entries":[{"token":123},{"safe":true}],"note":"token"}"##, ["keys":"token"])
        XCTAssertTrue(result.status.hasPrefix("2 fields"))
        XCTAssertFalse(result.text.contains("keep-private"))
        XCTAssertTrue(result.text.contains("Ada")); XCTAssertTrue(result.text.contains("\"note\": \"token\""))
        XCTAssertThrowsError(try run(.jsonRedact, "{}", ["keys":""]))
    }
    func testJSONLinesRoundTripReportsPhysicalLineAndKeepsNumbers() throws {
        let lines = "\r\n{\"id\":9007199254740993}\r\n\r\n[1,\"a\\nb\"]\r\n"
        let array = try run(.jsonLines, lines).text
        let back = try run(.jsonLines, array, ["mode":"Array → Lines"]).text
        XCTAssertEqual(back, "{\"id\":9007199254740993}\n[1,\"a\\nb\"]")
        XCTAssertEqual(try run(.jsonLines, back).text, array)
        XCTAssertThrowsError(try run(.jsonLines, "{}\n\ninvalid")) { XCTAssertTrue($0.localizedDescription.contains("line 3")) }
    }
    func testCSVFiltersBeforeProjectionAndRemovesProjectedDuplicates() throws {
        let input = "name,team,city\nAda,Platform,London\nAda,Platform,Tokyo\nLin,Design,London"
        let output = try run(.csvExplore, input, ["filter":"platform","columns":"name,team","rows":"Unique"])
        XCTAssertEqual(try CSVCodec.rows(output.text), [["name","team"],["Ada","Platform"]])
        guard case .table(let preview) = output.visual else { return XCTFail("Missing table") }
        XCTAssertEqual(preview.totalRows, 1)
        let quoted = try run(.csvExplore, "\"a,b\",c\n1,2", ["columns":"\"a,b\""])
        XCTAssertEqual(try CSVCodec.rows(quoted.text), [["a,b"],["1"]])
        XCTAssertThrowsError(try run(.csvExplore, input, ["columns":"missing"]))
        XCTAssertThrowsError(try run(.csvExplore, "a,a\n1,2"))
    }
    func testCSVPreviewBoundsDoNotTruncateCopiedResults() throws {
        let input = "index,value\n" + (0..<250).map { "\($0),v\($0)" }.joined(separator: "\n")
        let output = try run(.csvExplore, input)
        guard case .table(let preview) = output.visual else { return XCTFail("Missing table") }
        XCTAssertEqual(preview.rows.count, 100); XCTAssertEqual(preview.totalRows, 250)
        XCTAssertEqual(try CSVCodec.rows(output.text).count, 251)
        let manyColumns = (0..<40).map { "column\($0)" }.joined(separator: ",")
        let wide = try run(.csvExplore, manyColumns + "\n" + (0..<40).map(String.init).joined(separator: ","))
        guard case .table(let table) = wide.visual else { return XCTFail("Missing wide table") }
        XCTAssertEqual(table.columns.count, 30); XCTAssertEqual(table.totalColumns, 40)
        XCTAssertEqual(try CSVCodec.rows(wide.text)[0].count, 40)
        XCTAssertThrowsError(try run(.csvExplore, input, ["columns":"index\nvalue"]))
    }
    func testEnvironmentLiteralRoundTripAndRejectedAmbiguity() throws {
        let input = "export NAME=\"Bello Box\" # name\nRAW='${USER} $(echo hello)'\nHASH=a#b\nEMPTY=\nLINES=\"one\\ntwo\""
        let json = try run(.envFile, input).text
        let env = try run(.envFile, json, ["mode":"JSON → Env"]).text
        XCTAssertEqual(try run(.envFile, env).text, json)
        XCTAssertTrue(json.contains("${USER} $(echo hello)"))
        for value in ["KEY=1\nKEY=2", "1KEY=x", "KEY='missing", "KEY=\"ok\" extra"] { XCTAssertThrowsError(try run(.envFile, value)) }
        XCTAssertThrowsError(try run(.envFile, ##"{"KEY":2}"##, ["mode":"JSON → Env"]))
    }
    func testPlistTypedRoundTripPreservesDatesDataScalarsAndReservedNames() throws {
        let input = "<plist version=\"1.0\"><dict><key>type</key><string>data</string><key>empty</key><array/><key>data</key><data>AAH/</data><key>date</key><date>2026-09-12T00:00:00Z</date><key>int</key><integer>18446744073709551615</integer><key>real</key><real>2.5</real><key>flag</key><false/></dict></plist>"
        let json = try run(.plist, input).text, xml = try run(.plist, json, ["mode":"JSON → Plist"]).text
        XCTAssertEqual(try run(.plist, xml).text, json)
        let native = try PropertyListSerialization.propertyList(from: Data(xml.utf8), format: nil) as? [String: Any]
        XCTAssertEqual(native?["data"] as? Data, Data([0,1,255]))
        XCTAssertNotNil(native?["date"] as? Date)
        XCTAssertEqual((native?["int"] as? NSNumber)?.uint64Value, UInt64.max)
    }
    func testPlistRejectsEntitiesDuplicateKeysControlsAndWrongScalarTypes() throws {
        for input in ["<!DOCTYPE plist [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><plist><string>&x;</string></plist>", "<plist><dict><key>a</key><true/><key>a</key><false/></dict></plist>", "<plist><string><true/></string></plist>", "<plist><date>2026-02-30T00:00:00Z</date></plist>", "<plist><array><key>x</key></array></plist>"] { XCTAssertThrowsError(try run(.plist, input)) }
        XCTAssertThrowsError(try run(.plist, ##"{"type":"string","value":"\u0000"}"##, ["mode":"JSON → Plist"]))
        XCTAssertThrowsError(try run(.plist, ##"{"type":"integer","value":"2"}"##, ["mode":"JSON → Plist"]))
        XCTAssertThrowsError(try run(.plist, "<plist>" + String(repeating: "<array>", count: 65) + String(repeating: "</array>", count: 65) + "</plist>"))
    }
    func testSQLFormattingPreservesTokenSemanticsAndIsIdempotent() throws {
        let input = #"select .5,1e-3,0xFF,E'a\'b',"Case",[a]]b],:name,@param,$1,?12, $$line; -- raw$$, foo::int from t where a<=2 and b->>'name'='x''y';"# + "\n-- trailing comment\nselect 'a'\n'b'; /* nested /* inner */ outer */"
        let formatted = try run(.sqlFormat, input).text
        let before = try SQLFormatterTool.tokenize(input), after = try SQLFormatterTool.tokenize(formatted)
        func normalize(_ token: SQLFormatterTool.Token) -> String { token.kind == .word && SQLFormatterTool.keywords.contains(token.text.uppercased()) ? token.text.uppercased() : token.text }
        XCTAssertEqual(before.map(normalize), after.map(normalize))
        XCTAssertTrue(formatted.contains("1e-3")); XCTAssertTrue(formatted.contains(":name")); XCTAssertTrue(formatted.contains("'a'\n'b'"))
        XCTAssertEqual(try run(.sqlFormat, formatted).text, formatted)
        XCTAssertThrowsError(try run(.sqlFormat, "select 'unclosed"))
        XCTAssertThrowsError(try run(.sqlFormat, "select /* unclosed"))
    }
    func testHeadersPreserveDuplicatesPseudoFieldsAndOmitBody() throws {
        let input = "HTTP/1.1 200 OK\r\nSet-Cookie: a=1\r\nset-cookie: b=2\r\nContent-Type: text/plain\r\n\r\nnot a header"
        let result = try run(.httpHeaders, input)
        XCTAssertTrue(result.status.contains("1 repeated")); XCTAssertTrue(result.status.contains("body omitted")); XCTAssertFalse(result.text.contains("not a header"))
        guard case .object(let object) = try DeveloperJSON.parse(result.text), case .array(let headers) = object["headers"] else { return XCTFail("Missing headers") }
        XCTAssertEqual(headers.count, 3)
        XCTAssertTrue(try run(.httpHeaders, ":method: GET\n:authority: example.com").text.contains(":authority"))
        for input in ["Bad Name: value", "Header: value\n folded", "Header: x\u{00}y"] { XCTAssertThrowsError(try run(.httpHeaders, input)) }
    }
    func testSQLDialectCommentsEscapesAndOperatorsKeepTheirMeaning() throws {
        for (sql, dialect) in [("select 1+-- inline\n2;", "PostgreSQL"), (#"select U&'d\0061t', U&"d\0061t";"#, "PostgreSQL"), (#"select 'a\'b', a--b;"# + "\n# comment\nselect 1;", "MySQL")] {
            let result = try run(.sqlFormat, sql, ["dialect":dialect,"keywords":"Preserve"])
            XCTAssertEqual(try SQLFormatterTool.tokenize(sql, dialect: dialect).map(\.text), try SQLFormatterTool.tokenize(result.text, dialect: dialect).map(\.text))
        }
        let csv = try run(.csvExplore, "word\né\ne\u{301}", ["rows":"Unique"])
        XCTAssertEqual(try CSVCodec.rows(csv.text).count, 3, "Unicode normalization must not silently remove a CSV row")
    }
    func testCookiePairsAndAttributesRemainLiteralAndOrdered() throws {
        let input = "Set-Cookie: id=a=b; Expires=Wed, 09 Jun 2027 10:18:14 GMT; Secure; HttpOnly; Path=/; Path=/api\nSet-Cookie: theme=dark; SameSite=Lax"
        let result = try run(.cookies, input)
        guard case .array(let cookies) = try DeveloperJSON.parse(result.text), case .object(let first) = cookies[0], case .array(let attributes) = first["attributes"] else { return XCTFail("Missing cookies") }
        XCTAssertEqual(cookies.count, 2); XCTAssertEqual(attributes.count, 5); XCTAssertEqual(first["value"], .string("a=b"))
        let request = try run(.cookies, "Cookie: a=1; a=2; b=x=y", ["mode":"Cookie"])
        guard case .array(let pairs) = try DeveloperJSON.parse(request.text) else { return XCTFail("Missing request cookies") }
        XCTAssertEqual(pairs.count, 3)
        XCTAssertThrowsError(try run(.cookies, "a=1, b=2"))
    }
    private func sshField(_ bytes: [UInt8]) -> [UInt8] { let n = UInt32(bytes.count); return [UInt8(n >> 24),UInt8((n >> 16) & 255),UInt8((n >> 8) & 255),UInt8(n & 255)] + bytes }
    func testSSHKeyFingerprintsAndWireValidation() throws {
        let bytes = sshField(Array("ssh-ed25519".utf8)) + sshField(Array(0..<32))
        let key = "ssh-ed25519 " + Data(bytes).base64EncodedString()
        let hash = Data(SHA256.hash(data: Data(bytes))).base64EncodedString().replacingOccurrences(of: "=", with: "")
        XCTAssertTrue(try run(.sshKey, key).text.contains("SHA256:" + hash))
        XCTAssertThrowsError(try run(.sshKey, key.replacingOccurrences(of: "ssh-ed25519 ", with: "ssh-rsa ")))
        XCTAssertThrowsError(try run(.sshKey, "ssh-ed25519 " + Data(bytes + [0]).base64EncodedString()))
        XCTAssertThrowsError(try run(.sshKey, "-----BEGIN OPENSSH PRIVATE KEY-----"))
        let point = P256.Signing.PrivateKey().publicKey.x963Representation
        let ecdsa = sshField(Array("ecdsa-sha2-nistp256".utf8)) + sshField(Array("nistp256".utf8)) + sshField(Array(point))
        XCTAssertTrue(try run(.sshKey, "ecdsa-sha2-nistp256 " + Data(ecdsa).base64EncodedString()).text.contains("nistp256"))
        let rsa = sshField(Array("ssh-rsa".utf8)) + sshField([1,0,1]) + sshField([0,0x80,1])
        XCTAssertTrue(try run(.sshKey, "ssh-rsa " + Data(rsa).base64EncodedString()).text.contains("16-bit RSA"))
    }
    func testCertificateReadsPublicFieldsAndRejectsKeysAndBrokenPEM() throws {
        let result = try run(.certificate, CertificateTool.example)
        XCTAssertTrue(result.text.contains("Bello Box Example")); XCTAssertTrue(result.text.contains("SHA-256")); XCTAssertTrue(result.text.contains("2048 bits"))
        XCTAssertTrue(result.text.contains("Valid from  2026-09-12T")); XCTAssertTrue(result.text.contains("Valid until  2036-"))
        XCTAssertTrue(try run(.certificate, CertificateTool.example + "\n" + CertificateTool.example).status.hasPrefix("2 certificates"))
        for input in ["-----BEGIN PRIVATE KEY-----", "garbage " + CertificateTool.example, "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----"] { XCTAssertThrowsError(try run(.certificate, input)) }
    }
    func testUUIDRFCVectorsAndVariants() throws {
        let first = try run(.uuidInspect, "c232ab00-9414-11ec-b3c8-9f6bdeced846").text
        let sixth = try run(.uuidInspect, "1ec9414c-232a-6b00-b3c8-9f6bdeced846").text
        XCTAssertTrue(first.contains("2022-02-22T19:22:22.000Z")); XCTAssertTrue(sixth.contains("2022-02-22T19:22:22.000Z"))
        XCTAssertTrue(try run(.uuidInspect, AdditionalUtilityKind.uuidInspect.example).text.contains("2022-02-22T19:22:22.000Z"))
        XCTAssertTrue(try run(.uuidInspect, "urn:uuid:00000000-0000-0000-0000-000000000000").text.contains("Nil UUID"))
        XCTAssertTrue(try run(.uuidInspect, String(repeating:"f",count:32)).text.contains("Max UUID"))
        XCTAssertThrowsError(try run(.uuidInspect, "0000-00000000-0000-0000-000000000000"))
    }
    func testBitwiseWidthsSignedInterpretationRotationsAndOverflow() throws {
        for value in 0...255 {
            let result = try run(.bitwise, String(value), ["operand":"85","operation":"XOR"])
            XCTAssertTrue(result.text.hasPrefix("Unsigned  \(value ^ 85)\n"))
        }
        XCTAssertTrue(try run(.bitwise, "-1", ["width":"64","operation":"NOT"]).text.hasPrefix("Unsigned  0\n"))
        XCTAssertTrue(try run(.bitwise, "18446744073709551615", ["width":"64","operation":"OR","operand":"0"]).text.contains("Signed    -1"))
        XCTAssertTrue(try run(.bitwise, "128", ["operation":"Rotate left","operand":"1"]).text.hasPrefix("Unsigned  1\n"))
        XCTAssertTrue(try run(.bitwise, "1", ["operation":"Rotate right","operand":"1"]).text.hasPrefix("Unsigned  128\n"))
        XCTAssertThrowsError(try run(.bitwise, "256")); XCTAssertThrowsError(try run(.bitwise, "-129"))
        XCTAssertThrowsError(try run(.bitwise, "1", ["width":"64","operation":"Left shift","operand":"64"]))
    }
    func testStatisticsStableMomentsPercentilesAndHistogram() throws {
        let result = try run(.statistics, "1,2,3,4,5")
        XCTAssertTrue(result.text.contains("Mean        3")); XCTAssertTrue(result.text.contains("Median      3")); XCTAssertTrue(result.text.contains("P25         2"))
        XCTAssertTrue(result.text.contains("1.4142135623731"))
        XCTAssertTrue(try run(.statistics, "1,2,3,4,5", ["population":"Sample"]).text.contains("1.58113883008419"))
        guard case .statistics(let chart) = result.visual else { return XCTFail("Missing histogram") }
        XCTAssertEqual(chart.bins.reduce(0,+), 5)
        XCTAssertTrue(try run(.statistics, "4", ["population":"Sample"]).text.contains("Needs at least 2"))
        XCTAssertFalse(try run(.statistics, "1e150,-1e150,1e150,-1e150").text.lowercased().contains("nan"))
        for bad in ["1,NaN", "1,Infinity", "1e151", "x", String(repeating:"1 ",count:20_001)] { XCTAssertThrowsError(try run(.statistics, bad)) }
    }
    func testCalendarArithmeticLeapClampingWeekdaysAndDST() throws {
        XCTAssertTrue(try run(.dateMath, "2024-02-29", ["unit":"Years","amount":"1"]).text.contains("2025-02-28"))
        XCTAssertTrue(try run(.dateMath, "2026-01-31", ["unit":"Months","amount":"1"]).text.contains("2026-02-28"))
        XCTAssertTrue(try run(.dateMath, "2026-09-11", ["unit":"Weekdays","amount":"1"]).text.contains("2026-09-14"))
        XCTAssertTrue(try run(.dateMath, "2026-09-14", ["unit":"Weekdays","amount":"-1"]).text.contains("2026-09-11"))
        let dst = try run(.dateMath, "2026-03-08", ["unit":"Days","amount":"1","zone":"America/New_York"])
        XCTAssertTrue(dst.text.contains("82800 seconds"))
        for input in ["2026-02-30", "0000-01-01", "2026-09-12T24:00:00Z", "2026-09-12T12:00:00+99:00"] { XCTAssertThrowsError(try run(.dateMath, input)) }
        XCTAssertThrowsError(try run(.dateMath, "9999-12-31", ["unit":"Days","amount":"1"]))
    }
    func testAspectRatioAndTargetRounding() throws {
        let result = try run(.aspectRatio, "1920×1080", ["size":"640"])
        XCTAssertTrue(result.text.contains("16:9")); XCTAssertTrue(result.text.contains("640 × 360"))
        XCTAssertTrue(try run(.aspectRatio, "3:2", ["size":"10"]).text.contains("10 × 7"))
        XCTAssertThrowsError(try run(.aspectRatio, "0x1080"))
        XCTAssertThrowsError(try run(.aspectRatio, "1x10000000", ["size":"10000000"]))
    }
    func testBezierCSSMappingPresetsAndShadowLimits() throws {
        XCTAssertEqual(try BezierCurve.parse("linear").value(at: 0.5), 0.5, accuracy: 1e-12)
        XCTAssertEqual(try BezierCurve.parse("ease-in-out").value(at: 0.5), 0.5, accuracy: 1e-12)
        XCTAssertEqual(try BezierCurve.parse("ease").value(at: 0.5), 0.802403, accuracy: 1e-6)
        XCTAssertTrue(try run(.bezier, "cubic-bezier(0.25, 0.1, 0.25, 1)").text.contains("transition-timing-function: cubic-bezier(0.25, 0.1, 0.25, 1);"))
        XCTAssertThrowsError(try run(.bezier, "-1,0,1,1")); XCTAssertThrowsError(try run(.bezier, "0,NaN,1,1"))
        XCTAssertEqual(try run(.boxShadow, "#0008").text, "box-shadow: 0px 8px 24px 0px #00000088;")
        XCTAssertThrowsError(try run(.boxShadow, "#000", ["blur":"-1"]))
    }
    func testTextTablesEscapeHTMLMarkdownAndMultilineCells() throws {
        let input = "name,value\n\"<script>\",\"a|b\nc\""
        let html = try run(.textTable, input, ["format":"HTML"]).text
        XCTAssertTrue(html.contains("&lt;script&gt;")); XCTAssertFalse(html.contains("<script>")); XCTAssertTrue(html.contains("<br>"))
        let markdown = try run(.textTable, input).text
        XCTAssertTrue(markdown.contains("a\\|b<br>c"))
        XCTAssertTrue(try run(.textTable, "name,count\nAda,2", ["format":"Plain text"]).text.contains("Ada"))
    }
    func testShadowPreviewKeepsSoftEdgesWhenRasterized() throws {
        let context = try XCTUnwrap(CGContext(data: nil, width: 400, height: 200, bitsPerComponent: 8, bytesPerRow: 1600, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        let spec = BoxShadowSpec(color: try UtilityColor.parse("#00000080"), x: 0, y: 8, blur: 24, spread: 0)
        ShadowPreviewNSView.draw(spec, size: CGSize(width: 400, height: 200), scale: 1, in: context)
        let pixels = try XCTUnwrap(context.data).assumingMemoryBound(to: UInt8.self)
        let alpha = Set((0..<200).map { pixels[$0 * 1600 + 200 * 4 + 3] })
        XCTAssertGreaterThan(alpha.count, 8, "Blur must produce a soft gradient, not a solid offset shape")
        XCTAssertEqual(pixels[3], 0, "The offscreen caster must never appear in the preview")
    }
}

@MainActor
final class ExtendedUtilityWorkflowTests: XCTestCase {
    private func model(_ kind: AdditionalUtilityKind, input: String) -> UtilityWorkbenchModel {
        UtilityWorkbenchModel(command: kind.command, selection: TextSelection(text: input, anchorRect: nil, appName: "Tests", bundleID: nil, pid: nil), snippets: SnippetStore())
    }
    func testConverterDirectionsFollowSelectionAndRoundTripThroughUseAsInput() async throws {
        for kind in [AdditionalUtilityKind.jsonLines, .envFile, .plist] {
            let model = model(kind, input: kind.example); defer { model.cancel() }
            model.schedule()
            for _ in 0..<200 where model.busy { try await Task.sleep(nanoseconds: 10_000_000) }
            XCTAssertNil(model.error); XCTAssertTrue(model.canChain)
            let forward = model.output
            let reverseModel = self.model(kind, input: forward); defer { reverseModel.cancel() }
            XCTAssertEqual(reverseModel.utilityOptions["mode"], kind.options[0].choices[1])
            model.useOutputAsInput()
            for _ in 0..<200 where model.busy { try await Task.sleep(nanoseconds: 10_000_000) }
            XCTAssertNil(model.error)
            model.useOutputAsInput()
            for _ in 0..<200 where model.busy { try await Task.sleep(nanoseconds: 10_000_000) }
            XCTAssertEqual(model.output, forward)
        }
        XCTAssertEqual(model(.cookies, input: "Cookie: a=1").utilityOptions["mode"], "Cookie")
    }
    func testSecondDraftAndEveryOptionParticipateInPreviewLimitAndRecover() async throws {
        for kind in AdditionalUtilityKind.allCases where kind.extendedDefinition != nil {
            let model = model(kind, input: kind.example); defer { model.cancel() }
            model.previewsOnly = true; model.loadUtilityExample()
            let second = model.secondInput
            model.secondInput = String(repeating: "x", count: LauncherPreview.parsingByteLimit + 1)
            XCTAssertTrue(model.draftExceedsPreviewLimit); XCTAssertFalse(model.busy); XCTAssertNil(model.result)
            model.secondInput = second
            for _ in 0..<200 where model.busy { try await Task.sleep(nanoseconds: 10_000_000) }
            XCTAssertNil(model.error, kind.title); XCTAssertNotNil(model.result, kind.title)
            model.utilityOptions["testOversizedOption"] = String(repeating: "x", count: LauncherPreview.parsingByteLimit + 1)
            XCTAssertTrue(model.draftExceedsPreviewLimit); XCTAssertFalse(model.busy); XCTAssertNil(model.result)
        }
    }
}
