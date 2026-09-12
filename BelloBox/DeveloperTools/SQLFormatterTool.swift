import Foundation

enum SQLFormatterTool {
    enum Kind { case word, number, literal, symbol, lineComment, blockComment }
    struct Token { let text: String; let kind: Kind; let newlineBefore: Bool }
    static let keywords: Set<String> = ["SELECT", "DISTINCT", "FROM", "WHERE", "AND", "OR", "NOT", "NULL", "IS", "IN", "AS", "ON", "JOIN", "LEFT", "RIGHT", "FULL", "INNER", "OUTER", "CROSS", "GROUP", "ORDER", "BY", "ASC", "DESC", "HAVING", "LIMIT", "OFFSET", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "RETURNING", "WITH", "UNION", "ALL", "EXCEPT", "INTERSECT", "CASE", "WHEN", "THEN", "ELSE", "END", "TRUE", "FALSE", "BETWEEN", "LIKE", "EXISTS", "CREATE", "TABLE", "DROP", "ALTER"]
    static func tokenize(_ text: String, dialect: String = "PostgreSQL") throws -> [Token] {
        let chars = Array(text), n = chars.count
        guard n <= 150_000 else { throw UtilityError("Format up to 150,000 SQL characters at a time.") }
        var i = 0, tokens: [Token] = [], newline = false
        func isWord(_ c: Character) -> Bool { c.isLetter || c.isNumber || c == "_" || c == "$" }
        func at(_ offset: Int) -> Character? { i + offset < n ? chars[i + offset] : nil }
        while i < n {
            try Task.checkCancellation()
            if chars[i].isWhitespace { newline = newline || chars[i].isNewline; i += 1; continue }
            let start = i, kind: Kind
            if chars[i] == "-" && at(1) == "-" && (dialect != "MySQL" || at(2) == nil || at(2)?.isWhitespace == true) || dialect == "MySQL" && chars[i] == "#" {
                kind = .lineComment; i += chars[i] == "#" ? 1 : 2
                while i < n && !chars[i].isNewline { i += 1 }
            } else if chars[i] == "/" && at(1) == "*" {
                kind = .blockComment; i += 2; var depth = 1
                while i < n && depth > 0 {
                    if chars[i] == "/" && at(1) == "*" { depth += 1; i += 2 }
                    else if chars[i] == "*" && at(1) == "/" { depth -= 1; i += 2 }
                    else { i += 1 }
                    guard depth <= 64 else { throw UtilityError("SQL comment nesting exceeds 64 levels.") }
                }
                guard depth == 0 else { throw UtilityError("Close the /* block comment before formatting.") }
            } else if chars[i] == "$", let closing = chars[(i + 1)...].firstIndex(of: "$"), closing - i <= 64,
                      (closing == i + 1 || (chars[i + 1].isLetter || chars[i + 1] == "_") && chars[(i + 1)..<closing].allSatisfy({ $0.isLetter || $0.isNumber || $0 == "_" })) {
                let delimiter = Array(chars[i...closing]); i = closing + 1; kind = .literal
                var found = false
                while i + delimiter.count <= n {
                    if chars[i..<(i + delimiter.count)].elementsEqual(delimiter) { i += delimiter.count; found = true; break }
                    i += 1
                }
                guard found else { throw UtilityError("Close the SQL dollar-quoted string.") }
            } else if ["'", "\"", "`", "["].contains(chars[i]) || ["e", "E", "b", "B", "x", "X", "n", "N"].contains(chars[i]) && at(1) == "'" || ["u", "U"].contains(chars[i]) && at(1) == "&" && (at(2) == "'" || at(2) == "\"") {
                let escaped = chars[i] == "e" || chars[i] == "E" || dialect == "MySQL"
                if ["u", "U"].contains(chars[i]) && at(1) == "&" { i += 2 }
                if at(1) == "'" && !["'", "\"", "`", "["].contains(chars[i]) { i += 1 }
                let opening = chars[i], quote: Character = opening == "[" ? "]" : opening
                kind = .literal; i += 1; var found = false
                while i < n {
                    if escaped && chars[i] == "\\" { i = min(n, i + 2); continue }
                    if chars[i] == quote {
                        i += 1
                        if i < n && chars[i] == quote { i += 1; continue }
                        found = true; break
                    }
                    i += 1
                }
                guard found else { throw UtilityError("Close the quoted SQL literal or identifier.") }
            } else if dialect == "MySQL" && chars[i] == "-" && at(1) == "-" {
                kind = .symbol; i += 1
            } else if [":", "@", "?"].contains(chars[i]), let next = at(1), next.isLetter || next == "_" || chars[i] == "?" && next.isNumber {
                kind = .literal; i += 1
                while i < n && isWord(chars[i]) { i += 1 }
            } else if chars[i].isNumber || chars[i] == "." && at(1)?.isNumber == true {
                kind = .number
                if chars[i] == "0", at(1) == "x" || at(1) == "X" {
                    i += 2; while i < n && chars[i].isHexDigit { i += 1 }
                } else {
                    while i < n && chars[i].isNumber { i += 1 }
                    if i < n && chars[i] == "." { i += 1; while i < n && chars[i].isNumber { i += 1 } }
                    if i < n && ["e", "E"].contains(chars[i]) {
                        i += 1; if i < n && ["+", "-"].contains(chars[i]) { i += 1 }
                        while i < n && chars[i].isNumber { i += 1 }
                    }
                }
            } else if isWord(chars[i]) {
                kind = .word; i += 1
                while i < n && isWord(chars[i]) { i += 1 }
            } else {
                kind = .symbol; i += 1
                if "=<>!|&+*/%~^-:?@#".contains(chars[start]) {
                    while i < n && "=<>!|&+*/%~^-:?@#".contains(chars[i]) {
                        if chars[i] == "-" && at(1) == "-" || chars[i] == "/" && at(1) == "*" { break }
                        i += 1
                    }
                }
            }
            tokens.append(Token(text: String(chars[start..<i]), kind: kind, newlineBefore: newline)); newline = false
            guard tokens.count <= 30_000 else { throw UtilityError("SQL exceeds the 30,000-token formatting limit.") }
        }
        return tokens
    }
    static func format(_ input: String, uppercase: Bool, dialect: String = "PostgreSQL") throws -> WorkbenchResult {
        let tokens = try tokenize(input, dialect: dialect)
        var lines: [String] = [], line = "", depth = 0
        func flush() { if !line.trimmingCharacters(in: .whitespaces).isEmpty { lines.append(line); line = "" } }
        func append(_ text: String, tight: Bool = false) {
            if line.isEmpty { line = String(repeating: "  ", count: min(depth, 20)) + text }
            else { line += (tight ? "" : " ") + text }
        }
        let starters: Set<String> = ["SELECT", "FROM", "WHERE", "HAVING", "LIMIT", "OFFSET", "VALUES", "SET", "RETURNING", "UNION", "EXCEPT", "INTERSECT", "WITH"]
        for (index, token) in tokens.enumerated() {
            try Task.checkCancellation()
            let previous = index > 0 ? tokens[index - 1] : nil, upper = token.text.uppercased()
            let word = token.kind == .word, text = word && uppercase && keywords.contains(upper) ? upper : token.text
            let next = index + 1 < tokens.count ? tokens[index + 1].text.uppercased() : ""
            if token.kind == .lineComment { append(text); flush(); continue }
            if token.kind == .blockComment { flush(); append(text); flush(); continue }
            if word && (starters.contains(upper) || ["GROUP", "ORDER"].contains(upper) && next == "BY" || ["LEFT", "RIGHT", "FULL", "INNER", "CROSS"].contains(upper) && ["JOIN", "OUTER"].contains(next) || upper == "JOIN" && !(previous.map { ["LEFT", "RIGHT", "FULL", "INNER", "CROSS", "OUTER"].contains($0.text.uppercased()) } ?? false)) { flush() }
            if token.kind == .literal && previous?.kind == .literal && token.newlineBefore { flush() }
            if text == ")" { depth = max(0, depth - 1) }
            let tight = [",", ";", ")", ".", "::"].contains(text) || previous.map { ["(", ".", "::"].contains($0.text) } == true || text == "(" && previous?.kind == .word && !keywords.contains(previous!.text.uppercased())
            append(text, tight: tight)
            if text == "(" { depth += 1; guard depth <= 64 else { throw UtilityError("SQL parentheses exceed 64 levels.") } }
            if text == ";" || text == "," && depth == 0 { flush() }
        }
        flush()
        return .init(text: lines.joined(separator: "\n"), status: "\(dialect) formatted · literals preserved · not validated or executed")
    }
}
