import Foundation
import Yams

enum DataFormat: String, CaseIterable { case json = "JSON", yaml = "YAML", csv = "CSV" }
struct DataTable {
    let columns: [String]
    let rows: [[String]]
    let totalRows: Int
}
enum DataConversion {
    static func detectFormat(_ text: String) -> DataFormat {
        let input = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if input.hasPrefix("{") || input.hasPrefix("[") { return .json }
        let firstLine = input.prefix(while: { !$0.isNewline })
        if firstLine.contains(",") || firstLine.contains("\t") { return .csv }
        return input.contains(":") ? .yaml : .csv
    }
    static func convert(_ text: String, from: DataFormat, to: DataFormat, delimiter: Character = ",", inferTypes: Bool = false) throws -> (String, DataTable?) {
        try UtilityLimits.check(text)
        let value: DeveloperJSON
        switch from {
        case .json: value = try DeveloperJSON.parse(text)
        case .csv: value = try CSVCodec.decode(text, delimiter: delimiter, inferTypes: inferTypes)
        case .yaml: value = try decodeYAML(text)
        }
        let result: String
        switch to {
        case .json: result = value.formatted()
        case .yaml: result = try Yams.serialize(node: yamlNode(value))
        case .csv: result = try CSVCodec.encode(value, delimiter: delimiter)
        }
        return (result, table(value))
    }

    static func table(_ value: DeveloperJSON) -> DataTable? {
        guard case .array(let values) = value, !values.isEmpty,
              values.allSatisfy({ if case .object = $0 { return true }; return false }) else { return nil }
        let objects = values.map { value -> [String: DeveloperJSON] in if case .object(let obj) = value { return obj }; return [:] }
        let columns = Set(objects.flatMap { $0.keys }).sorted()
        return DataTable(columns: Array(columns.prefix(30)), rows: objects.prefix(30).map { row in columns.prefix(30).map { row[$0]?.scalarText ?? "" } }, totalRows: values.count)
    }

    private static func yamlNode(_ value: DeveloperJSON) -> Node {
        switch value {
        case .null: return Node("null", Tag(.null))
        case .bool(let value): return Node(value ? "true" : "false", Tag(.bool))
        case .number(let value): return Node(value, Tag(value.contains(".") || value.lowercased().contains("e") ? .float : .int))
        case .string(let value): return Node(value, Tag(.str), .doubleQuoted)
        case .array(let values): return Node(values.map(yamlNode))
        case .object(let values): return Node(values.keys.sorted().map { (Node($0, Tag(.str), .doubleQuoted), yamlNode(values[$0]!)) })
        }
    }

    private static func decodeYAML(_ text: String) throws -> DeveloperJSON {
        // YAML 1.2-style scalar resolution: words such as "yes" and "on" stay strings.
        let resolver = try Resolver.basic
            .appending(.bool, "^(?:true|True|TRUE|false|False|FALSE)$")
            .appending(.null, "^(?:~|null|Null|NULL|)$")
            .appending(.int, "^[-+]?(?:[0-9]+|0x[0-9a-fA-F]+|0o[0-7]+|0b[01]+)$")
            .appending(.float, "^[-+]?(?:[0-9]+\\.[0-9]*|\\.[0-9]+|[0-9]+)(?:[eE][-+]?[0-9]+)?$")
        guard let root = try Yams.compose(yaml: text, resolver) else { throw UtilityError("Enter a YAML document.") }
        var anchors: [String: Node] = [:], visited = 0
        func collect(_ node: Node, depth: Int) throws {
            guard depth < 64 else { throw UtilityError("YAML nesting exceeds 64 levels.") }
            if let anchor = node.anchor { anchors[anchor.rawValue] = node }
            switch node {
            case .mapping(let pairs): for pair in pairs { try collect(pair.value, depth: depth + 1) }
            case .sequence(let values): for value in values { try collect(value, depth: depth + 1) }
            default: break
            }
        }
        try collect(root, depth: 0)
        func convert(_ node: Node, depth: Int) throws -> DeveloperJSON {
            visited += 1
            guard depth < 64, visited <= 20_000 else { throw UtilityError("YAML exceeds the nesting or expanded-node limit. Check for recursive aliases.") }
            switch node {
            case .alias(let alias):
                guard let target = anchors[alias.anchor.rawValue] else { throw UtilityError("Unknown YAML alias: \(alias.anchor.rawValue)") }
                return try convert(target, depth: depth + 1)
            case .sequence(let values): return .array(try values.map { try convert($0, depth: depth + 1) })
            case .mapping(let pairs):
                var object: [String: DeveloperJSON] = [:]
                for pair in pairs {
                    guard case .scalar(let key) = pair.key, pair.key.tag == Tag(.str) else { throw UtilityError("JSON needs string property names; this YAML contains a non-string key.") }
                    guard key.string != "<<" else { throw UtilityError("Expand YAML merge keys (<<) before converting to JSON.") }
                    guard object[key.string] == nil else { throw UtilityError("Duplicate YAML key: \(key.string.prefix(60))") }
                    object[key.string] = try convert(pair.value, depth: depth + 1)
                }
                return .object(object)
            case .scalar(let scalar):
                let tag = node.tag
                if tag == Tag(.str) { return .string(scalar.string) }
                if tag == Tag(.null) { return .null }
                if tag == Tag(.bool), let value = node.bool { return .bool(value) }
                if tag == Tag(.int) || tag == Tag(.float) {
                    var number = scalar.string
                    if number.hasPrefix("+") { number.removeFirst() }
                    if number.hasPrefix(".") { number = "0" + number }
                    if number.hasPrefix("-.") { number = "-0" + number.dropFirst() }
                    if number.hasSuffix(".") { number += "0" }
                    if let json = try? DeveloperJSON.parse(number), case .number = json { return json }
                    if let value = node.int { return .number(String(value)) }
                    throw UtilityError("This YAML number cannot be represented as JSON: \(scalar.string.prefix(60))")
                }
                throw UtilityError("Unsupported YAML tag: \(tag.rawValue)")
            }
        }
        return try convert(root, depth: 0)
    }
}

