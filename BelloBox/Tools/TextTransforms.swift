import Foundation
import CryptoKit

// MARK: - Case conversion

enum CaseConverter {
    enum Style: String, CaseIterable, Identifiable {
        case upper = "UPPERCASE"
        case lower = "lowercase"
        case title = "Title Case"
        case sentence = "Sentence case"
        case camel = "camelCase"
        case pascal = "PascalCase"
        case snake = "snake_case"
        case kebab = "kebab-case"
        case constant = "CONSTANT_CASE"
        var id: String { rawValue }
    }

    /// Splits text into words, breaking on delimiters and camelCase boundaries.
    static func words(in text: String) -> [String] {
        var result: [String] = []
        var current = ""
        for ch in text {
            if ch.isLetter || ch.isNumber {
                if let last = current.last, last.isLowercase || last.isNumber, ch.isUppercase {
                    result.append(current)
                    current = String(ch)
                } else {
                    current.append(ch)
                }
            } else if !current.isEmpty {
                result.append(current)
                current = ""
            }
        }
        if !current.isEmpty { result.append(current) }
        return result
    }

    static func convert(_ text: String, to style: Style) -> String {
        switch style {
        case .upper:
            return text.uppercased()
        case .lower:
            return text.lowercased()
        case .title:
            return text.split(separator: " ", omittingEmptySubsequences: false)
                .map { $0.isEmpty ? "" : $0.prefix(1).uppercased() + $0.dropFirst().lowercased() }
                .joined(separator: " ")
        case .sentence:
            var out = ""
            var capitalize = true
            for ch in text.lowercased() {
                if capitalize, ch.isLetter {
                    out += ch.uppercased()
                    capitalize = false
                } else {
                    out.append(ch)
                    if ch == "." || ch == "!" || ch == "?" || ch == "\n" { capitalize = true }
                }
            }
            return out
        case .camel:
            let w = words(in: text)
            guard let first = w.first else { return "" }
            return first.lowercased() + w.dropFirst().map { $0.prefix(1).uppercased() + $0.dropFirst().lowercased() }.joined()
        case .pascal:
            return words(in: text).map { $0.prefix(1).uppercased() + $0.dropFirst().lowercased() }.joined()
        case .snake:
            return words(in: text).map { $0.lowercased() }.joined(separator: "_")
        case .kebab:
            return words(in: text).map { $0.lowercased() }.joined(separator: "-")
        case .constant:
            return words(in: text).map { $0.uppercased() }.joined(separator: "_")
        }
    }
}

// MARK: - Encoding

enum TextEncoder {
    enum Method: String, CaseIterable, Identifiable {
        case base64 = "Base64"
        case url = "URL"
        case html = "HTML entities"
        case hex = "Hex"
        var id: String { rawValue }
    }

    private static let urlUnreserved = CharacterSet(
        charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~"
    )

    static func encode(_ text: String, _ method: Method) -> String {
        switch method {
        case .base64:
            return Data(text.utf8).base64EncodedString()
        case .url:
            return text.addingPercentEncoding(withAllowedCharacters: urlUnreserved) ?? text
        case .html:
            return htmlEscape(text)
        case .hex:
            return Data(text.utf8).map { String(format: "%02x", $0) }.joined()
        }
    }

    static func htmlEscape(_ s: String) -> String {
        var out = ""
        for ch in s {
            switch ch {
            case "&": out += "&amp;"
            case "<": out += "&lt;"
            case ">": out += "&gt;"
            case "\"": out += "&quot;"
            case "'": out += "&#39;"
            default: out.append(ch)
            }
        }
        return out
    }
}

// MARK: - Decoding (with auto-detection)

enum TextDecoder {
    struct Decoded: Equatable {
        let format: String
        let output: String
    }

    enum Format: String, CaseIterable, Identifiable {
        case auto = "Auto-detect"
        case base64 = "Base64"
        case url = "URL"
        case html = "HTML entities"
        case hex = "Hex"
        var id: String { rawValue }
    }

    static func decode(_ text: String, as format: Format) -> Decoded? {
        switch format {
        case .auto: return autoDecode(text)
        case .base64: return base64Decode(text).map { Decoded(format: "Base64", output: $0) }
        case .url: return urlDecode(text).map { Decoded(format: "URL", output: $0) }
        case .html: return Decoded(format: "HTML entities", output: htmlUnescape(text))
        case .hex: return hexDecode(text).map { Decoded(format: "Hex", output: $0) }
        }
    }

    static func autoDecode(_ text: String) -> Decoded? {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return nil }

