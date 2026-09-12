import Foundation

enum DataWorkshop {
    static func run(_ kind: AdditionalUtilityKind, input: String, second: String, options: [String: String]) throws -> WorkbenchResult {
        switch kind {
        case .jsonMerge:
            guard !second.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { throw UtilityError("Add a merge patch in the second editor, or load the example.") }
            let target = try DeveloperJSON.parse(input), patch = try DeveloperJSON.parse(second)
            var budget = 50_000
            let result = try merge(target, patch: patch, budget: &budget)
            return .init(text: result.formatted(), status: "Patch applied locally · null removes fields · arrays replace as a whole")
        case .jsonRedact:
            let keys = options["keys", default: ""].split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).lowercased() }
            guard !keys.isEmpty, keys.count <= 100, keys.allSatisfy({ !$0.isEmpty && $0.utf8.count <= 128 }) else { throw UtilityError("Enter 1–100 field names, separated by commas (up to 128 bytes each).") }
            let names = Set(keys)
            var count = 0, budget = 50_000
            func redact(_ value: DeveloperJSON) throws -> DeveloperJSON {
                try spend(&budget)
                switch value {
                case .object(let object):
                    return .object(try object.mapValuesWithKeys { key, value in
                        if names.contains(key.lowercased()) { count += 1; return .string("[REDACTED]") }
                        return try redact(value)
                    })
                case .array(let array): return .array(try array.map(redact))
                default: return value
                }
            }
            let result = try redact(DeveloperJSON.parse(input))
            return .init(text: result.formatted(), status: "\(count) fields replaced · names match without case · review before sharing")
        case .jsonLines:
            let result: String, count: Int
            if options["mode"] == "Array → Lines" {
                guard case .array(let values) = try DeveloperJSON.parse(input), values.count <= 5_000 else { throw UtilityError("Enter a JSON array with up to 5,000 values.") }
                count = values.count
                result = values.map { $0.formatted(pretty: false) }.joined(separator: "\n")
            } else {
                let lines = normalizedLines(input)
                guard lines.count <= 10_000 else { throw UtilityError("Use up to 10,000 lines and 5,000 records.") }
                var values: [DeveloperJSON] = [], blank = 0
                for (index, line) in lines.enumerated() {
                    try Task.checkCancellation()
                    if line.trimmingCharacters(in: .whitespaces).isEmpty { blank += 1; continue }
                    do { values.append(try DeveloperJSON.parse(line)) }
                    catch { throw UtilityError("Record on line \(index + 1): \(error.localizedDescription)") }
                    guard values.count <= 5_000 else { throw UtilityError("Use up to 5,000 JSON records.") }
                }
                return .init(text: DeveloperJSON.array(values).formatted(), status: "\(values.count) records · \(blank) blank lines ignored · numbers preserved")
            }
            return .init(text: result, status: "\(count) JSON records · one value per line")
        case .csvExplore: return try exploreCSV(input, options: options)
        case .envFile: return try environment(input, reverse: options["mode"] == "JSON → Env")
        case .plist: return try PlistTool.convert(input, reverse: options["mode"] == "JSON → Plist")
        case .textTable: return try textTable(input, options: options)
        default: throw UtilityError("Choose a data tool.")
        }
    }
    static func normalizedLines(_ text: String) -> [String] {
        text.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n").components(separatedBy: "\n")
    }
    static func spend(_ budget: inout Int) throws {
        budget -= 1
        guard budget >= 0 else { throw UtilityError("This operation exceeds its 50,000-value work limit. Use a smaller input.") }
        try Task.checkCancellation()
    }
    static func merge(_ target: DeveloperJSON, patch: DeveloperJSON, budget: inout Int) throws -> DeveloperJSON {
        try spend(&budget)
        guard case .object(let changes) = patch else { return patch }
        var object: [String: DeveloperJSON] = [:]
        if case .object(let current) = target { object = current }
        for (key, value) in changes {
            if value == .null { object.removeValue(forKey: key) }
            else { object[key] = try merge(object[key] ?? .null, patch: value, budget: &budget) }
        }
        return .object(object)
    }
    static func csv(_ input: String, delimiter: Character = ",") throws -> (headers: [String], rows: [[String]]) {
        let all = try CSVCodec.rows(input, delimiter: delimiter)
        guard let headers = all.first, !headers.isEmpty, headers.count <= 200,
              headers.allSatisfy({ !$0.isEmpty }), Set(headers).count == headers.count else { throw UtilityError("The first row needs 1–200 unique, non-empty column names.") }
        guard all.count <= 5_001, all.count * headers.count <= 50_000 else { throw UtilityError("Use up to 5,000 rows and 50,000 cells.") }
        for (i, row) in all.dropFirst().enumerated() where row.count != headers.count { throw UtilityError("Row \(i + 2) has \(row.count) cells; expected \(headers.count).") }
        return (headers, Array(all.dropFirst()))
    }
    static func exploreCSV(_ input: String, options: [String: String]) throws -> WorkbenchResult {
        let delimiter: Character = options["delimiter"] == "Tab" ? "\t" : options["delimiter"] == "Semicolon" ? ";" : ","
        let source = try csv(input, delimiter: delimiter)
        let requested = options["columns", default: ""].trimmingCharacters(in: .whitespacesAndNewlines)
        // Quoted CSV column names can themselves contain commas.
        let requestedRows = requested.isEmpty ? [source.headers] : try CSVCodec.rows(requested)
        guard requestedRows.count == 1 else { throw UtilityError("Enter column names on one line, quoting names that contain commas.") }
        let columns = requestedRows[0]
        guard !columns.isEmpty, Set(columns).count == columns.count else { throw UtilityError("Choose unique columns, or leave Columns empty for all. Quote names containing commas.") }
        let indices = try columns.map { name -> Int in
            guard let index = source.headers.firstIndex(of: name) else { throw UtilityError("Unknown column: \(name.prefix(60)). Names are exact; quote names containing commas.") }
            return index
        }
        let filter = options["filter", default: ""], unique = options["rows"] == "Unique"
        var seen = Set<Data>(), rows: [[String]] = []
        for row in source.rows {
            try Task.checkCancellation()
            guard filter.isEmpty || row.contains(where: { $0.range(of: filter, options: [.caseInsensitive, .literal]) != nil }) else { continue }
            let projected = indices.map { row[$0] }
            if !unique || seen.insert(Data(DeveloperJSON.array(projected.map(DeveloperJSON.string)).formatted(pretty: false).utf8)).inserted { rows.append(projected) }
        }
        let output = ([columns] + rows).map { $0.map { CSVCodec.escape($0, delimiter: delimiter) }.joined(separator: String(delimiter)) }.joined(separator: "\r\n")
        let preview = DataTable(columns: Array(columns.prefix(30)), rows: rows.prefix(100).map { Array($0.prefix(30)) }, totalRows: rows.count, totalColumns: columns.count)
        return .init(text: output, status: "\(rows.count) of \(source.rows.count) rows · \(columns.count) columns · Copy includes all matches", visual: .table(preview))
    }
    static func environment(_ input: String, reverse: Bool) throws -> WorkbenchResult {
        if reverse {
            guard case .object(let object) = try DeveloperJSON.parse(input), object.count <= 5_000 else { throw UtilityError("Enter a JSON object of up to 5,000 string values.") }
            let output = try object.keys.sorted().map { key -> String in
                guard envKey(key), case .string(let value) = object[key]! else { throw UtilityError("Environment keys must be identifiers and values must be strings.") }
                return key + "=" + DeveloperJSON.quoted(value)
            }.joined(separator: "\n")
            return .init(text: output, status: "\(object.count) assignments · quoted values · no variable expansion")
        }
        let lines = normalizedLines(input)
        guard lines.count <= 5_000 else { throw UtilityError("Use up to 5,000 environment lines.") }
        var object: [String: DeveloperJSON] = [:]
        for (index, raw) in lines.enumerated() {
            try Task.checkCancellation()
            var line = raw.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") { continue }
            if line.hasPrefix("export ") { line = String(line.dropFirst(7)).trimmingCharacters(in: .whitespaces) }
            func error(_ detail: String) -> UtilityError { UtilityError("Line \(index + 1): \(detail)") }
            guard let equals = line.firstIndex(of: "=") else { throw error("Use KEY=value.") }
            let key = String(line[..<equals]).trimmingCharacters(in: .whitespaces)
            guard envKey(key) else { throw error("Use letters, digits, and underscores for keys; do not start with a digit.") }
            guard object[key] == nil else { throw error("Duplicate environment key: \(key.prefix(60)).") }
            let rawValue = String(line[line.index(after: equals)...]).trimmingCharacters(in: .whitespaces)
            var value: String
            if let quote = rawValue.first, quote == "\"" || quote == "'" {
                var escaped = false, end: String.Index?
                for i in rawValue.indices.dropFirst() {
                    let c = rawValue[i]
                    if escaped { escaped = false; continue }
                    if c == "\\" && quote == "\"" { escaped = true; continue }
                    if c == quote { end = i; break }
                }
                guard let end else { throw error("Close the quoted value on the same line; use \\n for a newline.") }
                let trailing = rawValue[rawValue.index(after: end)...].trimmingCharacters(in: .whitespaces)
                guard trailing.isEmpty || trailing.hasPrefix("#") else { throw error("Only a comment may follow a quoted value.") }
                if quote == "\"" {
                    guard case .string(let decoded) = try DeveloperJSON.parse(String(rawValue[...end])) else { throw error("Invalid quoted string.") }
                    value = decoded
                } else { value = String(rawValue[rawValue.index(after: rawValue.startIndex)..<end]) }
            } else {
                value = rawValue
                if let comment = value.indices.first(where: { value[$0] == "#" && ($0 == value.startIndex || value[value.index(before: $0)].isWhitespace) }) { value = String(value[..<comment]) }
                value = value.trimmingCharacters(in: .whitespaces)
            }
            object[key] = .string(value)
        }
        return .init(text: DeveloperJSON.object(object).formatted(), status: "\(object.count) variables · values stay literal · comments omitted")
    }
    static func envKey(_ key: String) -> Bool {
        let bytes = Array(key.utf8)
        func letter(_ c: UInt8) -> Bool { c == 95 || (65...90).contains(c) || (97...122).contains(c) }
        return !bytes.isEmpty && bytes.count <= 256 && letter(bytes[0]) && bytes.allSatisfy { letter($0) || (48...57).contains($0) }
    }
    static func textTable(_ input: String, options: [String: String]) throws -> WorkbenchResult {
        let data = try csv(input)
        guard data.headers.count <= 30, data.rows.count <= 1_000 else { throw UtilityError("Text tables support up to 30 columns and 1,000 rows.") }
        let rows = [data.headers] + data.rows, format = options["format"] ?? "Markdown", alignment = options["alignment"] ?? "Left"
        let output: String
        if format == "HTML" {
            func row(_ row: [String], tag: String) -> String {
                "  <tr>" + row.map { "<\(tag) style=\"text-align: \(alignment.lowercased())\">\(MarkdownTool.escapeHTML($0).replacingOccurrences(of: "\n", with: "<br>"))</\(tag)>" }.joined() + "</tr>"
            }
            output = "<table>\n<thead>\n" + row(data.headers, tag: "th") + "\n</thead>\n<tbody>\n" + data.rows.map { row($0, tag: "td") }.joined(separator: "\n") + "\n</tbody>\n</table>"
        } else {
            let sanitized = rows.map { $0.map { cell -> String in
                let cell = cell.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n")
                if format == "Markdown" { return MarkdownTool.escapeHTML(cell).replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "|", with: "\\|").replacingOccurrences(of: "`", with: "\\`").replacingOccurrences(of: "*", with: "\\*").replacingOccurrences(of: "_", with: "\\_").replacingOccurrences(of: "[", with: "\\[").replacingOccurrences(of: "]", with: "\\]").replacingOccurrences(of: "\n", with: "<br>") }
                return cell.replacingOccurrences(of: "\n", with: " ↵ ").replacingOccurrences(of: "\t", with: " ⇥ ")
            } }
            let widths = data.headers.indices.map { column in max(3, sanitized.map { $0[column].count }.max() ?? 0) }
            guard widths.reduce(0, +) <= 2_000 else { throw UtilityError("This table would be wider than 2,000 characters. Choose fewer columns or shorter cells.") }
            func row(_ cells: [String]) -> String {
                "| " + cells.enumerated().map { i, cell in
                    let padding = widths[i] - cell.count, left = alignment == "Right" ? padding : alignment == "Center" ? padding / 2 : 0
                    return String(repeating: " ", count: left) + cell + String(repeating: " ", count: padding - left)
                }.joined(separator: " | ") + " |"
            }
            let rule = "| " + widths.map { width in
                if format != "Markdown" { return String(repeating: "-", count: width) }
                return (alignment == "Center" || alignment == "Left" ? ":" : "-") + String(repeating: "-", count: width - 2) + (alignment == "Center" || alignment == "Right" ? ":" : "-")
            }.joined(separator: " | ") + " |"
            output = ([row(sanitized[0]), rule] + sanitized.dropFirst().map(row)).joined(separator: "\n")
        }
        return .init(text: output, status: "\(data.rows.count) rows × \(data.headers.count) columns · \(format)")
    }
}

private extension Dictionary {
    func mapValuesWithKeys<T>(_ transform: (Key, Value) throws -> T) rethrows -> [Key: T] {
        try Dictionary<Key, T>(uniqueKeysWithValues: map { (key, value) in (key, try transform(key, value)) })
    }
}
