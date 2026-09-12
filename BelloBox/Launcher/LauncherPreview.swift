import Foundation

/// Facts the palette knows on the main actor that a preview may mention. It is
/// captured once per preview and never contains the clipboard or any secret.
struct LauncherPreviewContext: Equatable, Sendable {
    var zoneIDs: [String] = []
    var snippetCount: Int = 0
    /// The configured AI provider's display name, or nil when none is usable.
    var aiProviderName: String? = nil
}

/// Read-only, bounded display data. Building a preview never opens a workbench,
/// touches the clipboard, persists input, evaluates a shell, or sends a request.
struct LauncherPreview: Equatable {
    struct Field: Equatable {
        let label: String
        let value: String
    }
    struct Clock: Equatable {
        let name: String
        let time: String
        let date: String
        let zone: String
        let quality: MeetingTimeQuality
    }
    /// One line of "what opening this tool gives you": a symbol and a short sentence.
    struct Action: Equatable {
        let symbol: String
        let text: String
    }
    enum Content: Equatable {
        /// Static clocks plus the parsed instant; the palette upgrades this to
        /// an interactive World Clock planner seeded at that instant.
        case clocks([Clock], instant: Date)
        case code(String)
        case fields([Field])
        case statistics([Field])
        case notice(String)
        /// Concise capabilities for tools that do not transform the selection.
        case actions([Action])
    }
    let title: String
    let subtitle: String
    let content: Content
    var isWarning = false
    static let parsingByteLimit = 64_000

