import Foundation

enum MathTool {
    static func display(_ value: Double) -> String { value == 0 ? "0" : String(format: "%.15g", locale: Locale(identifier: "en_US_POSIX"), value) }
    static func calculate(_ input: String) throws -> WorkbenchResult {
        guard input.utf8.count <= 4_096 else { throw UtilityError("Keep expressions within 4 KB.") }
        var parser = Parser(chars: Array(input.lowercased()))
        let value = try parser.expression()
        parser.space()
        guard parser.index == parser.chars.count else { throw UtilityError("Unexpected character at position \(parser.index + 1).") }
        guard value.isFinite else { throw UtilityError("The expression has no finite real result. Check division by zero and function domains.") }
        return WorkbenchResult(text: display(value), status: "Calculated · 15 significant digits · angles in radians")
    }
    private struct Parser {
        let chars: [Character]
        var index = 0, depth = 0
        func finite(_ value: Double) throws -> Double {
            guard value.isFinite else { throw UtilityError("A step in the expression has no finite real result. Check division by zero and function domains.") }
            return value
        }
        mutating func space() { while index < chars.count && chars[index].isWhitespace { index += 1 } }
        mutating func take(_ c: Character) -> Bool { space(); if index < chars.count && chars[index] == c { index += 1; return true }; return false }
        mutating func expression() throws -> Double {
            var v = try term()
            while true { if take("+") { v += try term() } else if take("-") { v -= try term() } else { return try finite(v) } }
        }
        mutating func term() throws -> Double {
            var v = try unary()
            while true { if take("*") { v *= try unary() } else if take("/") { v /= try unary() } else if take("%") { v = v.truncatingRemainder(dividingBy: try unary()) } else { return try finite(v) } }
        }
        mutating func unary() throws -> Double {
            depth += 1; defer { depth -= 1 }
            guard depth <= 64 else { throw UtilityError("The expression is nested too deeply (maximum 64).") }
            if take("+") { return try unary() }
            if take("-") { return -(try unary()) }
            let value = try primary()
            if take("^") { return pow(value, try unary()) }
            return value
        }
        mutating func primary() throws -> Double {
            space()
            if take("(") { let v = try expression(); guard take(")") else { throw UtilityError("Add the closing parenthesis.") }; return v }
            let start = index
            while index < chars.count && (chars[index].isASCII && (chars[index].isLetter || (index > start && chars[index].isNumber))) { index += 1 }
            if index > start {
                let name = String(chars[start..<index])
                if name == "pi" { return .pi }; if name == "e" { return M_E }
                guard take("(") else { throw UtilityError("Unknown constant \(name). Functions need parentheses.") }
                let v = try expression()
                var second: Double?
                if take(",") { second = try expression() }
                guard take(")") else { throw UtilityError("Add the closing parenthesis after \(name).") }
                if let second {
                    switch name { case "min": return min(v, second); case "max": return max(v, second); case "pow": return pow(v, second); default: throw UtilityError("\(name) does not take two arguments.") }
                }
                switch name {
                case "sqrt": return sqrt(v); case "abs": return abs(v); case "sin": return sin(v); case "cos": return cos(v); case "tan": return tan(v)
                case "ln": return log(v); case "log", "log10": return log10(v); case "exp": return exp(v); case "floor": return floor(v); case "ceil": return ceil(v); case "round": return v.rounded()
                default: throw UtilityError("Use sqrt, abs, sin, cos, tan, ln, log, exp, floor, ceil, round, min, max, or pow.")
                }
            }
            while index < chars.count && ((chars[index].isASCII && chars[index].isNumber) || chars[index] == ".") { index += 1 }
            if index > start && index < chars.count && chars[index] == "e" {
                index += 1
                if index < chars.count && (chars[index] == "+" || chars[index] == "-") { index += 1 }
                while index < chars.count && chars[index].isASCII && chars[index].isNumber { index += 1 }
            }
            guard index > start, let value = Double(String(chars[start..<index])), value.isFinite else { throw UtilityError("Enter a number or function at position \(start + 1).") }
            return value
        }
    }
}

