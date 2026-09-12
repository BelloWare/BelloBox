import Foundation

/// A typed tree avoids ambiguous $date/$data keys and loss of plist scalar types.
enum PlistTool {
    static let doctype = "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">"
    static func typed(_ type: String, _ value: DeveloperJSON) -> DeveloperJSON { .object(["type": .string(type), "value": value]) }
    static func convert(_ input: String, reverse: Bool) throws -> WorkbenchResult {
        if !reverse {
            let clean = input.replacingOccurrences(of: doctype, with: "")
            guard !clean.uppercased().contains("<!DOCTYPE"), !clean.uppercased().contains("<!ENTITY") else { throw UtilityError("Custom DTDs and entities are not accepted. The standard plist declaration is handled locally.") }
            let delegate = Reader(), parser = XMLParser(data: Data(clean.utf8))
            parser.shouldResolveExternalEntities = false; parser.delegate = delegate
            guard parser.parse(), let result = delegate.result else { throw delegate.failure ?? UtilityError("Invalid XML property list near line \(parser.lineNumber).") }
            return .init(text: result.formatted(), status: "Typed JSON · dictionaries, dates, data, and numeric types preserved")
        }
        let tree = try DeveloperJSON.parse(input)
        var budget = 50_000
        func xml(_ node: DeveloperJSON, depth: Int) throws -> String {
            try DataWorkshop.spend(&budget)
            guard depth < 64, case .object(let fields) = node, Set(fields.keys) == ["type", "value"], case .string(let type) = fields["type"], let value = fields["value"] else { throw UtilityError("Use typed JSON from this tool: each node has type and value (maximum 64 levels).") }
            func wrap(_ text: String) -> String { "<\(type)>\(text)</\(type)>" }
            switch (type, value) {
            case ("dict", .object(let values)):
                for key in values.keys { try validateCharacters(key) }
                return wrap(try values.keys.sorted().map { "<key>\(escape($0))</key>" + (try xml(values[$0]!, depth: depth + 1)) }.joined())
            case ("array", .array(let values)): return wrap(try values.map { try xml($0, depth: depth + 1) }.joined())
            case ("string", .string(let text)):
                try validateCharacters(text)
                return wrap(escape(text))
            case ("bool", .bool(let flag)): return flag ? "<true/>" : "<false/>"
            case ("integer", .number(let number)):
                guard Int64(number) != nil || UInt64(number) != nil else { throw UtilityError("Plist integers must fit signed or unsigned 64 bits.") }
                return wrap(number)
            case ("real", .number(let number)):
                guard let value = Double(number), value.isFinite else { throw UtilityError("Use a finite 64-bit floating-point plist real.") }
                return wrap(String(value))
            case ("date", .string(let date)):
                guard validDate(date) else { throw UtilityError("Plist dates use YYYY-MM-DDTHH:mm:ssZ in UTC.") }
                return wrap(date)
            case ("data", .string(let base64)):
                guard let data = Data(base64Encoded: base64) else { throw UtilityError("Plist data must be valid Base64.") }
                return wrap(data.base64EncodedString())
            default: throw UtilityError("Unsupported plist type or mismatched value: \(type.prefix(40)).")
            }
        }
        let body = try xml(tree, depth: 0)
        return .init(text: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\(doctype)\n<plist version=\"1.0\">\n\(body)\n</plist>", status: "XML plist · typed JSON converted locally")
    }
    static func escape(_ text: String) -> String {
        MarkdownTool.escapeHTML(text).replacingOccurrences(of: "\r", with: "&#13;")
    }
    static func validateCharacters(_ text: String) throws {
        guard text.unicodeScalars.allSatisfy({ [9, 10, 13].contains($0.value) || (0x20...0xD7FF).contains($0.value) || (0xE000...0xFFFD).contains($0.value) || (0x10000...0x10FFFF).contains($0.value) }) else { throw UtilityError("XML plist strings and keys cannot contain XML 1.0 control characters.") }
    }
    static func validDate(_ text: String) -> Bool {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX"); formatter.calendar = Calendar(identifier: .gregorian)
        formatter.timeZone = TimeZone(secondsFromGMT: 0); formatter.dateFormat = "yyyy-MM-dd'T'HH:mm:ss'Z'"; formatter.isLenient = false
        guard let date = formatter.date(from: text), formatter.string(from: date) == text else { return false }
        return (1...9999).contains(formatter.calendar.component(.year, from: date))
    }
    private final class Reader: NSObject, XMLParserDelegate {
        struct Node { let name: String; var pieces: [String] = []; var children: [(String, DeveloperJSON)] = [] }
        var stack: [Node] = [], result: DeveloperJSON?, failure: UtilityError?
        var nodes = 0
        func fail(_ parser: XMLParser, _ message: String) { failure = UtilityError(message); parser.abortParsing() }
        func parser(_ parser: XMLParser, didStartElement name: String, namespaceURI: String?, qualifiedName: String?, attributes: [String: String]) {
            nodes += 1
            guard !Task.isCancelled, stack.count < 64, nodes <= 20_000 else { fail(parser, "Plist exceeds 64 levels or 20,000 values, or was cancelled."); return }
            guard ["plist", "dict", "array", "key", "string", "integer", "real", "date", "data", "true", "false"].contains(name),
                  attributes.isEmpty || name == "plist" && attributes == ["version": "1.0"] else { fail(parser, "Unsupported plist element or attributes: \(name.prefix(40))."); return }
            guard !stack.isEmpty || name == "plist" else { fail(parser, "XML needs a plist root element."); return }
            stack.append(Node(name: name))
        }
        func parser(_ parser: XMLParser, foundCharacters string: String) {
            guard !stack.isEmpty else { return }
            if Task.isCancelled { fail(parser, "Cancelled."); return }
            stack[stack.count - 1].pieces.append(string)
        }
        func parser(_ parser: XMLParser, foundCDATA data: Data) { self.parser(parser, foundCharacters: String(decoding: data, as: UTF8.self)) }
        func parser(_ parser: XMLParser, resolveExternalEntityName name: String, systemID: String?) -> Data? { nil }
        func parser(_ parser: XMLParser, didEndElement name: String, namespaceURI: String?, qualifiedName: String?) {
            guard let node = stack.popLast(), failure == nil else { return }
            let text = node.pieces.joined(), trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            do {
                let value: DeveloperJSON
                if ["plist", "dict", "array"].contains(name) {
                    guard trimmed.isEmpty else { throw UtilityError("Text is not allowed directly inside a plist container.") }
                    if name == "plist" {
                        guard stack.isEmpty, node.children.count == 1, node.children[0].0 != "key" else { throw UtilityError("A plist root needs exactly one value.") }
                        result = node.children[0].1; return
                    }
                    if name == "array" {
                        guard node.children.allSatisfy({ $0.0 != "key" }) else { throw UtilityError("A key must be inside a dictionary.") }
                        value = typed("array", .array(node.children.map(\.1)))
                    } else {
                        guard node.children.count % 2 == 0 else { throw UtilityError("Each dictionary key needs a value.") }
                        var object: [String: DeveloperJSON] = [:]
                        for i in stride(from: 0, to: node.children.count, by: 2) {
                            guard node.children[i].0 == "key", case .string(let key) = node.children[i].1, node.children[i + 1].0 != "key", object[key] == nil else { throw UtilityError("Plist dictionary keys must be unique and followed by a value.") }
                            object[key] = node.children[i + 1].1
                        }
                        value = typed("dict", .object(object))
                    }
                } else {
                    guard node.children.isEmpty else { throw UtilityError("A plist scalar cannot contain elements.") }
                    switch name {
                    case "key": value = .string(text)
                    case "string": value = typed(name, .string(text))
                    case "true", "false":
                        guard trimmed.isEmpty else { throw UtilityError("Boolean elements must be empty.") }
                        value = typed("bool", .bool(name == "true"))
                    case "integer":
                        guard let integer = Int64(trimmed).map(String.init) ?? UInt64(trimmed).map(String.init) else { throw UtilityError("Plist integer exceeds 64 bits or is invalid.") }
                        value = typed(name, .number(integer))
                    case "real":
                        guard let real = Double(trimmed), real.isFinite else { throw UtilityError("Plist real must be finite.") }
                        value = typed(name, .number(String(real)))
                    case "date":
                        guard validDate(trimmed) else { throw UtilityError("Plist date must use YYYY-MM-DDTHH:mm:ssZ.") }
                        value = typed(name, .string(trimmed))
                    case "data":
                        let compact = trimmed.filter { !$0.isWhitespace }
                        guard let bytes = Data(base64Encoded: compact) else { throw UtilityError("Invalid Base64 plist data.") }
                        value = typed(name, .string(bytes.base64EncodedString()))
                    default: throw UtilityError("Invalid plist element.")
                    }
                }
                guard !stack.isEmpty else { throw UtilityError("Missing plist root.") }
                stack[stack.count - 1].children.append((name, value))
            } catch { fail(parser, error.localizedDescription) }
        }
    }
}
