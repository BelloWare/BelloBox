import Foundation

enum TextUtility {
    static func swiftLiteral(_ text: String) -> String {
        var output = "\""
        for scalar in text.unicodeScalars {
            switch scalar.value {
            case 34: output += "\\\""
            case 92: output += "\\\\"
            case 10: output += "\\n"
            case 13: output += "\\r"
            case 9: output += "\\t"
            case 0...31, 127...159, 0x2028, 0x2029: output += "\\u{\(String(scalar.value, radix: 16))}"
            default: output.unicodeScalars.append(scalar)
            }
        }
        return output + "\""
    }
    static func run(_ kind: AdditionalUtilityKind, input: String, second: String, options: [String: String]) throws -> WorkbenchResult {
        switch kind {
        case .unicode:
            let mode = options["mode"]!
            let normalized: String
            switch mode {
            case "NFC": normalized = input.precomposedStringWithCanonicalMapping
            case "NFD": normalized = input.decomposedStringWithCanonicalMapping
            case "NFKC": normalized = input.precomposedStringWithCompatibilityMapping
            case "NFKD": normalized = input.decomposedStringWithCompatibilityMapping
            default:
                let scalars = Array(input.unicodeScalars.prefix(4_001))
                guard scalars.count <= 4_000 else { throw UtilityError("Inspect up to 4,000 code points at a time. Normalization supports the full 500 KB input limit.") }
                let lines = try scalars.enumerated().map { index, scalar -> String in
                    try Task.checkCancellation()
                    let character = String(scalar)
                    let utf8 = character.utf8.map { String(format: "%02X", $0) }.joined(separator: " ")
                    let utf16 = character.utf16.map { String(format: "%04X", $0) }.joined(separator: " ")
                    return "\(index + 1). U+\(String(format: "%04X", scalar.value))  \(scalar.properties.name ?? "Unnamed / control")\n   \(DeveloperJSON.string(character).formatted(pretty: false))  UTF-8: \(utf8)  UTF-16: \(utf16)"
                }
                return WorkbenchResult(text: lines.joined(separator: "\n"), status: "\(input.count) graphemes · \(scalars.count) code points · \(input.utf8.count) UTF-8 bytes")
            }
            return WorkbenchResult(text: normalized, status: "\(mode) normalization · \(input.unicodeScalars.elementsEqual(normalized.unicodeScalars) ? "already normalized" : "code points changed")")
        case .stringEscape:
            let result: String
            switch options["mode"] {
            case "JSON unquote":
                guard case .string(let value) = try DeveloperJSON.parse(input) else { throw UtilityError("Unquote expects one JSON string, including its double quotes.") }
                result = value
            case "Swift literal": result = swiftLiteral(input)
            case "Shell quote":
                guard !input.contains("\0") else { throw UtilityError("A shell argument cannot contain a NUL character.") }
                result = "'" + input.replacingOccurrences(of: "'", with: "'\"'\"'") + "'"
            default: result = DeveloperJSON.string(input).formatted(pretty: false)
            }
            return WorkbenchResult(text: result, status: "\(options["mode"]!) · literal text only")
        case .extract:
            let detector = try NSDataDetector(types: NSTextCheckingResult.CheckingType.link.rawValue)
            let wantEmail = options["mode"] == "Emails", unique = options["duplicates"] == "Unique"
            var seen = Set<String>(), values: [String] = [], tooMany = false
            detector.enumerateMatches(in: input, range: NSRange(input.startIndex..., in: input)) { match, _, stop in
                guard !Task.isCancelled else { stop.pointee = true; return }
                guard let match, let url = match.url, (url.scheme?.lowercased() == "mailto") == wantEmail, let range = Range(match.range, in: input) else { return }
                let value = String(input[range])
                if !unique || seen.insert(value).inserted { values.append(value) }
                if values.count > 5_000 { tooMany = true; stop.pointee = true }
            }
            try Task.checkCancellation()
            guard !tooMany else { throw UtilityError("Extract up to 5,000 matches at a time.") }
            return WorkbenchResult(text: values.joined(separator: "\n"), status: "\(values.count) \(wantEmail ? "email addresses" : "links") · \(unique ? "duplicates removed" : "all occurrences")")
        case .listSet:
            let mode = options["matching"]!, trim = mode != "Exact", insensitive = mode == "Trim & ignore case"
            func list(_ text: String) -> [(String, String)] {
                var seen = Set<String>()
                return text.components(separatedBy: .newlines).compactMap { line in
                    let value = trim ? line.trimmingCharacters(in: .whitespaces) : line
                    guard !value.trimmingCharacters(in: .whitespaces).isEmpty else { return nil }
                    let key = insensitive ? value.lowercased(with: Locale(identifier: "en_US_POSIX")) : value
                    return seen.insert(key).inserted ? (key, value) : nil
                }
            }
            let a = list(input), b = list(second), keysA = Set(a.map(\.0)), keysB = Set(b.map(\.0))
            let result: [(String, String)]
            switch options["mode"] {
            case "Intersection": result = a.filter { keysB.contains($0.0) }
            case "A − B": result = a.filter { !keysB.contains($0.0) }
            case "B − A": result = b.filter { !keysA.contains($0.0) }
            case "Symmetric difference": result = a.filter { !keysB.contains($0.0) } + b.filter { !keysA.contains($0.0) }
            default: result = a + b.filter { !keysA.contains($0.0) }
            }
            return WorkbenchResult(text: result.map(\.1).joined(separator: "\n"), status: "\(result.count) items · stable order · blank lines and duplicates removed")
        case .semver: return try SemanticVersionTool.run(input, mode: options["mode"]!)
        default: throw UtilityError("Choose a text tool.")
        }
    }
}

