import Foundation

enum JSONPointerTool {
    static func tokens(_ pointer: String) throws -> [String] {
        guard pointer.utf8.count <= 8_192 else { throw UtilityError("Keep JSON pointers within 8 KB.") }
        var value = pointer
        if value.hasPrefix("#") {
            guard let decoded = String(value.dropFirst()).removingPercentEncoding else { throw UtilityError("Invalid percent encoding in the pointer fragment.") }
            value = decoded
        }
        if value.isEmpty { return [] }
        guard value.hasPrefix("/") else { throw UtilityError("A JSON pointer starts with /, or is empty for the root. URI fragments start with #/.") }
        return try value.dropFirst().split(separator: "/", omittingEmptySubsequences: false).map { part in
            let chars = Array(part); var i = 0, decoded = ""
            while i < chars.count {
                if chars[i] == "~" {
                    guard i + 1 < chars.count, chars[i + 1] == "0" || chars[i + 1] == "1" else { throw UtilityError("Only ~0 (tilde) and ~1 (slash) are valid pointer escapes.") }
                    decoded.append(chars[i + 1] == "0" ? "~" : "/"); i += 2
                } else { decoded.append(chars[i]); i += 1 }
            }
            return decoded
        }
    }
    static func escape(_ key: String) -> String { key.replacingOccurrences(of: "~", with: "~0").replacingOccurrences(of: "/", with: "~1") }
    static func index(_ token: String) -> Int? {
        guard !token.isEmpty, token == "0" || !token.hasPrefix("0"), token.utf8.allSatisfy({ (48...57).contains($0) }) else { return nil }
        return Int(token)
    }
    static func resolve(_ value: DeveloperJSON, pointer: String) throws -> DeveloperJSON {
        var current = value
        for token in try tokens(pointer) {
            switch current {
            case .object(let fields):
                // RFC 6901 compares code points; do not normalize a lookup key.
                guard let key = fields.keys.first(where: { $0.unicodeScalars.elementsEqual(token.unicodeScalars) }), let next = fields[key] else { throw UtilityError("No property named ‘\(String(token.prefix(80)))’ at this path.") }
                current = next
            case .array(let values):
                guard let index = index(token), values.indices.contains(index) else { throw UtilityError("‘\(String(token.prefix(40)))’ is not an existing array index. Use 0-based digits without leading zeros.") }
                current = values[index]
            default: throw UtilityError("The pointer continues beyond a scalar value.")
            }
        }
        return current
    }
}

