import Foundation

enum ComparisonMode: String, CaseIterable { case lines = "Lines", words = "Words", json = "JSON fields" }
struct ComparisonRow: Identifiable, Equatable {
    enum Kind { case same, added, removed }
    let id: Int
    let kind: Kind
    let text: String
}
struct ComparisonResult {
    let rows: [ComparisonRow]
    var added: Int { rows.filter { $0.kind == .added }.count }
    var removed: Int { rows.filter { $0.kind == .removed }.count }
    var text: String { rows.map { ($0.kind == .added ? "+ " : $0.kind == .removed ? "− " : "  ") + $0.text }.joined(separator: "\n") }
}
enum TextComparison {
    static func compare(_ left: String, _ right: String, mode: ComparisonMode, ignoreWhitespace: Bool) throws -> ComparisonResult {
        try UtilityLimits.check(left); try UtilityLimits.check(right)
        func tokens(_ text: String) throws -> [String] {
            if mode == .json {
                func flatten(_ value: DeveloperJSON, path: String) -> [String] {
                    switch value {
                    case .object(let values) where !values.isEmpty:
                        return values.keys.sorted().flatMap { flatten(values[$0]!, path: path + "[" + DeveloperJSON.quoted($0) + "]") }
                    case .array(let values) where !values.isEmpty:
                        return values.enumerated().flatMap { flatten($0.element, path: path + "[\($0.offset)]") }
                    default: return [path + " = " + value.formatted(pretty: false)]
                    }
                }
                return flatten(try DeveloperJSON.parse(text), path: "$")
            }
            if text.isEmpty { return [] }
            let normalized = text.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n")
            if mode == .words { return normalized.split(whereSeparator: \.isWhitespace).map(String.init) }
            return normalized.components(separatedBy: "\n").map {
                ignoreWhitespace ? $0.split(whereSeparator: \.isWhitespace).joined(separator: " ") : $0
            }
        }
        let a = try tokens(left), b = try tokens(right)
        guard a.count + b.count <= 8_000 else { throw UtilityError("Compare up to 8,000 combined lines, words, or JSON fields. Narrow the selection first.") }
        let difference = b.difference(from: a)
        var removals = Set<Int>(), insertions = Set<Int>()
        for change in difference {
            switch change {
            case .remove(let offset, _, _): removals.insert(offset)
            case .insert(let offset, _, _): insertions.insert(offset)
            }
        }
        var rows: [ComparisonRow] = [], i = 0, j = 0
        func append(_ text: String, _ kind: ComparisonRow.Kind) { rows.append(.init(id: rows.count, kind: kind, text: text)) }
        while i < a.count || j < b.count {
            if i < a.count && removals.contains(i) { append(a[i], .removed); i += 1 }
            else if j < b.count && insertions.contains(j) { append(b[j], .added); j += 1 }
            else if i < a.count && j < b.count { append(a[i], .same); i += 1; j += 1 }
            else { break }
        }
        return ComparisonResult(rows: rows)
    }
}

enum JWTInspector {
    static func inspect(_ raw: String, now: Date = Date()) throws -> String {
        try UtilityLimits.check(raw)
        var token = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if token.lowercased().hasPrefix("bearer ") { token = String(token.dropFirst(7)) }
        let parts = token.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count != 5 else { throw UtilityError("This is an encrypted JWT (JWE). Its payload needs a decryption key and cannot be decoded here.") }
        guard parts.count == 3 else { throw UtilityError("A signed JWT has three dot-separated parts: header.payload.signature.") }
        func decode(_ part: Substring) throws -> DeveloperJSON {
            guard !part.isEmpty, part.utf8.allSatisfy({ (65...90).contains($0) || (97...122).contains($0) || (48...57).contains($0) || $0 == 45 || $0 == 95 }) else {
                throw UtilityError("The token contains an invalid Base64URL segment.")
            }
            var base64 = part.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
            base64 += String(repeating: "=", count: (4 - base64.count % 4) % 4)
            guard let data = Data(base64Encoded: base64), let text = String(data: data, encoding: .utf8) else { throw UtilityError("The token segment is not UTF-8 JSON.") }
            let json = try DeveloperJSON.parse(text)
            guard case .object = json else { throw UtilityError("JWT header and payload must be JSON objects.") }
            return json
        }
        let header = try decode(parts[0]), payload = try decode(parts[1])
        var times: [String] = []
        if case .object(let claims) = payload {
            let formatter = ISO8601DateFormatter()
            for (key, label) in [("exp", "Expires"), ("iat", "Issued"), ("nbf", "Not before")] {
                if let claim = claims[key] {
                    guard case .number(let number) = claim, let seconds = Double(number), seconds.isFinite,
                          seconds >= -62_135_596_800, seconds <= 253_402_300_799 else {
                        times.append("\(label): invalid numeric date"); continue
                    }
                    let date = Date(timeIntervalSince1970: seconds)
                    let relative = key == "exp" ? (date <= now ? " · expired" : " · in \(Int(date.timeIntervalSince(now))) seconds") : ""
                    times.append("\(label): \(formatter.string(from: date))\(relative)")
                }
            }
        }
        return (["DECODED · SIGNATURE NOT VERIFIED", "", "HEADER", header.formatted(), "", "PAYLOAD", payload.formatted()] + (times.isEmpty ? [] : ["", "TIMES (UTC)"] + times)).joined(separator: "\n")
    }
}

