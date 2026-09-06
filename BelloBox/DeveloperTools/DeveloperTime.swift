import Foundation

enum EpochUnit: String, CaseIterable { case automatic = "Auto", seconds = "Seconds", milliseconds = "Milliseconds" }
enum DeveloperTime {
    static func date(_ text: String, unit: EpochUnit = .automatic, zone: TimeZone = .current) throws -> Date {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if let number = Double(text), number.isFinite {
            let divisor: Double = unit == .milliseconds || (unit == .automatic && abs(number) >= 100_000_000_000) ? 1_000 : 1
            let seconds = number / divisor
            guard (-62_135_596_800...253_402_300_799).contains(seconds) else { throw UtilityError("Choose the correct timestamp unit. Supported dates are years 1–9999.") }
            return Date(timeIntervalSince1970: seconds)
        }
        if let date = TimestampSummary.make(from: text, timeZone: zone)?.date { return date }
        throw UtilityError("Enter a Unix timestamp or a date such as 2026-09-06T09:30:00Z.")
    }
    static func summary(_ text: String, unit: EpochUnit, zone: TimeZone, comparedWith other: String = "") throws -> String {
        let date = try date(text, unit: unit, zone: zone)
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let local = DateFormatter()
        local.locale = Locale(identifier: "en_US_POSIX")
        local.timeZone = zone
        local.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS XXX"
        var rows = ["UTC: \(iso.string(from: date))", "\(zone.identifier): \(local.string(from: date))", "Unix seconds: \(String(format: "%.3f", locale: Locale(identifier: "en_US_POSIX"), date.timeIntervalSince1970))", "Unix milliseconds: \(Int64((date.timeIntervalSince1970 * 1_000).rounded()))"]
        if !other.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            let delta = try Self.date(other, unit: unit, zone: zone).timeIntervalSince(date)
            rows += ["", "Difference (second − first): \(String(format: "%.3f", delta)) seconds", "\(String(format: "%.3f", delta / 3_600)) hours · \(String(format: "%.3f", delta / 86_400)) days"]
        }
        return rows.joined(separator: "\n")
    }
}

struct CronSchedule {
    struct Field {
        let values: Set<Int>
        let wildcard: Bool
        let source: String
    }
    let fields: [Field]
    let expression: String
    init(_ expression: String) throws {
        let aliases = ["@yearly": "0 0 1 1 *", "@annually": "0 0 1 1 *", "@monthly": "0 0 1 * *", "@weekly": "0 0 * * 0", "@daily": "0 0 * * *", "@hourly": "0 * * * *"]
        let expanded = aliases[expression.lowercased().trimmingCharacters(in: .whitespaces)] ?? expression
        let parts = expanded.uppercased().split(whereSeparator: \.isWhitespace).map(String.init)
        guard parts.count == 5 else { throw UtilityError("Use standard 5-field cron: minute hour day-of-month month day-of-week. Seconds and Quartz extensions are not supported.") }
        self.expression = parts.joined(separator: " ")
        let bounds = [0...59, 0...23, 1...31, 1...12, 0...7]
        let months = Dictionary(uniqueKeysWithValues: ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"].enumerated().map { ($0.element, $0.offset + 1) })
        let days = Dictionary(uniqueKeysWithValues: ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"].enumerated().map { ($0.element, $0.offset) })
        fields = try parts.enumerated().map { index, source in
            let range = bounds[index], names = index == 3 ? months : index == 4 ? days : [:]
            func number(_ text: String) throws -> Int {
                guard let n = Int(text) ?? names[text], range.contains(n) else { throw UtilityError("Invalid cron field \(index + 1): \(source)") }
                return n
            }
            var values = Set<Int>()
            for chunk in source.split(separator: ",", omittingEmptySubsequences: false) {
                let stepped = chunk.split(separator: "/", omittingEmptySubsequences: false).map(String.init)
                guard (1...2).contains(stepped.count), !stepped[0].isEmpty else { throw UtilityError("Invalid cron list or step.") }
                let step: Int
                if stepped.count == 2 {
                    guard let n = Int(stepped[1]), n > 0, n <= range.count else { throw UtilityError("Cron steps must be positive and within the field range.") }
                    step = n
                } else { step = 1 }
                let span = stepped[0].split(separator: "-", omittingEmptySubsequences: false).map(String.init)
                let start: Int, end: Int
                if stepped[0] == "*" { start = range.lowerBound; end = range.upperBound }
                else if span.count == 2 { start = try number(span[0]); end = try number(span[1]) }
                else if span.count == 1 { start = try number(span[0]); end = stepped.count == 2 ? range.upperBound : start }
                else { throw UtilityError("Invalid cron range.") }
                guard start <= end else { throw UtilityError("Cron ranges must run from lower to higher values.") }
                for n in stride(from: start, through: end, by: step) { values.insert(index == 4 && n == 7 ? 0 : n) }
            }
            return Field(values: values, wildcard: source.hasPrefix("*"), source: source)
        }
    }

    var explanation: String {
        let names = ["Minutes", "Hours (24-hour)", "Days of month", "Months", "Days of week (0 = Sunday)"]
        return zip(names, fields).map { name, field in
            name + ": " + (field.source == "*" ? "every value" : field.source.hasPrefix("*/") ? "every \(field.source.dropFirst(2))" : field.source)
        }.joined(separator: "\n") + (!fields[2].wildcard && !fields[4].wildcard ? "\nRuns when either day-of-month or day-of-week matches." : "")
    }

    func next(after now: Date, zone: TimeZone, count: Int = 5) throws -> [Date] {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = zone
        var day = calendar.startOfDay(for: now)
        var output: [Date] = []
        for _ in 0..<(366 * 8) {
            try Task.checkCancellation()
            let parts = calendar.dateComponents([.year, .month, .day, .weekday], from: day)
            let dom = fields[2].values.contains(parts.day!), dow = fields[4].values.contains(parts.weekday! - 1)
            let matchesDay = fields[2].wildcard || fields[4].wildcard ? dom && dow : dom || dow
            if fields[3].values.contains(parts.month!) && matchesDay {
                var candidates = Set<Date>()
                for hour in fields[1].values.sorted() {
                    for minute in fields[0].values.sorted() {
                        var match = parts
                        match.weekday = nil; match.hour = hour; match.minute = minute; match.second = 0
                        for policy in [Calendar.RepeatedTimePolicy.first, .last] {
                            if let date = calendar.nextDate(after: day.addingTimeInterval(-1), matching: match, matchingPolicy: .strict, repeatedTimePolicy: policy),
                               date > now, calendar.isDate(date, inSameDayAs: day) { candidates.insert(date) }
                        }
                    }
                }
                output.append(contentsOf: candidates.sorted())
                if output.count >= count { return Array(output.prefix(count)) }
            }
            guard let next = calendar.date(byAdding: .day, value: 1, to: day) else { break }
            day = next
        }
        return output
    }
}