struct SemanticVersion: Comparable {
    let original: String, core: [String], prerelease: [String]
    init(_ text: String) throws {
        guard text.utf8.count <= 256,
              text.range(of: "^(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\\.[0-9A-Za-z-]+)*)?(\\+[0-9A-Za-z-]+(\\.[0-9A-Za-z-]+)*)?$", options: .regularExpression) == text.startIndex..<text.endIndex else { throw UtilityError("‘\(String(text.prefix(60)))’ is not a semantic version. Use major.minor.patch, optionally -prerelease and +build.") }
        original = text
        let version = text.split(separator: "+", maxSplits: 1)[0]
        let parts = version.split(separator: "-", maxSplits: 1)
        core = parts[0].split(separator: ".").map(String.init)
        prerelease = parts.count == 2 ? parts[1].split(separator: ".").map(String.init) : []
        guard prerelease.allSatisfy({ !Self.numeric($0) || $0 == "0" || !$0.hasPrefix("0") }) else { throw UtilityError("Numeric prerelease identifiers cannot have leading zeros.") }
    }
    static func numeric(_ s: String) -> Bool { s.utf8.allSatisfy { (48...57).contains($0) } }
    static func numberLess(_ a: String, _ b: String) -> Bool { a.count == b.count ? a < b : a.count < b.count }
    static func == (a: Self, b: Self) -> Bool { a.core == b.core && a.prerelease == b.prerelease }
    static func < (a: Self, b: Self) -> Bool {
        for (x, y) in zip(a.core, b.core) where x != y { return numberLess(x, y) }
        if a.prerelease.isEmpty { return false }
        if b.prerelease.isEmpty { return true }
        for (x, y) in zip(a.prerelease, b.prerelease) where x != y {
            let nx = numeric(x), ny = numeric(y)
            if nx != ny { return nx }
            return nx ? numberLess(x, y) : x < y
        }
        return a.prerelease.count < b.prerelease.count
    }
}

enum SemanticVersionTool {
    static func run(_ input: String, mode: String) throws -> WorkbenchResult {
        let lines = input.components(separatedBy: .newlines).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        guard !lines.isEmpty, lines.count <= 5_000 else { throw UtilityError("Provide 1–5,000 versions, one per line.") }
        let versions = try lines.map(SemanticVersion.init)
        if mode == "Compare first two" {
            guard versions.count == 2 else { throw UtilityError("Comparison needs exactly two versions, one per line.") }
            let a = versions[0], b = versions[1], relation = a == b ? "=" : a < b ? "<" : ">"
            return WorkbenchResult(text: "\(a.original) \(relation) \(b.original)", status: "Semantic precedence · build metadata does not affect ordering")
        }
        let sorted = versions.enumerated().sorted { a, b in a.element == b.element ? a.offset < b.offset : mode == "Descending" ? b.element < a.element : a.element < b.element }
        return WorkbenchResult(text: sorted.map(\.element.original).joined(separator: "\n"), status: "\(versions.count) versions · SemVer 2.0 · equal versions keep their input order")
    }
}
