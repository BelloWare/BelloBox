import Foundation

struct TimestampSummary: Equatable {
    let date: Date
    let relativeTime: String
    let localDateTime: String

    static func make(
        from selectedText: String,
        relativeTo now: Date = Date(),
        locale: Locale = .current,
        timeZone: TimeZone = .current
    ) -> TimestampSummary? {
        guard let date = TimestampParser.parse(selectedText, localTimeZone: timeZone) else { return nil }

        let relativeFormatter = RelativeDateTimeFormatter()
        relativeFormatter.locale = locale
        relativeFormatter.dateTimeStyle = .numeric
        relativeFormatter.unitsStyle = .full

        let localFormatter = DateFormatter()
        localFormatter.locale = locale
        localFormatter.timeZone = timeZone
        localFormatter.dateStyle = .medium
        localFormatter.timeStyle = .medium

        let zone = timeZone.abbreviation(for: date) ?? timeZone.identifier
        return TimestampSummary(
            date: date,
            relativeTime: relativeFormatter.localizedString(for: date, relativeTo: now),
            localDateTime: "\(localFormatter.string(from: date)) \(zone)"
        )
    }
}

private enum TimestampParser {
    private static let utc = TimeZone(secondsFromGMT: 0)!
    private static let zonedISO8601Pattern =
        #"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?(?:Z|[+-][0-9]{2}(?::?[0-9]{2})?)$"#
    private static let localISO8601Pattern =
        #"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?$"#
    private static let rfc2822Pattern =
        #"^[A-Za-z]{3}, [0-9]{1,2} [A-Za-z]{3} [0-9]{4} [0-9]{2}:[0-9]{2}:[0-9]{2} (?:[+-][0-9]{4}|GMT|UTC)$"#
    private static let internetDatePattern =
        #"^[0-9]{1,2} [A-Za-z]{3} [0-9]{4} [0-9]{2}:[0-9]{2}:[0-9]{2} (?:[+-][0-9]{4}|GMT|UTC)$"#
    private static let javaScriptDatePattern =
        #"^[A-Za-z]{3} [A-Za-z]{3} [0-9]{2} [0-9]{4} [0-9]{2}:[0-9]{2}:[0-9]{2} GMT[+-][0-9]{4}(?: \([^()\r\n]+\))?$"#

    static func parse(_ selectedText: String, localTimeZone: TimeZone) -> Date? {
        let trimmed = selectedText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let value = removingMatchingQuotes(from: trimmed)

        return parseUnixTimestamp(value)
            ?? parseISO8601Timestamp(value, localTimeZone: localTimeZone)
            ?? parseRFC2822Timestamp(value)
            ?? parseJavaScriptDate(value)
    }

    private static func parseUnixTimestamp(_ value: String) -> Date? {
        if isASCIIDigits(value) {
            let divisor: Double
            switch value.count {
            case 10:
                divisor = 1
            case 13:
                divisor = 1_000
            case 16:
                divisor = 1_000_000
            case 19:
                divisor = 1_000_000_000
            default:
                return nil
            }

            guard let rawValue = UInt64(value) else { return nil }
            return Date(timeIntervalSince1970: Double(rawValue) / divisor)
        }

        let components = value.split(separator: ".", omittingEmptySubsequences: false)
        guard components.count == 2 else { return nil }
        let wholeSeconds = String(components[0])
        let fractionalSeconds = String(components[1])
        guard wholeSeconds.count == 10 else { return nil }
        guard (1...9).contains(fractionalSeconds.count) else { return nil }
        guard isASCIIDigits(wholeSeconds), isASCIIDigits(fractionalSeconds) else { return nil }
        guard let seconds = Double(value), seconds.isFinite else { return nil }
        return Date(timeIntervalSince1970: seconds)
    }

    private static func parseISO8601Timestamp(_ value: String, localTimeZone: TimeZone) -> Date? {
        var normalized = value
        normalizeDateTimeSeparator(in: &normalized)
        normalizeUTCName(in: &normalized)
        removeSpaceBeforeNumericOffset(in: &normalized)
        if normalized.last == "z" {
            normalized.replaceSubrange(normalized.index(before: normalized.endIndex)..., with: "Z")
        }

        if matches(zonedISO8601Pattern, value: normalized) {
            return formatter(
                format: normalized.contains(".")
                    ? "yyyy-MM-dd'T'HH:mm:ss.SSSSSSSSSXXXXX"
                    : "yyyy-MM-dd'T'HH:mm:ssXXXXX",
                timeZone: utc
            ).date(from: normalized)
        }

        guard matches(localISO8601Pattern, value: normalized) else { return nil }
        return formatter(
            format: normalized.contains(".")
                ? "yyyy-MM-dd'T'HH:mm:ss.SSSSSSSSS"
                : "yyyy-MM-dd'T'HH:mm:ss",
            timeZone: localTimeZone
        ).date(from: normalized)
    }

    private static func parseRFC2822Timestamp(_ value: String) -> Date? {
        if matches(rfc2822Pattern, value: value) {
            return formatter(
                format: "EEE, dd MMM yyyy HH:mm:ss Z",
                timeZone: utc
            ).date(from: value)
        }

        guard matches(internetDatePattern, value: value) else { return nil }
        return formatter(
            format: "dd MMM yyyy HH:mm:ss Z",
            timeZone: utc
        ).date(from: value)
    }

    private static func parseJavaScriptDate(_ value: String) -> Date? {
        guard matches(javaScriptDatePattern, value: value) else { return nil }
        let timestamp: String
        if let zoneName = value.range(of: " (", options: .backwards) {
            timestamp = String(value[..<zoneName.lowerBound])
        } else {
            timestamp = value
        }
        return formatter(
            format: "EEE MMM dd yyyy HH:mm:ss 'GMT'Z",
            timeZone: utc
        ).date(from: timestamp)
    }

    private static func removingMatchingQuotes(from value: String) -> String {
        guard value.count >= 2, let first = value.first, first == value.last else { return value }
        guard first == "\"" || first == "'" || first == "`" else { return value }
        return String(value.dropFirst().dropLast())
    }

    private static func normalizeDateTimeSeparator(in value: inout String) {
        guard value.count > 10 else { return }
        let separator = value.index(value.startIndex, offsetBy: 10)
        guard value[separator] == " " || value[separator] == "t" else { return }
        value.replaceSubrange(separator...separator, with: "T")
    }

    private static func normalizeUTCName(in value: inout String) {
        let lowercased = value.lowercased()
        guard lowercased.hasSuffix(" utc") || lowercased.hasSuffix(" gmt") else { return }
        value.removeLast(4)
        value.append("Z")
    }

    private static func removeSpaceBeforeNumericOffset(in value: inout String) {
        guard let offset = value.range(
            of: #" [+-][0-9]{2}(?::?[0-9]{2})?$"#,
            options: .regularExpression
        ) else { return }
        value.remove(at: offset.lowerBound)
    }

    private static func formatter(format: String, timeZone: TimeZone) -> DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.timeZone = timeZone
        formatter.isLenient = false
        formatter.dateFormat = format
        return formatter
    }

    private static func matches(_ pattern: String, value: String) -> Bool {
        guard let range = value.range(of: pattern, options: .regularExpression) else { return false }
        return range.lowerBound == value.startIndex && range.upperBound == value.endIndex
    }

    private static func isASCIIDigits(_ value: String) -> Bool {
        !value.isEmpty && value.utf8.allSatisfy { $0 >= 48 && $0 <= 57 }
    }
}
