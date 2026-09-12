import Foundation

enum JSONCodeTool {
    private indirect enum Shape {
        case bool, number, string, null, any
        case array(Shape), object([String: Shape]), nullable(Shape)
    }
    static func generate(_ value: DeveloperJSON, language: String, name: String) throws -> String {
        guard name.range(of: "^[A-Z][A-Za-z0-9_]{0,63}$", options: .regularExpression) == name.startIndex..<name.endIndex,
              !["String", "Double", "Decimal", "Bool", "Int", "Array", "Dictionary", "Optional", "Any", "Self", "Type", "JSONValue", "Date", "Data", "URL", "Codable", "CodingKey", "Decoder", "Encoder"].contains(name) else { throw UtilityError("Use a unique type name starting with a capital letter (letters, digits, underscores; up to 64 characters).") }
        var budget = 5_000
        func infer(_ value: DeveloperJSON) throws -> Shape {
            budget -= 1; guard budget >= 0 else { throw UtilityError("Infer types from up to 5,000 JSON values at a time.") }
            try Task.checkCancellation()
            switch value {
            case .null: return .null
            case .bool: return .bool
            case .number: return .number
            case .string: return .string
            case .array(let array):
                guard array.count <= 2_000 else { throw UtilityError("Use at most 2,000 samples in one array.") }
                var shape: Shape?
                for child in array { let next = try infer(child); shape = shape.map { merge($0, next) } ?? next }
                return .array(shape ?? .any)
            case .object(let object):
                guard object.count <= 200 else { throw UtilityError("Use at most 200 fields per object.") }
                return .object(try object.mapValues(infer))
            }
        }
        let shape = try infer(value), swift = language == "Swift"
        var declarations: [String] = [], usedNames: Set<String> = [name, "JSONValue"], needsJSONValue = false, typeCount = 0
        func uniqueName(_ hint: String) -> String {
            let base = hint.utf8.count <= 64 ? hint : String(hint.prefix(48))
            var result = base, suffix = 2
            while usedNames.contains(result) { result = base + String(suffix); suffix += 1 }
            usedNames.insert(result); return result
        }
        func type(_ shape: Shape, hint: String, rootName: String? = nil) throws -> String {
            switch shape {
            case .bool: return swift ? "Bool" : "boolean"
            case .number: return swift ? "Decimal" : "number"
            case .string: return swift ? "String" : "string"
            case .any, .null: needsJSONValue = true; return swift ? "JSONValue" : (isNull(shape) ? "null" : "unknown")
            case .nullable(let wrapped): return swift ? "\(try type(wrapped, hint: hint))?" : "(\(try type(wrapped, hint: hint)) | null)"
            case .array(let child): return swift ? "[\(try type(child, hint: hint + "Item"))]" : "Array<\(try type(child, hint: hint + "Item"))>"
            case .object(let fields):
                typeCount += 1; guard typeCount <= 300 else { throw UtilityError("The inferred model exceeds 300 types.") }
                let typeName = rootName ?? uniqueName(hint), keys = fields.keys.sorted()
                var lines: [String] = [], codingKeys: [String] = [], propertyNames = Set<String>()
                for (index, key) in keys.enumerated() {
                    let field = fields[key]!, optional: Bool, wrapped: Shape
                    if case .nullable(let child) = field { optional = true; wrapped = child } else if case .null = field { optional = true; wrapped = .any } else { optional = false; wrapped = field }
                    let childHint = typeName + "Field\(index + 1)"
                    let childType = try type(wrapped, hint: childHint)
                    if swift {
                        var property = key.range(of: "^[A-Za-z_][A-Za-z0-9_]*$", options: .regularExpression) != nil && !["_", "self", "Self", "Type", "Protocol", "CodingKeys"].contains(key) ? key : "field\(index + 1)"
                        while propertyNames.contains(property) { property += "_" }
                        propertyNames.insert(property)
                        lines.append("    let `\(property)`: \(childType)\(optional ? "?" : "")")
                        codingKeys.append("        case `\(property)` = \(TextUtility.swiftLiteral(key))")
                    } else {
                        lines.append("    \(DeveloperJSON.string(key).formatted(pretty: false))\(optional ? "?" : ""): \(childType)\(optional ? " | null" : "");")
                    }
                }
                if swift && keys.isEmpty { declarations.append("struct \(typeName): Codable {}"); return typeName }
                let body: String
                let properties = lines.joined(separator: "\n")
                if swift {
                    let keysBody = codingKeys.joined(separator: "\n")
                    body = "struct \(typeName): Codable {\n\(properties)\n\n    enum CodingKeys: String, CodingKey {\n\(keysBody)\n    }\n}"
                } else { body = "export interface \(typeName) {\n\(properties)\n}" }
                declarations.append(body); return typeName
            }
        }
        let rootType: String
        if case .object = shape { rootType = try type(shape, hint: name, rootName: name) }
        else { rootType = try type(shape, hint: name) }
        if rootType != name { declarations.append(swift ? "typealias \(name) = \(rootType)" : "export type \(name) = \(rootType);") }
        var output = swift ? "import Foundation\n\n// Inferred from all samples. Numbers use Decimal; validate your schema and numeric range.\n\n" : "// Inferred from all samples. Missing/null fields are optional.\n// JavaScript numbers cannot represent every JSON integer exactly.\n\n"
        output += declarations.joined(separator: "\n\n")
        if swift && needsJSONValue { output += "\n\n" + jsonValueHelper }
        return output
    }
    private static func isNull(_ value: Shape) -> Bool { if case .null = value { return true }; return false }
    private static func optional(_ value: Shape) -> Shape { switch value { case .null, .nullable: return value; default: return .nullable(value) } }
    private static func merge(_ a: Shape, _ b: Shape) -> Shape {
        switch (a, b) {
        case (.null, .null): return .null
        case (.null, _): return optional(b)
        case (_, .null): return optional(a)
        case (.nullable(let a), .nullable(let b)): return optional(merge(a, b))
        case (.nullable(let a), _): return optional(merge(a, b))
        case (_, .nullable(let b)): return optional(merge(a, b))
        case (.bool, .bool): return .bool
        case (.number, .number): return .number
        case (.string, .string): return .string
        case (.array(let a), .array(let b)): return .array(merge(a, b))
        case (.object(let a), .object(let b)):
            var fields: [String: Shape] = [:]
            for key in Set(a.keys).union(b.keys) {
                if let left = a[key], let right = b[key] { fields[key] = merge(left, right) }
                else { fields[key] = optional(a[key] ?? b[key]!) }
            }
            return .object(fields)
        default: return .any
        }
    }
    private static let jsonValueHelper = """
    indirect enum JSONValue: Codable {
        case null, bool(Bool), number(Decimal), string(String)
        case array([JSONValue]), object([String: JSONValue])
        init(from decoder: Decoder) throws {
            let c = try decoder.singleValueContainer()
            if c.decodeNil() { self = .null }
            else if let v = try? c.decode(Bool.self) { self = .bool(v) }
            else if let v = try? c.decode(Decimal.self) { self = .number(v) }
            else if let v = try? c.decode(String.self) { self = .string(v) }
            else if let v = try? c.decode([JSONValue].self) { self = .array(v) }
            else { self = .object(try c.decode([String: JSONValue].self)) }
        }
        func encode(to encoder: Encoder) throws {
            var c = encoder.singleValueContainer()
            switch self {
            case .null: try c.encodeNil()
            case .bool(let v): try c.encode(v)
            case .number(let v): try c.encode(v)
            case .string(let v): try c.encode(v)
            case .array(let v): try c.encode(v)
            case .object(let v): try c.encode(v)
            }
        }
    }
    """
}
