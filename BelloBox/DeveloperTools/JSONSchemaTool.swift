import Foundation

/// A deliberately explicit 2020-12 subset. Unknown validation keywords are errors,
/// including regex/format and remote refs; they must never turn into a false pass.
enum JSONSchemaTool {
    static let supported: Set<String> = ["$schema", "$defs", "$ref", "$comment", "title", "description", "default", "examples", "readOnly", "writeOnly", "deprecated", "type", "enum", "const", "properties", "required", "additionalProperties", "minProperties", "maxProperties", "items", "prefixItems", "minItems", "maxItems", "uniqueItems", "minLength", "maxLength", "minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum", "allOf", "anyOf", "oneOf", "not"]
    static let types: Set<String> = ["null", "boolean", "object", "array", "number", "integer", "string"]
    static func validate(_ input: String, schema text: String) throws -> WorkbenchResult {
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { throw UtilityError("Add a JSON schema in the second editor, or load the example. Remote references and unsupported keywords are reported.") }
        let value = try DeveloperJSON.parse(input), schema = try DeveloperJSON.parse(text)
        var checker = Checker(root: schema)
        try checker.lint(schema, depth: 0)
        let issues = try checker.check(value, schema: schema, path: "", depth: 0)
        let body = issues.isEmpty ? "Valid against this schema.\n\nSupported: types, enum/const, required/properties, object/array/string bounds, numeric bounds, prefixItems/items, uniqueItems, allOf/anyOf/oneOf/not, and local $defs/$ref.\n\nUnsupported keywords are rejected, never ignored. No remote resources are fetched." : issues.prefix(100).joined(separator: "\n") + (issues.count >= 100 ? "\nShowing the first 100 issues." : "")
        return .init(text: body, status: issues.isEmpty ? "Valid · supported JSON Schema 2020-12 subset" : "\(issues.count >= 100 ? "100+" : String(issues.count)) validation issues · paths use JSON Pointer", warning: !issues.isEmpty)
    }
    private struct Checker {
        let root: DeveloperJSON
        var budget = 50_000
        var references: [String: DeveloperJSON] = [:]
        mutating func resolve(_ pointer: String) throws -> DeveloperJSON {
            if let cached = references[pointer] { return cached }
            let value = try JSONPointerTool.resolve(root, pointer: pointer)
            references[pointer] = value
            return value
        }
        mutating func step(_ depth: Int) throws {
            try DataWorkshop.spend(&budget)
            guard depth < 64 else { throw UtilityError("Schema recursion exceeds 64 levels. Check for a reference cycle.") }
        }
        mutating func lint(_ schema: DeveloperJSON, depth: Int) throws {
            try step(depth)
            if case .bool = schema { return }
            guard case .object(let fields) = schema else { throw UtilityError("A schema must be a JSON object or boolean.") }
            if let unknown = fields.keys.sorted().first(where: { !supported.contains($0) }) { throw UtilityError("Unsupported schema keyword: \(unknown.prefix(80)). This validator checks a local 2020-12 subset; it will not ignore constraints.") }
            if let dialect = fields["$schema"], dialect != .string("https://json-schema.org/draft/2020-12/schema") { throw UtilityError("Use the 2020-12 schema dialect, or omit $schema.") }
            if let ref = fields["$ref"] {
                guard case .string(let pointer) = ref, pointer.hasPrefix("#") else { throw UtilityError("Only local JSON Pointer references (# or #/…) are supported.") }
                _ = try resolve(pointer)
            }
            if let type = fields["type"] {
                let names: [String]
                if case .string(let name) = type { names = [name] }
                else if case .array(let array) = type { names = try array.map { value in guard case .string(let name) = value else { throw UtilityError("Schema types must be strings.") }; return name } }
                else { throw UtilityError("Schema type must be a name or list of names.") }
                guard !names.isEmpty, Set(names).count == names.count, names.allSatisfy(types.contains) else { throw UtilityError("Use unique JSON type names in type.") }
            }
            if let enumeration = fields["enum"] { guard case .array(let values) = enumeration, !values.isEmpty else { throw UtilityError("Schema enum needs a non-empty array.") } }
            if let required = fields["required"] {
                guard case .array(let values) = required else { throw UtilityError("Schema required must be an array of unique strings.") }
                let names = try values.map { value -> Data in guard case .string(let name) = value else { throw UtilityError("Schema required must contain strings.") }; return Data(name.utf8) }
                guard Set(names).count == names.count else { throw UtilityError("Schema required contains duplicate names.") }
            }
            for key in ["minProperties", "maxProperties", "minItems", "maxItems", "minLength", "maxLength"] where fields[key] != nil {
                guard case .number(let text) = fields[key]!, let number = try JSONExactNumber(text).integerValue, number >= 0 else { throw UtilityError("\(key) must be a non-negative whole number within this Mac's integer range.") }
            }
            for key in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] where fields[key] != nil {
                guard case .number(let text) = fields[key]! else { throw UtilityError("\(key) must be a number.") }
                _ = try JSONExactNumber(text)
            }
            if let unique = fields["uniqueItems"], case .bool = unique {} else if fields["uniqueItems"] != nil { throw UtilityError("uniqueItems must be a boolean.") }
            for key in ["properties", "$defs"] {
                if let container = fields[key] {
                    guard case .object(let schemas) = container else { throw UtilityError("\(key) must be an object of schemas.") }
                    for child in schemas.values { try lint(child, depth: depth + 1) }
                }
            }
            for key in ["items", "additionalProperties", "not"] { if let child = fields[key] { try lint(child, depth: depth + 1) } }
            for key in ["prefixItems", "allOf", "anyOf", "oneOf"] {
                if let container = fields[key] {
                    guard case .array(let schemas) = container, !schemas.isEmpty else { throw UtilityError("\(key) must be a non-empty array of schemas.") }
                    for child in schemas { try lint(child, depth: depth + 1) }
                }
            }
        }
        mutating func canonical(_ value: DeveloperJSON) throws -> Data {
            func encode(_ node: DeveloperJSON, budget: inout Int) throws -> String {
                try DataWorkshop.spend(&budget)
                switch node {
                case .number(let text): return "n" + (try JSONExactNumber(text)).canonical
                case .array(let children): return "[" + (try children.map { try encode($0, budget: &budget) }).joined(separator: ",") + "]"
                case .object(let fields): return "{" + (try fields.keys.sorted().map { DeveloperJSON.quoted($0) + ":" + (try encode(fields[$0]!, budget: &budget)) }).joined(separator: ",") + "}"
                default: return node.formatted(pretty: false)
                }
            }
            return Data(try encode(value, budget: &budget).utf8)
        }
        private func valueTag(_ value: DeveloperJSON) -> String { if case .string = value { return "s" }; return "" }
        mutating func check(_ value: DeveloperJSON, schema: DeveloperJSON, path: String, depth: Int) throws -> [String] {
            try step(depth)
            if case .bool(let allowed) = schema { return allowed ? [] : ["\(path.isEmpty ? "(root)" : path): value is not allowed"] }
            guard case .object(let fields) = schema else { throw UtilityError("A referenced schema must be an object or boolean.") }
            // References can point at any location, including inside annotations.
            // Lint the resolved node too before using it as a schema.
            var errors: [String] = []
            func issue(_ message: String) { if errors.count < 100 { errors.append("\(path.isEmpty ? "(root)" : String(path.prefix(512))): \(message)") } }
            if case .string(let ref) = fields["$ref"] {
                let resolved = try resolve(ref)
                try lint(resolved, depth: depth + 1)
                errors += try check(value, schema: resolved, path: path, depth: depth + 1)
            }
            if let type = fields["type"] {
                let names: [String]
                if case .array(let list) = type { names = list.map(\.scalarText) } else { names = [type.scalarText] }
                if try !names.contains(where: { try matches(value, type: $0) }) { issue("expected " + names.joined(separator: " or ")); return Array(errors.prefix(100)) }
            }
            if let constant = fields["const"], try canonical(value) != canonical(constant) { issue("does not match const") }
            if case .array(let values) = fields["enum"] {
                let actual = try canonical(value)
                var matched = false
                for candidate in values { if try canonical(candidate) == actual { matched = true; break } }
                if !matched { issue("not one of the allowed enum values") }
            }
            switch value {
            case .object(let object):
                let keys = Set(object.keys.map { Data($0.utf8) })
                if case .array(let required) = fields["required"] {
                    for name in required.map(\.scalarText) {
                        try DataWorkshop.spend(&budget)
                        if !keys.contains(Data(name.utf8)) { issue("missing required field \(DeveloperJSON.quoted(String(name.prefix(80))))") }
                    }
                }
                var properties: [String: DeveloperJSON] = [:]
                if case .object(let provided) = fields["properties"] { properties = provided }
                let exactProperties = Dictionary(uniqueKeysWithValues: properties.map { (Data($0.key.utf8), $0.value) })
                for key in object.keys.sorted() {
                    try DataWorkshop.spend(&budget)
                    if let child = exactProperties[Data(key.utf8)] ?? fields["additionalProperties"] {
                        errors += try check(object[key]!, schema: child, path: path + "/" + JSONPointerTool.escape(key), depth: depth + 1)
                        if errors.count >= 100 { break }
                    }
                }
                bounds(object.count, fields: fields, minimum: "minProperties", maximum: "maxProperties", label: "properties", issue: issue)
            case .array(let array):
                let prefix: [DeveloperJSON]
                if case .array(let children) = fields["prefixItems"] { prefix = children } else { prefix = [] }
                for (i, child) in array.enumerated() {
                    if let rule = i < prefix.count ? prefix[i] : fields["items"] {
                        errors += try check(child, schema: rule, path: path + "/\(i)", depth: depth + 1)
                        if errors.count >= 100 { break }
                    }
                }
                if fields["uniqueItems"] == .bool(true) {
                    var seen = Set<Data>()
                    for child in array { if !seen.insert(try canonical(child)).inserted { issue("array items must be unique"); break } }
                }
                bounds(array.count, fields: fields, minimum: "minItems", maximum: "maxItems", label: "items", issue: issue)
            case .string(let text): bounds(text.unicodeScalars.count, fields: fields, minimum: "minLength", maximum: "maxLength", label: "Unicode code points", issue: issue)
            case .number(let text):
                for key in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
                    if case .number(let threshold) = fields[key] {
                        let comparison = try JSONExactNumber(text).compare(JSONExactNumber(threshold))
                        if key == "minimum" && comparison < 0 || key == "maximum" && comparison > 0 || key == "exclusiveMinimum" && comparison <= 0 || key == "exclusiveMaximum" && comparison >= 0 { issue("does not satisfy \(key) \(threshold.prefix(80))") }
                    }
                }
            default: break
            }
            for key in ["allOf", "anyOf", "oneOf"] {
                if case .array(let branches) = fields[key] {
                    var passes = 0
                    for branch in branches { if try check(value, schema: branch, path: path, depth: depth + 1).isEmpty { passes += 1 } }
                    if key == "allOf" && passes != branches.count || key == "anyOf" && passes == 0 || key == "oneOf" && passes != 1 { issue("\(key): matched \(passes) of \(branches.count) branches") }
                }
            }
            if let negative = fields["not"], try check(value, schema: negative, path: path, depth: depth + 1).isEmpty { issue("matches a disallowed not schema") }
            return Array(errors.prefix(100))
        }
        func matches(_ value: DeveloperJSON, type: String) throws -> Bool {
            switch (value, type) {
            case (.null, "null"), (.bool, "boolean"), (.object, "object"), (.array, "array"), (.string, "string"), (.number, "number"): return true
            case (.number(let text), "integer"): return try JSONExactNumber(text).isInteger
            default: return false
            }
        }
        func bounds(_ count: Int, fields: [String: DeveloperJSON], minimum: String, maximum: String, label: String, issue: (String) -> Void) {
            if case .number(let text) = fields[minimum], let min = (try? JSONExactNumber(text))?.integerValue, count < min { issue("needs at least \(min) \(label)") }
            if case .number(let text) = fields[maximum], let max = (try? JSONExactNumber(text))?.integerValue, count > max { issue("allows at most \(max) \(label)") }
        }
    }
}