enum JSONPathEntries {
    static func flatten(_ value: DeveloperJSON) throws -> DeveloperJSON {
        var entries: [DeveloperJSON] = [], pathBytes = 0
        func walk(_ value: DeveloperJSON, path: String) throws {
            guard entries.count < 20_000 else { throw UtilityError("Flatten up to 20,000 JSON values at a time.") }
            try Task.checkCancellation()
            pathBytes += path.utf8.count
            guard path.utf8.count <= 8_192, pathBytes <= 2_000_000 else { throw UtilityError("Flattened paths exceed the 8 KB per-path or 2 MB total path limit. Use shorter keys or a smaller document.") }
            var entry: [String: DeveloperJSON] = ["path": .string(path)]
            switch value {
            case .object: entry["type"] = .string("object")
            case .array: entry["type"] = .string("array")
            default: entry["type"] = .string("value"); entry["value"] = value
            }
            entries.append(.object(entry))
            if case .object(let object) = value { for key in object.keys.sorted() { try walk(object[key]!, path: path + "/" + JSONPointerTool.escape(key)) } }
            if case .array(let array) = value { for (index, child) in array.enumerated() { try walk(child, path: path + "/\(index)") } }
        }
        try walk(value, path: ""); return .array(entries)
    }
    private final class Node { var entry: [String: DeveloperJSON]?; var children: [String: Node] = [:] }
    static func unflatten(_ value: DeveloperJSON) throws -> DeveloperJSON {
        guard case .array(let entries) = value, !entries.isEmpty, entries.count <= 20_000 else { throw UtilityError("Use the typed entry array produced by Flatten (up to 20,000 entries).") }
        let root = Node()
        var nodeCount = 1
        for entry in entries {
            try Task.checkCancellation()
            guard case .object(let fields) = entry, case .string(let path) = fields["path"], case .string(let type) = fields["type"], ["object", "array", "value"].contains(type), Set(fields.keys).isSubset(of: ["path", "type", "value"]) else { throw UtilityError("Each entry needs path, type (object/array/value), and a value only for scalars.") }
            guard !path.hasPrefix("#") else { throw UtilityError("Typed entries use plain JSON pointers, not URI fragments.") }
            let tokens = try JSONPointerTool.tokens(path)
            guard tokens.count <= 64 else { throw UtilityError("JSON is nested too deeply (maximum 64).") }
            var node = root
            for token in tokens {
                if node.children[token] == nil {
                    nodeCount += 1
                    guard nodeCount <= 20_000 else { throw UtilityError("The path tree exceeds 20,000 values.") }
                    node.children[token] = Node()
                }
                node = node.children[token]!
            }
            guard node.entry == nil else { throw UtilityError("Two entries use the same path.") }
            node.entry = fields
        }
        func build(_ node: Node) throws -> DeveloperJSON {
            guard let entry = node.entry, case .string(let type) = entry["type"] else { throw UtilityError("Every parent, including the root, needs its own typed entry.") }
            if type == "value" {
                guard node.children.isEmpty, let value = entry["value"] else { throw UtilityError("Scalar entries need a value and cannot have children.") }
                switch value { case .array, .object: throw UtilityError("Use array/object container entries for nested values."); default: return value }
            }
            guard entry["value"] == nil else { throw UtilityError("Container entries do not have a value field.") }
            if type == "object" { return .object(try node.children.mapValues(build)) }
            var result: [DeveloperJSON] = []
            for index in 0..<node.children.count {
                guard let child = node.children[String(index)] else { throw UtilityError("Array paths must be consecutive indices starting at 0.") }
                result.append(try build(child))
            }
            return .array(result)
        }
        return try build(root)
    }
}

enum StructuredUtility {
    static func run(_ kind: AdditionalUtilityKind, input: String, options: [String: String]) throws -> WorkbenchResult {
        if kind == .xmlJSON { return WorkbenchResult(text: try OrderedXML.convert(input).formatted(), status: "Ordered XML tree · attributes and mixed content preserved") }
        let json = try DeveloperJSON.parse(input)
        switch kind {
        case .jsonPointer: return WorkbenchResult(text: try JSONPointerTool.resolve(json, pointer: options["pointer"] ?? "").formatted(), status: "Resolved JSON pointer · numbers preserved")
        case .jsonFlatten:
            let flatten = options["mode"] == "Flatten"
            return WorkbenchResult(text: try (flatten ? JSONPathEntries.flatten(json) : JSONPathEntries.unflatten(json)).formatted(), status: flatten ? "Typed paths · empty containers and array types preserved" : "Reconstructed JSON")
        case .jsonCode: return WorkbenchResult(text: try JSONCodeTool.generate(json, language: options["language"]!, name: options["name"]!), status: "Types inferred from every sample · review before integrating")
        case .sqlInsert: return WorkbenchResult(text: try sql(json, table: options["table"]!, dialect: options["dialect"]!), status: "SQL generated locally · never executed")
        default: throw UtilityError("Choose a structured-data tool.")
        }
    }
    static func sql(_ input: DeveloperJSON, table: String, dialect: String) throws -> String {
        guard !table.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, table.utf8.count <= 256, !table.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else { throw UtilityError("Enter a table name of 1–256 bytes without control characters. It is quoted as one identifier.") }
        let rows: [DeveloperJSON]
        if case .array(let values) = input { rows = values } else { rows = [input] }
        guard !rows.isEmpty, rows.count <= 5_000 else { throw UtilityError("Provide 1–5,000 JSON objects.") }
        var records: [[String: DeveloperJSON]] = [], columns = Set<String>()
        for row in rows {
            guard case .object(let record) = row, !record.isEmpty else { throw UtilityError("Each record must be a nonempty JSON object.") }
            records.append(record); columns.formUnion(record.keys)
        }
        guard columns.count <= 500 else { throw UtilityError("Use at most 500 columns.") }
        func identifier(_ s: String) throws -> String {
            guard !s.isEmpty, !s.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else { throw UtilityError("Column names must be nonempty and cannot contain control characters.") }
            return "\"" + s.replacingOccurrences(of: "\"", with: "\"\"") + "\""
        }
        func literal(_ value: DeveloperJSON) throws -> String {
            switch value {
            case .null: return "NULL"
            case .bool(let flag): return dialect == "SQLite" ? (flag ? "1" : "0") : (flag ? "TRUE" : "FALSE")
            case .number(let n): return n
            default:
                let raw = value.scalarText
                guard !raw.contains("\0") else { throw UtilityError("SQL text literals cannot contain a NUL character.") }
                let escaped = raw.replacingOccurrences(of: "'", with: "''")
                // PostgreSQL E strings are independent of standard_conforming_strings.
                return dialect == "PostgreSQL" ? "E'" + escaped.replacingOccurrences(of: "\\", with: "\\\\") + "'" : "'" + escaped + "'"
            }
        }
        let sorted = columns.sorted(), header = "INSERT INTO \(try identifier(table)) (\(try sorted.map(identifier).joined(separator: ", "))) VALUES\n"
        var values: [String] = [], outputBytes = header.utf8.count
        for record in records {
            try Task.checkCancellation()
            let line = "(" + (try sorted.map { try literal(record[$0] ?? .null) }).joined(separator: ", ") + ")"
            outputBytes += line.utf8.count + 2
            guard outputBytes <= 4_096_000 else { throw UtilityError("The SQL result exceeds 4 MB. Use fewer records or columns.") }
            values.append(line)
        }
        return header + values.joined(separator: ",\n") + ";"
    }
}