    static func make(text: String, command: LauncherCommand, context: LauncherPreviewContext = LauncherPreviewContext(),
                     now: Date = Date(), localZone: TimeZone = .current, locale: Locale = .current) throws -> Self {
        try UtilityLimits.check(text)
        let input = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if input.isEmpty { return empty(for: command, context: context, now: now, localZone: localZone, locale: locale) }
        if Self.parses(command), text.utf8.prefix(parsingByteLimit + 1).count > parsingByteLimit {
            return Self(title: "Full selection ready", subtitle: "\(text.count.formatted()) characters · preview kept compact",
                content: .notice("Open \(command.title) to work with the complete selection. Nothing has been truncated."))
        }
        switch command {
        case .calculator, .units, .numberBase, .color, .contrast, .gradient, .markdown, .jsonPointer, .jsonFlatten, .jsonCode, .sqlInsert, .xmlJSON, .unicode, .stringEscape, .extract, .listSet, .semver, .subnet, .chmod, .hmac, .jsonSchema, .jsonMerge, .jsonRedact, .jsonLines, .csvExplore, .envFile, .plist, .sqlFormat, .httpHeaders, .cookies, .certificate, .sshKey, .uuidInspect, .bitwise, .statistics, .dateMath, .aspectRatio, .bezier, .boxShadow, .textTable:
            return Self(title: command.title, subtitle: command.subtitle, content: .actions(capabilities(for: command)))
        case .worldClock:
            guard let summary = TimestampSummary.make(from: input, relativeTo: now, locale: locale, timeZone: localZone) else {
                return clocks(at: now, zoneIDs: context.zoneIDs, localZone: localZone, locale: locale, showSeconds: false,
                              title: "Current time", subtitle: "No timestamp in the selection · press ↵ for the planner")
            }
            return clocks(at: summary.date, zoneIDs: context.zoneIDs, localZone: localZone, locale: locale, showSeconds: true,
                          title: "Timestamp recognized", subtitle: summary.relativeTime)
        case .time:
            guard let summary = TimestampSummary.make(from: input, relativeTo: now, locale: locale, timeZone: localZone) else {
                throw UtilityError("Select a Unix timestamp or an ISO date to convert it.")
            }
            let iso = ISO8601DateFormatter()
            iso.timeZone = TimeZone(secondsFromGMT: 0)
            let seconds = summary.date.timeIntervalSince1970
            let milliseconds = (seconds * 1_000).rounded()
            return Self(title: "Timestamp recognized", subtitle: summary.relativeTime,
                content: .fields([Field(label: "Local", value: summary.localDateTime),
                                  Field(label: "UTC", value: iso.string(from: summary.date)),
                                  Field(label: "Unix", value: "\(Int64(seconds.rounded(.down))) s · \(Int64(milliseconds)) ms")]))
        case .json:
            let json = try DeveloperJSON.parse(input)
            let description: String
            switch json {
            case .object(let values): description = "\(values.count) properties"
            case .array(let values): description = "\(values.count) items"
            default: description = "JSON value"
            }
            return code(json.formatted(), title: "Valid JSON", subtitle: description + " · numbers preserved")
        case .jwt:
            let output = try JWTInspector.inspect(input, now: now)
            // The warning remains visible above the excerpt, even when it is long.
            return code(output, title: "JWT decoded", subtitle: "Signature not verified", warning: true)
        case .url:
            let url = try URLInspection(input)
            var fields = [Field(label: "Host", value: url.host + (url.port.isEmpty ? "" : ":" + url.port)),
                          Field(label: "Path", value: url.path.isEmpty ? "/" : url.path)]
            fields += url.parameters.prefix(2).map { Field(label: $0.name, value: $0.hasValue ? $0.value : "(flag)") }
            if !url.fragment.isEmpty && fields.count < 4 { fields.append(Field(label: "Fragment", value: url.fragment)) }
            return Self(title: "URL recognized", subtitle: "\(url.scheme.uppercased()) · \(url.parameters.count) query parameters",
                        content: .fields(bounded(fields)))
        case .http:
            if input.hasPrefix("curl") {
                let draft = try CurlImporter.parse(input)
                let headers = draft.headers.split(separator: "\n").count
                return Self(title: "cURL request recognized", subtitle: "Review in the editor before sending",
                    content: .fields(bounded([Field(label: "Method", value: draft.method), Field(label: "URL", value: draft.url),
                        Field(label: "Headers", value: "\(headers)"), Field(label: "Body", value: "\(draft.body.utf8.count.formatted()) bytes")])))
            }
            let url = try URLInspection(input)
            return Self(title: "Request from URL", subtitle: "Nothing is sent until you choose Send",
                content: .fields(bounded([Field(label: "Method", value: "GET"), Field(label: "URL", value: input),
                    Field(label: "Host", value: url.host)])))
        case .cron:
            let cron = try CronSchedule(input)
            let next = try cron.next(after: now, zone: localZone, count: 3)
            let formatter = DateFormatter(); formatter.locale = locale; formatter.timeZone = localZone
            formatter.setLocalizedDateFormatFromTemplate("EEE MMM d HHmm")
            let fields = next.enumerated().map { Field(label: "Next \($0.offset + 1)", value: formatter.string(from: $0.element)) }
            return Self(title: "Cron schedule", subtitle: localZone.identifier + " · standard 5-field cron",
                content: fields.isEmpty ? .notice("No matching run in the next 8 years. Open the schedule to inspect its fields.") : .fields(fields))
        case .convert:
            let format = DataConversion.detectFormat(input)
            let delimiter: Character = input.contains("\t") && !input.contains(",") ? "\t" : ","
            let (output, table) = try DataConversion.convert(input, from: format, to: .json, delimiter: delimiter)
            let detail = table.map { "\($0.totalRows) rows · \($0.columns.count) columns" } ?? "JSON preview"
            return code(output, title: "\(format.rawValue) recognized", subtitle: detail)
        case .qr:
            let bytes = text.utf8.count
            let fits = bytes <= QRCodeGenerator.maxByteCount
            let capacity = fits
                ? "\((QRCodeGenerator.maxByteCount - bytes).formatted()) bytes to spare"
                : "\((bytes - QRCodeGenerator.maxByteCount).formatted()) bytes over the \(QRCodeGenerator.maxByteCount.formatted())-byte limit"
            return Self(title: fits ? "Ready to encode" : "Selection too long for one code",
                        subtitle: fits ? "Scannable from the popup · copy or save as PNG" : "Shorten the text in the popup",
                        content: .fields(bounded([Field(label: "Content", value: firstLine(text)),
                                                  Field(label: "Size", value: "\(bytes.formatted()) bytes · \(capacity)")])),
                        isWarning: !fits)
        case .compare:
            return Self(title: "Ready to compare", subtitle: "Diff against the clipboard or a pinned selection in the tool",
                        content: .statistics(textStatistics(text)))
        case .regex:
            return Self(title: "Test text ready", subtitle: "Type a pattern in the tool to see live matches",
                        content: .statistics(textStatistics(text)))
        case .snippets:
            let placeholders = SnippetTemplate.placeholders(text)
            var actions = [Action(symbol: "square.and.arrow.down", text: "Save this selection as a reusable snippet")]
            actions.append(placeholders.isEmpty
                ? Action(symbol: "curlybraces", text: "Add {{fields}} to fill in before inserting")
                : Action(symbol: "curlybraces", text: "\(placeholders.count) field\(placeholders.count == 1 ? "" : "s") to fill: " + placeholders.prefix(4).joined(separator: ", ")))
            actions.append(snippetCountAction(context.snippetCount))
            return Self(title: "Snippet template", subtitle: "\(text.count.formatted()) characters", content: .actions(actions))
        case .ai:
            return Self(title: "Ask AI about the selection", subtitle: aiSubtitle(context),
                        content: .actions([
                            Action(symbol: "wand.and.stars", text: "Rewrite, fix, summarize, translate, or ask a question"),
                            Action(symbol: "hand.raised", text: "Nothing is sent until you choose an action"),
                            Action(symbol: "text.alignleft", text: "\(text.count.formatted()) characters selected")
                        ]))
        case .generate, .screenshot, .scrollCapture, .recording, .videoToGIF, .settings, .home:
            return staticPreview(for: command, context: context)
        case .textTools:
            return Self(title: "Text at a glance", subtitle: "Case, encoding, hashes, and line tools",
                        content: .statistics(textStatistics(text)))
        }
    }

