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
        let timestamp = selectedText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard timestamp.count == 10 || timestamp.count == 13 else { return nil }
        guard timestamp.utf8.allSatisfy({ $0 >= 48 && $0 <= 57 }) else { return nil }
        guard let rawValue = UInt64(timestamp) else { return nil }

        let seconds = timestamp.count == 13 ? Double(rawValue) / 1_000 : Double(rawValue)
        let date = Date(timeIntervalSince1970: seconds)

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
