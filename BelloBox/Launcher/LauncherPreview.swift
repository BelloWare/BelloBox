import Foundation

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
    enum Content: Equatable {
        case clocks([Clock])
        case code(String)
        case fields([Field])
        case statistics([Field])
        case notice(String)
    }
    let title: String
    let subtitle: String
    let content: Content
    var isWarning = false
    static let parsingByteLimit = 64_000

    static func make(text: String, command: LauncherCommand, zoneIDs: [String], now: Date = Date(),
                     localZone: TimeZone = .current, locale: Locale = .current) throws -> Self {
        try UtilityLimits.check(text)
        guard text.utf8.prefix(parsingByteLimit + 1).count <= parsingByteLimit else {
            return Self(title: "Full selection ready", subtitle: "\(text.count.formatted()) characters · preview kept compact",
                content: .notice("Open \(command.title) to work with the complete selection. Nothing has been truncated."))
        }
        let input = text.trimmingCharacters(in: .whitespacesAndNewlines)
        switch command {
        case .worldClock, .time:
            guard let summary = TimestampSummary.make(from: input, relativeTo: now, locale: locale, timeZone: localZone) else {
                throw UtilityError("Select a Unix timestamp or an ISO date to compare times.")
            }
            // Keep the saved order; show local time and UTC as useful fallbacks
            // without changing the user's World Clock locations.
            var ids: [String] = []
            for identifier in WorldClockZoneCatalog.validIdentifiers(zoneIDs) + [localZone.identifier, "UTC"] {
                let id = ["GMT", "Etc/GMT", "Etc/UTC", "UTC"].contains(identifier) ? "UTC" : identifier
                if !ids.contains(id) { ids.append(id) }
            }
            let clocks = ids.prefix(3).compactMap { id -> Clock? in
                guard let zone = TimeZone(identifier: id) else { return nil }
                let time = DateFormatter(); time.locale = locale; time.timeZone = zone
                time.setLocalizedDateFormatFromTemplate("jmmss")
                let day = DateFormatter(); day.locale = locale; day.timeZone = zone
                day.setLocalizedDateFormatFromTemplate("EEE MMM d yyyy")
                let offset = zone.secondsFromGMT(for: summary.date)
                let offsetText = String(format: "UTC%@%02d:%02d", offset < 0 ? "−" : "+", abs(offset) / 3_600, abs(offset) % 3_600 / 60)
                return Clock(name: WorldClockZoneCatalog.option(for: id).name, time: time.string(from: summary.date),
                             date: day.string(from: summary.date), zone: offsetText,
                             quality: MeetingTimeQuality.at(summary.date, in: zone))
            }
            return Self(title: "Timestamp recognized", subtitle: summary.relativeTime, content: .clocks(clocks))
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
            let draft = try CurlImporter.parse(input)
            let headers = draft.headers.split(separator: "\n").count
            return Self(title: "cURL request recognized", subtitle: "Review in the editor before sending",
                content: .fields(bounded([Field(label: "Method", value: draft.method), Field(label: "URL", value: draft.url),
                    Field(label: "Headers", value: "\(headers)"), Field(label: "Body", value: "\(draft.body.utf8.count.formatted()) bytes")])))
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
        default:
            let words = input.split(whereSeparator: \.isWhitespace).count
            let lines = text.isEmpty ? 0 : text.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n").components(separatedBy: "\n").count
            return Self(title: "Text at a glance", subtitle: "Case, encoding, hashes, and line tools",
                content: .statistics([Field(label: "Characters", value: text.count.formatted()),
                    Field(label: "Words", value: words.formatted()), Field(label: "Lines", value: lines.formatted()),
                    Field(label: "UTF-8 bytes", value: text.utf8.count.formatted())]))
        }
    }

    static func failure(_ error: Error, command: LauncherCommand) -> Self {
        Self(title: "\(command.title) can help inspect this", subtitle: "Selection needs attention",
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