enum CSVCodec {
    static func rows(_ text: String, delimiter: Character = ",") throws -> [[String]] {
        let chars = Array(text), count = chars.count
        var rows: [[String]] = [], row: [String] = [], field = "", i = 0, quoted = false, closed = false
        func finishField() { row.append(field); field = ""; closed = false }
        func finishRow() { finishField(); rows.append(row); row = [] }
        while i < count {
            let char = chars[i]
            if quoted {
                if char == "\"" {
                    if i + 1 < count && chars[i + 1] == "\"" { field.append("\""); i += 1 }
                    else { quoted = false; closed = true }
                } else { field.append(char) }
            } else if char == delimiter { finishField() }
            else if char == "\n" || char == "\r\n" || char == "\r" {
                finishRow()
                if char == "\r" && i + 1 < count && chars[i + 1] == "\n" { i += 1 }
            } else if char == "\"" && field.isEmpty && !closed { quoted = true }
            else {
                guard !closed && char != "\"" else { throw UtilityError("Unexpected character after a quoted CSV field near character \(i + 1).") }
                field.append(char)
            }
            guard rows.count <= 5_000, row.count <= 200 else { throw UtilityError("CSV supports up to 5,000 rows and 200 columns.") }
            i += 1
        }
        guard !quoted else { throw UtilityError("CSV has an unclosed quoted field.") }
        if !field.isEmpty || !row.isEmpty || closed { finishRow() }
        return rows
    }
    static func decode(_ text: String, delimiter: Character, inferTypes: Bool) throws -> DeveloperJSON {
        let rows = try rows(text, delimiter: delimiter)
        guard let headers = rows.first, !headers.isEmpty, headers.allSatisfy({ !$0.isEmpty }), Set(headers).count == headers.count else {
            throw UtilityError("The first CSV row must contain unique, non-empty column names.")
        }
        return .array(try rows.dropFirst().enumerated().map { index, row in
            guard row.count == headers.count else { throw UtilityError("CSV row \(index + 2) has \(row.count) fields; expected \(headers.count).") }
            return .object(Dictionary(uniqueKeysWithValues: zip(headers, row).map { key, value in
                if inferTypes, let parsed = try? DeveloperJSON.parse(value), !value.isEmpty {
                    switch parsed {
                    case .number, .bool, .null: return (key, parsed)
                    default: break
                    }
                }
                return (key, .string(value))
            }))
        })
    }
    static func escape(_ value: String, delimiter: Character) -> String {
        if value.contains(delimiter) || value.contains("\"") || value.contains("\n") || value.contains("\r") {
            return "\"" + value.replacingOccurrences(of: "\"", with: "\"\"") + "\""
        }
        return value
    }
    static func encode(_ value: DeveloperJSON, delimiter: Character) throws -> String {
        guard case .array(let values) = value, !values.isEmpty else { throw UtilityError("CSV output requires a non-empty JSON array of objects.") }
        let objects = try values.map { value -> [String: DeveloperJSON] in
            guard case .object(let object) = value else { throw UtilityError("Every CSV row must be a JSON object.") }
            return object
        }
        let columns = Set(objects.flatMap { $0.keys }).sorted()
        guard !columns.isEmpty, columns.count <= 200 else { throw UtilityError("CSV output needs 1–200 columns.") }
        let rows = [columns] + objects.map { object in columns.map { key in
            guard let value = object[key], value != .null else { return "" }
            return value.scalarText
        } }
        return rows.map { $0.map { escape($0, delimiter: delimiter) }.joined(separator: String(delimiter)) }.joined(separator: "\r\n")
    }
}