struct RegexInspection {
    let ranges: [NSRange]
    let details: String
    let extracted: String
    let replaced: String
}
enum RegexTester {
    static func inspect(text: String, pattern: String, replacement: String, caseInsensitive: Bool, multiline: Bool, timeout: TimeInterval = 0.3) throws -> RegexInspection {
        guard text.utf8.count <= 100_000, pattern.utf8.count <= 4_000 else { throw UtilityError("Regex testing supports 100 KB of text and a 4 KB pattern.") }
        guard !pattern.isEmpty else { throw UtilityError("Enter a regular expression.") }
        var options: NSRegularExpression.Options = []
        if caseInsensitive { options.insert(.caseInsensitive) }
        if multiline { options.insert(.anchorsMatchLines) }
        let regex = try NSRegularExpression(pattern: pattern, options: options)
        let deadline = ProcessInfo.processInfo.systemUptime + timeout
        var matches: [NSTextCheckingResult] = [], stopped = false
        regex.enumerateMatches(in: text, options: [.reportProgress], range: NSRange(text.startIndex..., in: text)) { match, _, stop in
            if ProcessInfo.processInfo.systemUptime > deadline || Task.isCancelled || matches.count >= 1_000 {
                stopped = true; stop.pointee = true; return
            }
            if let match { matches.append(match) }
        }
        guard !stopped else { throw UtilityError("Stopped after the time or 1,000-match limit. Narrow the text or simplify the pattern.") }
        let ns = text as NSString
        var details: [String] = [], extracted: [String] = []
        for (index, match) in matches.enumerated() {
            let value = ns.substring(with: match.range)
            extracted.append(value)
            details.append("\(index + 1). [\(match.range.location)..<\(NSMaxRange(match.range))] \(value)")
            for group in 1..<match.numberOfRanges {
                let range = match.range(at: group)
                details.append("   $\(group): " + (range.location == NSNotFound ? "(not matched)" : ns.substring(with: range)))
            }
        }
        // Reuse the bounded match results; never run the expression a second time for replacement.
        let output = NSMutableString(string: text)
        for match in matches.reversed() {
            output.replaceCharacters(in: match.range, with: regex.replacementString(for: match, in: text, offset: 0, template: replacement))
        }
        return RegexInspection(ranges: matches.map(\.range), details: details.isEmpty ? "No matches." : details.joined(separator: "\n"), extracted: extracted.joined(separator: "\n"), replaced: output as String)
    }
}

struct URLParameter: Identifiable, Equatable {
    var id = UUID()
    var name: String
    var value: String
    var hasValue: Bool = true
}
struct URLInspection {
    var scheme: String
    var host: String
    var port: String
    var path: String
    var fragment: String
    var parameters: [URLParameter]
    private var components: URLComponents

    init(_ text: String) throws {
        try UtilityLimits.check(text)
        guard let c = URLComponents(string: text.trimmingCharacters(in: .whitespacesAndNewlines)),
              let scheme = c.scheme, ["https", "http"].contains(scheme.lowercased()), let host = c.host, !host.isEmpty else {
            throw UtilityError("Enter a complete http:// or https:// URL.")
        }
        components = c
        self.scheme = scheme
        self.host = host
        port = c.port.map(String.init) ?? ""
        path = c.path
        fragment = c.fragment ?? ""
        parameters = (c.queryItems ?? []).map { URLParameter(name: $0.name, value: $0.value ?? "", hasValue: $0.value != nil) }
    }
    func rebuilt() throws -> String {
        var c = components
        c.scheme = scheme
        c.host = host
        if port.isEmpty { c.port = nil }
        else {
            guard let number = Int(port), (1...65535).contains(number) else { throw UtilityError("The port must be between 1 and 65535.") }
            c.port = number
        }
        c.path = path
        c.fragment = fragment.isEmpty ? nil : fragment
        c.queryItems = parameters.isEmpty ? nil : parameters.map { URLQueryItem(name: $0.name, value: $0.hasValue ? $0.value : nil) }
        guard !host.isEmpty, !host.contains(where: \.isWhitespace), let url = c.url else { throw UtilityError("The edited URL is invalid.") }
        return url.absoluteString
    }
}
