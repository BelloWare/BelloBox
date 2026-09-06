import Foundation

struct UtilityError: LocalizedError, Equatable {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

enum UtilityLimits {
    static let inputBytes = 512_000
    static func check(_ text: String) throws {
        guard text.utf8.count <= inputBytes else { throw UtilityError("Use up to 500 KB of text at a time.") }
        try Task.checkCancellation()
    }
}

/// Keeps number lexemes intact: formatting IDs must never round them through Double.
indirect enum DeveloperJSON: Equatable {
    case null, bool(Bool), string(String), number(String)
    case array([DeveloperJSON]), object([String: DeveloperJSON])

    static func parse(_ text: String) throws -> DeveloperJSON {
        try UtilityLimits.check(text)
        var parser = Parser(bytes: Array(text.utf8))
        let value = try parser.value(depth: 0)
        parser.whitespace()
        guard parser.index == parser.bytes.count else { throw parser.error("Unexpected content after JSON") }
        return value
    }

    var scalarText: String {
        if case let .string(value) = self { return value }
        return formatted(pretty: false)
    }

    func formatted(pretty: Bool = true, depth: Int = 0) -> String {
        let pad = String(repeating: "  ", count: depth)
        func collection(_ values: [String], _ open: String, _ close: String) -> String {
            guard !values.isEmpty else { return open + close }
            if !pretty { return open + values.joined(separator: ",") + close }
            return open + "\n" + values.map { pad + "  " + $0 }.joined(separator: ",\n") + "\n" + pad + close
        }
        switch self {
        case .null: return "null"
        case .bool(let value): return value ? "true" : "false"
        case .number(let value): return value
        case .string(let value): return Self.quoted(value)
        case .array(let values): return collection(values.map { $0.formatted(pretty: pretty, depth: depth + 1) }, "[", "]")
        case .object(let values):
            return collection(values.keys.sorted().map {
                Self.quoted($0) + (pretty ? ": " : ":") + values[$0]!.formatted(pretty: pretty, depth: depth + 1)
            }, "{", "}")
        }
    }

    static func quoted(_ value: String) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.withoutEscapingSlashes]
        return String(decoding: try! encoder.encode(value), as: UTF8.self)
    }

    private struct Parser {
        let bytes: [UInt8]
        var index = 0
        var current: UInt8? { index < bytes.count ? bytes[index] : nil }
        mutating func whitespace() { while let c = current, [9, 10, 13, 32].contains(c) { index += 1 } }
        func error(_ text: String) -> UtilityError {
            let prefix = bytes.prefix(index)
            let line = prefix.filter { $0 == 10 }.count + 1
            let column = index - (prefix.lastIndex(of: 10) ?? -1)
            return UtilityError("\(text) at line \(line), byte column \(column).")
        }
        mutating func consume(_ byte: UInt8) -> Bool {
            whitespace()
            guard current == byte else { return false }
            index += 1
            return true
        }
        mutating func value(depth: Int) throws -> DeveloperJSON {
            guard depth < 64 else { throw error("JSON nesting exceeds 64 levels") }
            whitespace()
            switch current ?? 0 {
            case 34: return .string(try string())
            case 123:
                index += 1
                var values: [String: DeveloperJSON] = [:]
                if consume(125) { return .object(values) }
                repeat {
                    whitespace()
                    guard current == 34 else { throw error("Expected a quoted property name") }
                    let key = try string()
                    guard values[key] == nil else { throw error("Duplicate property \(Self.keyDescription(key))") }
                    guard consume(58) else { throw error("Expected ':'") }
                    values[key] = try value(depth: depth + 1)
                    if consume(125) { return .object(values) }
                    guard consume(44) else { throw error("Expected ',' or '}'") }
                } while true
            case 91:
                index += 1
                var values: [DeveloperJSON] = []
                if consume(93) { return .array(values) }
                repeat {
                    values.append(try value(depth: depth + 1))
                    if consume(93) { return .array(values) }
                    guard consume(44) else { throw error("Expected ',' or ']'") }
                } while true
            case 116: try literal("true"); return .bool(true)
            case 102: try literal("false"); return .bool(false)
            case 110: try literal("null"); return .null
            case 45, 48...57:
                let start = index
                if current == 45 { index += 1 }
                if current == 48 { index += 1 }
                else { try digits() }
                if current == 46 { index += 1; try digits() }
                if current == 101 || current == 69 {
                    index += 1
                    if current == 43 || current == 45 { index += 1 }
                    try digits()
                }
                return .number(String(decoding: bytes[start..<index], as: UTF8.self))
            default: throw error("Expected a JSON value")
            }
        }
        static func keyDescription(_ key: String) -> String { String(key.prefix(60)) }
        mutating func digits() throws {
            let start = index
            while let c = current, (48...57).contains(c) { index += 1 }
            guard index > start else { throw error("Expected a digit") }
        }
        mutating func literal(_ text: String) throws {
            let expected = Array(text.utf8)
            guard bytes.dropFirst(index).starts(with: expected) else { throw error("Expected \(text)") }
            index += expected.count
        }
        mutating func string() throws -> String {
            let start = index
            index += 1
            var escaped = false
            while let c = current {
                index += 1
                if !escaped && c == 34 {
                    do { return try JSONDecoder().decode(String.self, from: Data(bytes[start..<index])) }
                    catch { throw self.error("Invalid JSON string or escape") }
                }
                if c < 32 { throw error("Unescaped control character") }
                if escaped { escaped = false }
                else if c == 92 { escaped = true }
            }
            throw error("Unclosed string")
        }
    }
}