enum UnitTool {
    struct Unit { let name: String, dimension: String; let scale: Double; var offset: Double = 0 }
    static let units: [Unit] = [
        .init(name: "m", dimension: "Length", scale: 1), .init(name: "km", dimension: "Length", scale: 1000), .init(name: "cm", dimension: "Length", scale: 0.01), .init(name: "mm", dimension: "Length", scale: 0.001), .init(name: "in", dimension: "Length", scale: 0.0254), .init(name: "ft", dimension: "Length", scale: 0.3048), .init(name: "yd", dimension: "Length", scale: 0.9144), .init(name: "mi", dimension: "Length", scale: 1609.344),
        .init(name: "kg", dimension: "Mass", scale: 1), .init(name: "g", dimension: "Mass", scale: 0.001), .init(name: "lb", dimension: "Mass", scale: 0.45359237), .init(name: "oz", dimension: "Mass", scale: 0.028349523125),
        .init(name: "°C", dimension: "Temperature", scale: 1, offset: 273.15), .init(name: "°F", dimension: "Temperature", scale: 5/9, offset: 273.15 - 32 * 5/9), .init(name: "K", dimension: "Temperature", scale: 1),
        .init(name: "B", dimension: "Data", scale: 1), .init(name: "bit", dimension: "Data", scale: 0.125), .init(name: "kB", dimension: "Data", scale: 1000), .init(name: "MB", dimension: "Data", scale: 1e6), .init(name: "GB", dimension: "Data", scale: 1e9), .init(name: "TB", dimension: "Data", scale: 1e12), .init(name: "KiB", dimension: "Data", scale: 1024), .init(name: "MiB", dimension: "Data", scale: 1048576), .init(name: "GiB", dimension: "Data", scale: 1073741824), .init(name: "TiB", dimension: "Data", scale: 1099511627776),
        .init(name: "s", dimension: "Duration", scale: 1), .init(name: "ms", dimension: "Duration", scale: 0.001), .init(name: "min", dimension: "Duration", scale: 60), .init(name: "h", dimension: "Duration", scale: 3600), .init(name: "day", dimension: "Duration", scale: 86400), .init(name: "week", dimension: "Duration", scale: 604800),
        .init(name: "m/s", dimension: "Speed", scale: 1), .init(name: "km/h", dimension: "Speed", scale: 1/3.6), .init(name: "mph", dimension: "Speed", scale: 0.44704), .init(name: "knot", dimension: "Speed", scale: 1852/3600)
    ]
    static func convert(_ input: String, from: String, to: String) throws -> WorkbenchResult {
        guard let value = Double(input.trimmingCharacters(in: .whitespacesAndNewlines)), value.isFinite else { throw UtilityError("Enter a finite number, without a unit or thousands separators.") }
        guard let a = units.first(where: { $0.name == from }), let b = units.first(where: { $0.name == to }), a.dimension == b.dimension else { throw UtilityError("Choose two units of the same kind.") }
        let base = value * a.scale + a.offset
        guard a.dimension != "Temperature" || base >= -1e-10 else { throw UtilityError("Temperature cannot be below absolute zero.") }
        let converted = (base - b.offset) / b.scale
        guard converted.isFinite else { throw UtilityError("This value is too large to convert.") }
        return WorkbenchResult(text: "\(MathTool.display(converted)) \(b.name)", status: "\(a.dimension) · \(MathTool.display(value)) \(a.name) → \(b.name)")
    }
}

enum NumberBaseTool {
    static func convert(_ input: String, base: Int, outputBase: Int? = nil) throws -> WorkbenchResult {
        guard [2, 8, 10, 16].contains(base) else { throw UtilityError("Choose base 2, 8, 10, or 16.") }
        var raw = input.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let negative = raw.hasPrefix("-")
        if raw.hasPrefix("-") || raw.hasPrefix("+") { raw.removeFirst() }
        let prefix = base == 2 ? "0b" : base == 8 ? "0o" : base == 16 ? "0x" : ""
        if !prefix.isEmpty && raw.hasPrefix(prefix) { raw.removeFirst(2) }
        guard !raw.isEmpty, raw.utf8.count <= 256 else { throw UtilityError("Enter 1–256 digits in the chosen base.") }
        let alphabet = Array("0123456789abcdef")
        var bytes = [0]
        for char in raw {
            guard let digit = alphabet.firstIndex(of: char), digit < base else { throw UtilityError("‘\(char)’ is not a base-\(base) digit.") }
            var carry = digit
            for i in bytes.indices { let value = bytes[i] * base + carry; bytes[i] = value % 256; carry = value / 256 }
            while carry > 0 { bytes.append(carry % 256); carry /= 256 }
        }
        func format(_ radix: Int) -> String {
            var remaining = bytes, digits: [Character] = []
            while remaining.contains(where: { $0 != 0 }) {
                var remainder = 0
                for i in remaining.indices.reversed() { let value = remainder * 256 + remaining[i]; remaining[i] = value / radix; remainder = value % radix }
                digits.append(alphabet[remainder])
            }
            let value = digits.isEmpty ? "0" : String(digits.reversed())
            return negative && value != "0" ? "-" + value : value
        }
        if let outputBase {
            guard [2, 8, 10, 16].contains(outputBase) else { throw UtilityError("Choose output base 2, 8, 10, or 16.") }
            return WorkbenchResult(text: format(outputBase), status: "Exact integer · base \(base) → \(outputBase)")
        }
        return WorkbenchResult(text: "Decimal  \(format(10))\nHex      \(format(16).uppercased())\nOctal    \(format(8))\nBinary   \(format(2))", status: "Exact integer · no floating-point rounding")
    }
}