    /// The preview for a tool that opens without input, or for the palette without a selection.
    private static func empty(for command: LauncherCommand, context: LauncherPreviewContext, now: Date, localZone: TimeZone, locale: Locale) -> Self {
        switch command {
        case .calculator, .units, .numberBase, .color, .contrast, .gradient, .markdown, .jsonPointer, .jsonFlatten, .jsonCode, .sqlInsert, .xmlJSON, .unicode, .stringEscape, .extract, .listSet, .semver, .subnet, .chmod, .hmac, .jsonSchema, .jsonMerge, .jsonRedact, .jsonLines, .csvExplore, .envFile, .plist, .sqlFormat, .httpHeaders, .cookies, .certificate, .sshKey, .uuidInspect, .bitwise, .statistics, .dateMath, .aspectRatio, .bezier, .boxShadow, .textTable:
            return Self(title: command.title, subtitle: command.subtitle, content: .actions(capabilities(for: command)))
        case .worldClock:
            return clocks(at: now, zoneIDs: context.zoneIDs, localZone: localZone, locale: locale, showSeconds: false,
                          title: "Current time", subtitle: "Your locations · press ↵ to plan a meeting")
        case .json, .compare, .jwt, .regex, .url, .time, .cron, .convert, .http, .textTools, .qr, .ai:
            var actions = capabilities(for: command, context: context)
            actions.append(Action(symbol: "doc.on.clipboard", text: "Opens with an empty input · paste or choose Use Clipboard"))
            return Self(title: command.title, subtitle: "No text selected", content: .actions(Array(actions.prefix(3))))
        case .snippets:
            return Self(title: command.title, subtitle: "No text selected", content: .actions([
                Action(symbol: "text.badge.plus", text: "Insert a saved snippet, filling its {{fields}} first"),
                Action(symbol: "curlybraces", text: "Starts from a template with {{name}} and {{selection}}"),
                snippetCountAction(context.snippetCount)
            ]))
        case .generate, .screenshot, .scrollCapture, .recording, .videoToGIF, .settings, .home:
            return staticPreview(for: command, context: context)
        }
    }

    private static func staticPreview(for command: LauncherCommand, context: LauncherPreviewContext) -> Self {
        let subtitle: String
        switch command {
        case .generate: subtitle = "The selection is not needed"
        case .screenshot, .scrollCapture: subtitle = "Stays on this Mac · needs Screen Recording permission"
        case .recording: subtitle = "MOV or silent GIF · needs Screen Recording permission"
        case .videoToGIF: subtitle = "Converts locally · you choose the file"
        case .settings, .home: subtitle = "Bello Box"
        default: subtitle = ""
        }
        return Self(title: command.title, subtitle: subtitle, content: .actions(Array(capabilities(for: command, context: context).prefix(3))))
    }