        if t.range(of: "%[0-9A-Fa-f]{2}", options: .regularExpression) != nil,
           let decoded = urlDecode(t), decoded != t {
            return Decoded(format: "URL", output: decoded)
        }
        if t.range(of: "&(#[0-9]+|#x[0-9A-Fa-f]+|[a-zA-Z][a-zA-Z0-9]+);", options: .regularExpression) != nil {
            let decoded = htmlUnescape(t)
            if decoded != t { return Decoded(format: "HTML entities", output: decoded) }
        }
        if looksLikeBase64(t), let decoded = base64Decode(t), isPrintable(decoded) {
            return Decoded(format: "Base64", output: decoded)
        }
        if looksLikeHex(t), let decoded = hexDecode(t), isPrintable(decoded) {
            return Decoded(format: "Hex", output: decoded)
        }
        return nil
    }

    static func base64Decode(_ s: String) -> String? {
        let cleaned = s.trimmingCharacters(in: .whitespacesAndNewlines)
        let data = Data(base64Encoded: cleaned)
            ?? Data(base64Encoded: cleaned, options: .ignoreUnknownCharacters)
        return data.flatMap { String(data: $0, encoding: .utf8) }
    }

    static func urlDecode(_ s: String) -> String? {
        s.replacingOccurrences(of: "+", with: " ").removingPercentEncoding
    }

    static func hexDecode(_ s: String) -> String? {
        let cleaned = s.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: " ", with: "")
        guard cleaned.count % 2 == 0, !cleaned.isEmpty else { return nil }
        var bytes = [UInt8]()
        var index = cleaned.startIndex
        while index < cleaned.endIndex {
            let next = cleaned.index(index, offsetBy: 2)
            guard let byte = UInt8(cleaned[index..<next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        return String(bytes: bytes, encoding: .utf8)
    }

    static func htmlUnescape(_ s: String) -> String {
        var result = s
        let named = ["&amp;": "&", "&lt;": "<", "&gt;": ">", "&quot;": "\"", "&#39;": "'", "&apos;": "'", "&nbsp;": " "]
        for (entity, value) in named {
            result = result.replacingOccurrences(of: entity, with: value)
        }
        // Numeric entities &#NN; and &#xHH;
        if let regex = try? NSRegularExpression(pattern: "&#(x?)([0-9A-Fa-f]+);") {
            let ns = result as NSString
            var output = ""
            var lastEnd = 0
            for match in regex.matches(in: result, range: NSRange(location: 0, length: ns.length)) {
                output += ns.substring(with: NSRange(location: lastEnd, length: match.range.location - lastEnd))
                let isHex = ns.substring(with: match.range(at: 1)) == "x"
                let digits = ns.substring(with: match.range(at: 2))
                if let code = UInt32(digits, radix: isHex ? 16 : 10), let scalar = Unicode.Scalar(code) {
                    output.append(Character(scalar))
                } else {
                    output += ns.substring(with: match.range)
                }
                lastEnd = match.range.location + match.range.length
            }
            output += ns.substring(from: lastEnd)
            result = output
        }
        return result
    }

    // Heuristics

    static func looksLikeBase64(_ s: String) -> Bool {
        let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
        guard t.count >= 8, t.count % 4 == 0 else { return false }
        return t.range(of: "^[A-Za-z0-9+/]+={0,2}$", options: .regularExpression) != nil
    }

    static func looksLikeHex(_ s: String) -> Bool {
        let t = s.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: " ", with: "")
        guard t.count >= 4, t.count % 2 == 0 else { return false }
        return t.range(of: "^[0-9A-Fa-f]+$", options: .regularExpression) != nil
    }

    static func isPrintable(_ s: String) -> Bool {
        guard !s.isEmpty else { return false }
        return !s.unicodeScalars.contains { scalar in
            // Reject C0 control characters other than tab/newline/carriage return.
            scalar.value < 0x20 && scalar != "\t" && scalar != "\n" && scalar != "\r"
        }
    }
}

// MARK: - Hashing

enum HashTool {
    enum Algorithm: String, CaseIterable, Identifiable {
        case md5 = "MD5"
        case sha1 = "SHA-1"
        case sha256 = "SHA-256"
        case sha512 = "SHA-512"
        var id: String { rawValue }
    }

    static func hash(_ text: String, _ algorithm: Algorithm) -> String {
        let data = Data(text.utf8)
        switch algorithm {
        case .md5: return hex(Insecure.MD5.hash(data: data))
        case .sha1: return hex(Insecure.SHA1.hash(data: data))
        case .sha256: return hex(SHA256.hash(data: data))
        case .sha512: return hex(SHA512.hash(data: data))
        }
    }

    private static func hex<D: Sequence>(_ digest: D) -> String where D.Element == UInt8 {
        digest.map { String(format: "%02x", $0) }.joined()
    }
}

// MARK: - Line operations