/// Compare decimal JSON numbers without conversion to floating point or huge expansion.
struct JSONExactNumber {
    let negative: Bool, digits: String, exponent: Int
    init(_ text: String) throws {
        guard text.utf8.count <= 4_096 else { throw UtilityError("Schema numeric comparisons support up to 4,096 digits.") }
        let parts = text.lowercased().split(separator: "e", omittingEmptySubsequences: false)
        guard parts.count <= 2, let power = parts.count == 2 ? Int(parts[1]) : 0, (-1_000_000...1_000_000).contains(power) else { throw UtilityError("Schema numeric exponents must be within ±1,000,000.") }
        let mantissa = String(parts[0]), sign = mantissa.hasPrefix("-"), unsigned = sign ? String(mantissa.dropFirst()) : mantissa
        let decimal = unsigned.split(separator: ".", omittingEmptySubsequences: false)
        var digits = decimal.joined(), exponent = power - (decimal.count == 2 ? decimal[1].count : 0)
        digits = String(digits.drop(while: { $0 == "0" }))
        if digits.isEmpty { self.negative = false; self.digits = "0"; self.exponent = 0; return }
        while digits.last == "0" { digits.removeLast(); exponent += 1 }
        self.negative = sign; self.digits = digits; self.exponent = exponent
    }
    var canonical: String { (negative ? "-" : "") + digits + "e" + String(exponent) }
    var isInteger: Bool { digits == "0" || exponent >= 0 }
    var integerValue: Int? {
        guard isInteger, exponent >= 0, digits.count + exponent <= 19 else { return nil }
        return Int((negative ? "-" : "") + digits + String(repeating: "0", count: exponent))
    }
    func compare(_ other: Self) -> Int {
        if canonical == other.canonical { return 0 }
        if negative != other.negative { return negative ? -1 : 1 }
        let sign = negative ? -1 : 1
        if digits == "0" { return -sign }
        if other.digits == "0" { return sign }
        let a = digits.count + exponent, b = other.digits.count + other.exponent
        if a != b { return (a < b ? -1 : 1) * sign }
        let size = max(digits.count, other.digits.count)
        let left = digits + String(repeating: "0", count: size - digits.count), right = other.digits + String(repeating: "0", count: size - other.digits.count)
        return (left < right ? -1 : 1) * sign
    }
}
