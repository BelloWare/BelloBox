import Foundation

enum LauncherCommand: String, CaseIterable, Identifiable {
    case json, compare, jwt, regex, url, time, cron, convert, snippets, http, generate
    case ai, screenshot, scrollCapture, recording, worldClock, qr, textTools, settings, home
    var id: String { rawValue }
    var isDeveloperTool: Bool { Self.allCases.firstIndex(of: self)! < Self.allCases.firstIndex(of: .ai)! }
    var title: String {
        switch self {
        case .json: return "JSON Tools"
        case .compare: return "Compare Text & JSON"
        case .jwt: return "Inspect JWT"
        case .regex: return "Regex Tester"
        case .url: return "URL & Query Editor"
        case .time: return "Timestamp Converter"
        case .cron: return "Cron Schedule"
        case .convert: return "Convert JSON, YAML & CSV"
        case .snippets: return "Snippets & Templates"
        case .http: return "HTTP & cURL"
        case .generate: return "Developer Generators"
        case .ai: return "Ask AI"
        case .screenshot: return "Screenshot"
        case .scrollCapture: return "Scrolling Screenshot"
        case .recording: return "Screen Recording"
        case .worldClock: return "World Clock"
        case .qr: return "QR Code"
        case .textTools: return "Text Tools"
        case .settings: return "Settings"
        case .home: return "Home & Status"
        }
    }
    var subtitle: String {
        switch self {
        case .json: return "Pretty-print, minify, and validate without rounding numbers"
        case .compare: return "Find changes between selections, clipboard text, and JSON fields"
        case .jwt: return "Read token claims and expiration times locally"
        case .regex: return "Test patterns, inspect groups, extract, and replace"
        case .url: return "Edit URL components and repeated query parameters"
        case .time: return "Unix seconds, milliseconds, ISO dates, and time differences"
        case .cron: return "Explain five-field cron and preview upcoming runs"
        case .convert: return "Convert structured data and preview rows as a table"
        case .snippets: return "Save reusable text with fields you fill before inserting"
        case .http: return "Import a cURL command, edit the request, then send"
        case .generate: return "UUIDs, random strings, timestamps, and sample records"
        case .ai: return "Rewrite, explain, summarize, or ask about the selected text"
        case .screenshot: return "Capture an area, window, or screen and annotate"
        case .scrollCapture: return "Capture a longer page and stitch it into one image"
        case .recording: return "Record screen, audio, cursor, clicks, and keys"
        case .worldClock: return "Compare live time or plan a meeting across locations"
        case .qr: return "Create a scannable code from text or a link"
        case .textTools: return "Case, encode, decode, hashes, lines, and counts"
        case .settings: return "Shortcuts, permissions, providers, and defaults"
        case .home: return "App status, setup guide, and updates"
        }
    }
    var symbol: String {
        switch self {
        case .json: return "curlybraces"
        case .compare: return "arrow.left.arrow.right"
        case .jwt: return "key.horizontal"
        case .regex: return "asterisk"
        case .url: return "link"
        case .time: return "clock"
        case .cron: return "calendar.badge.clock"
        case .convert: return "tablecells"
        case .snippets: return "text.badge.plus"
        case .http: return "network"
        case .generate: return "number"
        case .ai: return "wand.and.stars"
        case .screenshot: return "camera.viewfinder"
        case .scrollCapture: return "arrow.down.doc"
        case .recording: return "record.circle"
        case .worldClock: return "globe"
        case .qr: return "qrcode"
        case .textTools: return "wrench.and.screwdriver"
        case .settings: return "gearshape"
        case .home: return "house"
        }
    }
    var keywords: String {
        switch self {
        case .json: return "format prettify pretty print compact lint errors sort object array"
        case .compare: return "diff difference compare clipboard changes"
        case .jwt: return "bearer authorization auth token decode exp iat nbf"
        case .regex: return "regular expression matches capture groups extract replace ICU"
        case .url: return "uri query parameters encode decode host path fragment"
        case .time: return "epoch unix timestamp date utc milliseconds seconds duration"
        case .cron: return "crontab scheduled jobs next run daily weekly"
        case .convert: return "json yaml csv tsv spreadsheet table data format"
        case .snippets: return "template boilerplate saved text variables insert"
        case .http: return "curl request api response rest headers post get endpoint"
        case .generate: return "generate uuid guid random password string test data fixture records"
        default: return subtitle
        }
    }
    static func suggestions(for text: String) -> [LauncherCommand] {
        guard !text.isEmpty, text.utf8.prefix(UtilityLimits.inputBytes + 1).count <= UtilityLimits.inputBytes else { return [] }
        let sample = String(String.UnicodeScalarView(text.unicodeScalars.prefix(2_048)))
        let text = sample.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.hasPrefix("{") || text.hasPrefix("[") { return [.json, .compare, .convert] }
        if text.hasPrefix("curl ") || text.hasPrefix("curl\n") { return [.http] }
        if text.split(separator: ".", omittingEmptySubsequences: false).count == 3 && !text.contains(where: \.isWhitespace), text.hasPrefix("eyJ") { return [.jwt] }
        if text.lowercased().hasPrefix("bearer eyj") { return [.jwt] }
        if text.lowercased().hasPrefix("https://") || text.lowercased().hasPrefix("http://") { return [.url, .qr, .http] }
        if text.utf8.count <= 256 {
            if TimestampSummary.make(from: text) != nil { return [.worldClock, .time] }
            if (try? CronSchedule(text)) != nil { return [.cron] }
        }
        if text.contains("\n") && (text.contains(": ") || text.contains(",") || text.contains("\t")) { return [.convert, .compare] }
        return [.textTools, .compare, .snippets]
    }
    static func search(_ query: String, input: String, favorites: Set<String>, recents: [String], suggested: [LauncherCommand]? = nil) -> [LauncherCommand] {
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let terms = query.lowercased().split(whereSeparator: \.isWhitespace)
        let suggested = suggested ?? suggestions(for: input)
        return allCases.filter { command in
            let haystack = (command.title + " " + command.keywords).lowercased()
            return terms.allSatisfy { haystack.contains($0) }
        }.sorted { left, right in
            func score(_ command: Self) -> Int {
                var score = favorites.contains(command.id) ? 100 : 0
                // A favorite must not displace the best interpretation of the
                // selection. Explicit search prefixes still take precedence.
                if let i = suggested.firstIndex(of: command) { score += 1_000 - i * 200 }
                if let i = recents.firstIndex(of: command.id) { score += 40 - i }
                if !query.isEmpty && command.title.lowercased().hasPrefix(query.lowercased()) { score += 10_000 }
                return score
            }
            let a = score(left), b = score(right)
            return a == b ? allCases.firstIndex(of: left)! < allCases.firstIndex(of: right)! : a > b
        }
    }
}