    /// Two or three sentences per tool: what opening it lets you do.
    static func capabilities(for command: LauncherCommand, context: LauncherPreviewContext = LauncherPreviewContext()) -> [Action] {
        switch command {
        case .calculator, .units, .numberBase, .color, .contrast, .gradient, .markdown, .jsonPointer, .jsonFlatten, .jsonCode, .sqlInsert, .xmlJSON, .unicode, .stringEscape, .extract, .listSet, .semver, .subnet, .chmod, .hmac, .jsonSchema, .jsonMerge, .jsonRedact, .jsonLines, .csvExplore, .envFile, .plist, .sqlFormat, .httpHeaders, .cookies, .certificate, .sshKey, .uuidInspect, .bitwise, .statistics, .dateMath, .aspectRatio, .bezier, .boxShadow, .textTable: return [Action(symbol: command.symbol, text: command.subtitle), Action(symbol: "slider.horizontal.3", text: "Edit here or open the full tool; your draft carries over")]
        case .json: return [Action(symbol: "curlybraces", text: "Pretty-print, minify, validate, and sort keys"),
                            Action(symbol: "number", text: "Large numbers are never rounded")]
        case .compare: return [Action(symbol: "arrow.left.arrow.right", text: "Diff lines, words, or JSON fields"),
                               Action(symbol: "pin", text: "Pin one text, then compare it with another selection")]
        case .jwt: return [Action(symbol: "key.horizontal", text: "Decode header and claims locally"),
                           Action(symbol: "clock", text: "Shows issued, not-before, and expiry times")]
        case .regex: return [Action(symbol: "asterisk", text: "Live matches, groups, extract, and replace"),
                             Action(symbol: "textformat", text: "ICU regular expressions")]
        case .url: return [Action(symbol: "link", text: "Edit scheme, host, path, and repeated parameters"),
                           Action(symbol: "hammer", text: "Rebuild the URL from your edits")]
        case .time: return [Action(symbol: "clock", text: "Unix seconds, milliseconds, and ISO dates"),
                            Action(symbol: "arrow.left.arrow.right", text: "Differences between two times in any zone")]
        case .cron: return [Action(symbol: "calendar.badge.clock", text: "Explain a five-field schedule"),
                            Action(symbol: "list.number", text: "Preview the next runs in a time zone")]
        case .convert: return [Action(symbol: "tablecells", text: "JSON, YAML, CSV, and TSV in any direction"),
                               Action(symbol: "eye", text: "Preview rows as a table before converting")]
        case .snippets: return [Action(symbol: "text.badge.plus", text: "Insert saved text with fields you fill first"),
                                snippetCountAction(context.snippetCount)]
        case .http: return [Action(symbol: "network", text: "Import a cURL command or start from a URL"),
                            Action(symbol: "paperplane", text: "Sends only when you choose Send")]
        case .generate: return [Action(symbol: "number", text: "UUIDs, random strings, and timestamps"),
                                Action(symbol: "tablecells", text: "Sample records as lines, JSON, or CSV")]
        case .ai: return [Action(symbol: "wand.and.stars", text: "Rewrite, fix, summarize, translate, or ask"),
                          Action(symbol: "sparkles", text: aiSubtitle(context))]
        case .screenshot: return [Action(symbol: "camera.viewfinder", text: "Capture an area, window, or screen"),
                                  Action(symbol: "pencil.tip", text: "Annotate, mask, erase, and crop, then copy or save"),
                                  Action(symbol: "text.viewfinder", text: "Read text with local OCR")]
        case .scrollCapture: return [Action(symbol: "arrow.down.doc", text: "Select an area, then scroll it yourself or auto-scroll"),
                                     Action(symbol: "rectangle.stack", text: "Frames are stitched into one tall image"),
                                     Action(symbol: "magnifyingglass", text: "Review at fit, fit width, or 100%")]
        case .recording: return [Action(symbol: "record.circle", text: "Record an area, window, or screen"),
                                 Action(symbol: "keyboard", text: "Audio, cursor, clicks, keys, privacy, and countdown"),
                                 Action(symbol: "photo.stack", text: "Deliver a movie or a silent GIF")]
        case .videoToGIF: return [Action(symbol: "film", text: "Choose a movie on this Mac"),
                                  Action(symbol: "slider.horizontal.3", text: "Trim, frame rate, width, and looping"),
                                  Action(symbol: "photo.stack", text: "Preview the GIF, then save it")]
        case .worldClock: return [Action(symbol: "globe", text: "Compare live time across your locations"),
                                  Action(symbol: "calendar", text: "Plan a meeting on a shared timeline")]
        case .qr: return [Action(symbol: "qrcode", text: "A live, scannable code from any text or link"),
                          Action(symbol: "square.and.arrow.down", text: "Copy the image or save it as PNG")]
        case .textTools: return [Action(symbol: "textformat", text: "Case, encode, decode, and pretty-print"),
                                 Action(symbol: "number", text: "Hashes, line tools, and counts")]
        case .settings: return [Action(symbol: "keyboard", text: "Shortcuts and default tool behavior"),
                                Action(symbol: "lock.shield", text: "Permissions and AI providers")]
        case .home: return [Action(symbol: "house", text: "App status, setup guide, and updates"),
                            Action(symbol: "square.grid.2x2", text: "Every tool by category")]
        }
    }