/// XML is represented as an ordered tree, so repeated children, namespaces,
/// attributes and text on either side of a child never silently disappear.
private final class OrderedXML: NSObject, XMLParserDelegate {
    private struct Element { let name: String, attributes: [String: String]; var children: [DeveloperJSON] = [] }
    private var stack: [Element] = [], root: DeveloperJSON?, failure: String?, count = 0
    static func convert(_ input: String) throws -> DeveloperJSON {
        guard !input.localizedCaseInsensitiveContains("<!DOCTYPE"), !input.localizedCaseInsensitiveContains("<!ENTITY") else { throw UtilityError("DTD and entity declarations are not supported. Paste self-contained XML.") }
        let delegate = OrderedXML(), parser = XMLParser(data: Data(input.utf8))
        parser.shouldResolveExternalEntities = false
        parser.externalEntityResolvingPolicy = .never
        parser.delegate = delegate
        guard parser.parse(), let result = delegate.root else { throw UtilityError(delegate.failure ?? "Invalid XML at line \(parser.lineNumber), column \(parser.columnNumber).") }
        return result
    }
    func parser(_ parser: XMLParser, didStartElement elementName: String, namespaceURI: String?, qualifiedName qName: String?, attributes: [String: String]) {
        count += 1
        guard stack.count < 64, count <= 20_000, !Task.isCancelled else { failure = "XML is limited to 64 levels and 20,000 elements."; parser.abortParsing(); return }
        stack.append(Element(name: qName ?? elementName, attributes: attributes))
    }
    func parser(_ parser: XMLParser, foundCharacters string: String) {
        guard !stack.isEmpty else { return }
        let i = stack.count - 1
        if case .string(let previous) = stack[i].children.last { stack[i].children[stack[i].children.count - 1] = .string(previous + string) }
        else { stack[i].children.append(.string(string)) }
    }
    func parser(_ parser: XMLParser, foundCDATA CDATABlock: Data) { self.parser(parser, foundCharacters: String(decoding: CDATABlock, as: UTF8.self)) }
    func parser(_ parser: XMLParser, didEndElement elementName: String, namespaceURI: String?, qualifiedName qName: String?) {
        guard let element = stack.popLast() else { return }
        let value = DeveloperJSON.object(["name": .string(element.name), "attributes": .object(element.attributes.mapValues(DeveloperJSON.string)), "children": .array(element.children)])
        if stack.isEmpty { root = value } else { stack[stack.count - 1].children.append(value) }
    }
    func parser(_ parser: XMLParser, resolveExternalEntityName name: String, systemID: String?) -> Data? { nil }
}