enum LineTool {
    enum Operation: String, CaseIterable, Identifiable {
        case sortAscending = "Sort A→Z"
        case sortDescending = "Sort Z→A"
        case reverse = "Reverse"
        case dedupe = "Remove duplicates"
        case removeEmpty = "Remove empty lines"
        case trim = "Trim each line"
        var id: String { rawValue }
    }

    static func apply(_ text: String, _ operation: Operation) -> String {
        let normalized = text.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n")
        var lines = normalized.components(separatedBy: "\n")
        switch operation {
        case .sortAscending:
            lines.sort { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
        case .sortDescending:
            lines.sort { $0.localizedCaseInsensitiveCompare($1) == .orderedDescending }
        case .reverse:
            lines.reverse()
        case .dedupe:
            var seen = Set<String>()
            lines = lines.filter { seen.insert($0).inserted }
        case .removeEmpty:
            lines = lines.filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }
        case .trim:
            lines = lines.map { $0.trimmingCharacters(in: .whitespaces) }
        }
        return lines.joined(separator: "\n")
    }
}

// MARK: - Counting

enum TextStats {
    static func characters(_ s: String) -> Int { s.count }

    static func charactersNoSpaces(_ s: String) -> Int {
        s.unicodeScalars.reduce(0) { $1.properties.isWhitespace ? $0 : $0 + 1 }
    }

    static func words(_ s: String) -> Int {
        s.split { $0 == " " || $0 == "\n" || $0 == "\t" || $0 == "\r" }.count
    }

    static func lines(_ s: String) -> Int {
        if s.isEmpty { return 0 }
        let normalized = s.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n")
        return normalized.components(separatedBy: "\n").count
    }
}

// MARK: - Pretty printing (auto-detected)

enum PrettyPrinter {
    struct Result: Equatable {
        let language: String
        let output: String
    }

    static func prettyPrint(_ text: String) -> Result? {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return nil }

        if isJSON(t) {
            return Result(language: "JSON", output: reindent(t))
        }
        if t.hasPrefix("<"), let xml = prettyXML(t) {
            return Result(language: isHTML(t) ? "HTML" : "XML", output: xml)
        }
        if t.contains("{") || t.contains(";") {
            return Result(language: "Code", output: reindent(t))
        }
        return nil
    }

    static func isJSON(_ s: String) -> Bool {
        guard let first = s.first, first == "{" || first == "[" else { return false }
        guard let data = s.data(using: .utf8) else { return false }
        return (try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])) != nil
    }

    static func isHTML(_ s: String) -> Bool {
        let lower = s.lowercased()
        return lower.contains("<!doctype html") || lower.contains("<html")
    }

    static func prettyXML(_ s: String) -> String? {
        guard let data = s.data(using: .utf8) else { return nil }
        let options: XMLNode.Options = isHTML(s) ? [.documentTidyHTML] : []
        guard let document = try? XMLDocument(data: data, options: options) else { return nil }
        let pretty = document.xmlData(options: [.nodePrettyPrint])
        return String(data: pretty, encoding: .utf8)
    }

    /// A quote-aware reindenter for brace/bracket structured text (JSON, CSS, JS).
    static func reindent(_ s: String, indent: String = "  ") -> String {
        var out: [Character] = []
        var depth = 0
        var inString = false
        var delimiter: Character = "\""
        var escaped = false

        func trimTrailingSpaces() { while out.last == " " { out.removeLast() } }
        func newline() {
            trimTrailingSpaces()
            out.append("\n")
            out.append(contentsOf: String(repeating: indent, count: max(0, depth)))
        }
        func spaceIfNeeded() {
            if let last = out.last, last != " ", last != "\n" { out.append(" ") }
        }

        for ch in s {
            if inString {
                out.append(ch)
                if escaped { escaped = false }
                else if ch == "\\" { escaped = true }
                else if ch == delimiter { inString = false }
                continue
            }
            switch ch {
            case "\"", "'":
                inString = true
                delimiter = ch
                out.append(ch)
            case "{", "[":
                trimTrailingSpaces()
                out.append(ch)
                depth += 1
                newline()
            case "}", "]":
                depth = max(0, depth - 1)
                newline()
                out.append(ch)
            case ",":
                trimTrailingSpaces()
                out.append(ch)
                newline()
            case ";":
                trimTrailingSpaces()
                out.append(ch)
                newline()
            case ":":
                trimTrailingSpaces()
                out.append(ch)
                out.append(" ")
            case " ", "\t", "\n", "\r":
                spaceIfNeeded()
            default:
                out.append(ch)
            }
        }

        return String(out)
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map { line -> String in
                var s = String(line)
                while s.hasSuffix(" ") { s.removeLast() }
                return s
            }
            .filter { !$0.isEmpty }
            .joined(separator: "\n")
    }
}