    private static func aiSubtitle(_ context: LauncherPreviewContext) -> String {
        context.aiProviderName.map { "Provider: \($0)" } ?? "No AI provider yet · set one up in Settings"
    }

    private static func snippetCountAction(_ count: Int) -> Action {
        Action(symbol: "folder", text: count == 0 ? "No snippets saved yet" : "\(count.formatted()) snippet\(count == 1 ? "" : "s") saved on this Mac")
    }

    /// Tools whose preview has to parse the whole selection.
    private static func parses(_ command: LauncherCommand) -> Bool {
        if command.additionalTool != nil { return true }
        return [.json, .jwt, .url, .http, .cron, .convert, .time, .worldClock].contains(command)
    }

    private static func clocks(at instant: Date, zoneIDs: [String], localZone: TimeZone, locale: Locale, showSeconds: Bool,
                               title: String, subtitle: String) -> Self {
        // Keep the saved order; show local time and UTC as useful fallbacks
        // without changing the user's World Clock locations.
        let ids = WorldClockViewModel.previewZoneIDs(saved: zoneIDs, localZone: localZone)
        let clocks = ids.compactMap { id -> Clock? in
            guard let zone = TimeZone(identifier: id) else { return nil }
            let time = DateFormatter(); time.locale = locale; time.timeZone = zone
            time.setLocalizedDateFormatFromTemplate(showSeconds ? "jmmss" : "jmm")
            let day = DateFormatter(); day.locale = locale; day.timeZone = zone
            day.setLocalizedDateFormatFromTemplate("EEE MMM d yyyy")
            let offset = zone.secondsFromGMT(for: instant)
            let offsetText = String(format: "UTC%@%02d:%02d", offset < 0 ? "−" : "+", abs(offset) / 3_600, abs(offset) % 3_600 / 60)
            return Clock(name: WorldClockZoneCatalog.option(for: id).name, time: time.string(from: instant),
                         date: day.string(from: instant), zone: offsetText,
                         quality: MeetingTimeQuality.at(instant, in: zone))
        }
        return Self(title: title, subtitle: subtitle, content: .clocks(clocks, instant: instant))
    }

    static func textStatistics(_ text: String) -> [Field] {
        let input = text.trimmingCharacters(in: .whitespacesAndNewlines)
        let words = input.split(whereSeparator: \.isWhitespace).count
        let lines = text.isEmpty ? 0 : text.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n").components(separatedBy: "\n").count
        return [Field(label: "Characters", value: text.count.formatted()),
                Field(label: "Words", value: words.formatted()), Field(label: "Lines", value: lines.formatted()),
                Field(label: "UTF-8 bytes", value: text.utf8.count.formatted())]
    }

    private static func firstLine(_ text: String) -> String {
        let line = text.split(whereSeparator: \.isNewline).first.map(String.init) ?? text
        return String(String.UnicodeScalarView(line.unicodeScalars.prefix(120))) + (line.unicodeScalars.count > 120 ? "…" : "")
    }

    static func failure(_ error: Error, command: LauncherCommand) -> Self {
        Self(title: "Not recognized for \(command.title)", subtitle: "Opens with the selection so you can edit it",
             content: .notice(String(String.UnicodeScalarView(error.localizedDescription.unicodeScalars.prefix(240)))), isWarning: true)
    }
    private static func code(_ text: String, title: String, subtitle: String, warning: Bool = false) -> Self {
        let first = String(String.UnicodeScalarView(text.unicodeScalars.prefix(900)))
        let lines = first.components(separatedBy: "\n")
        let excerpt = lines.prefix(5).joined(separator: "\n")
        let shortened = text != excerpt
        return Self(title: title, subtitle: subtitle + (shortened ? " · excerpt" : ""),
                    content: .code(excerpt + (shortened ? "\n…" : "")), isWarning: warning)
    }
    private static func bounded(_ fields: [Field]) -> [Field] {
        fields.prefix(4).map { field in
            func limit(_ value: String) -> String {
                let scalars = value.unicodeScalars.prefix(160)
                return String(String.UnicodeScalarView(scalars)) + (value.unicodeScalars.count > 160 ? "…" : "")
            }
            return Field(label: limit(field.label), value: limit(field.value))
        }
    }
}
