import Foundation

enum MeetingTimeQuality: Int, CaseIterable, Comparable {
    case poor
    case extended
    case working

    static func < (lhs: MeetingTimeQuality, rhs: MeetingTimeQuality) -> Bool {
        lhs.rawValue < rhs.rawValue
    }

    var label: String {
        switch self {
        case .working: return "Working hours"
        case .extended: return "Outside core hours"
        case .poor: return "Late night / early morning"
        }
    }

    static func at(_ date: Date, in timeZone: TimeZone) -> MeetingTimeQuality {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = timeZone
        let components = calendar.dateComponents([.hour, .minute], from: date)
        let minuteOfDay = (components.hour ?? 0) * 60 + (components.minute ?? 0)

        switch minuteOfDay {
        case (9 * 60)..<(17 * 60):
            return .working
        case (7 * 60)..<(9 * 60), (17 * 60)..<(21 * 60):
            return .extended
        default:
            return .poor
        }
    }

    static func combined(at date: Date, timeZones: [TimeZone]) -> MeetingTimeQuality {
        guard !timeZones.isEmpty else { return .poor }
        let qualities = timeZones.map { at(date, in: $0) }
        if qualities.allSatisfy({ $0 == .working }) { return .working }
        if qualities.contains(.poor) { return .poor }
        return .extended
    }
}

struct WorldClockTimeline: Equatable {
    let anchorTimeZone: TimeZone
    let start: Date
    let end: Date

    init(containing date: Date, anchorTimeZone: TimeZone) {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = anchorTimeZone
        let start = calendar.startOfDay(for: date)
        guard let end = calendar.date(byAdding: .day, value: 1, to: start) else {
            preconditionFailure("A Gregorian calendar day must have a successor")
        }

        self.anchorTimeZone = anchorTimeZone
        self.start = start
        self.end = end
    }

    var duration: TimeInterval { end.timeIntervalSince(start) }

    func date(at offset: TimeInterval) -> Date {
        start.addingTimeInterval(min(max(offset, 0), duration))
    }

    func offset(for date: Date) -> TimeInterval {
        min(max(date.timeIntervalSince(start), 0), duration)
    }

    func moving(byDays dayDelta: Int, preservingLocalTimeOf date: Date) -> (timeline: WorldClockTimeline, date: Date) {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = anchorTimeZone
        guard let targetDay = calendar.date(byAdding: .day, value: dayDelta, to: start) else {
            preconditionFailure("A Gregorian calendar day must be movable")
        }

        let targetTimeline = WorldClockTimeline(containing: targetDay, anchorTimeZone: anchorTimeZone)
        let time = calendar.dateComponents([.hour, .minute, .second], from: date)
        let matching = DateComponents(hour: time.hour, minute: time.minute, second: time.second)
        let searchStart = targetTimeline.start.addingTimeInterval(-1)
        let targetDate = calendar.nextDate(
            after: searchStart,
            matching: matching,
            matchingPolicy: .nextTime,
            repeatedTimePolicy: .first,
            direction: .forward
        ) ?? targetTimeline.start

        return (targetTimeline, min(targetDate, targetTimeline.end))
    }
}

struct WorldClockZoneOption: Identifiable, Equatable {
    let id: String
    let name: String
    let region: String
    fileprivate let searchTerms: String

    var subtitle: String {
        region.isEmpty ? id : "\(region) - \(id)"
    }
}

enum WorldClockZoneCatalog {
    private static let suggestedIDs = [
        "UTC",
        "America/Los_Angeles",
        "America/New_York",
        "Europe/London",
        "Europe/Berlin",
        "Asia/Singapore",
        "Asia/Tokyo",
        "Asia/Kolkata",
        "Australia/Sydney",
    ]

    private static let preferredNames: [String: String] = [
        "America/Chicago": "Chicago",
        "America/Denver": "Denver",
        "America/Los_Angeles": "Los Angeles",
        "America/New_York": "New York",
        "America/Sao_Paulo": "Sao Paulo",
        "America/Toronto": "Toronto",
        "America/Vancouver": "Vancouver",
        "Asia/Dubai": "Dubai",
        "Asia/Hong_Kong": "Hong Kong",
        "Asia/Kolkata": "India",
        "Asia/Seoul": "Seoul",
        "Asia/Shanghai": "Shanghai",
        "Asia/Singapore": "Singapore",
        "Asia/Tokyo": "Tokyo",
        "Australia/Sydney": "Sydney",
        "Europe/Berlin": "Berlin",
        "Europe/London": "London",
        "Europe/Paris": "Paris",
        "Pacific/Auckland": "Auckland",
        "UTC": "UTC",
    ]

    private static let aliases: [String: String] = [
        "America/Los_Angeles": "san francisco bay area pacific time pst pdt",
        "America/New_York": "eastern time est edt",
        "America/Chicago": "central time cst cdt",
        "America/Denver": "mountain time mst mdt",
        "Asia/Kolkata": "bengaluru bangalore mumbai delhi india ist",
        "Asia/Shanghai": "beijing china cst",
        "Europe/London": "uk britain england gmt bst",
        "UTC": "gmt universal coordinated",
    ]

    static let all: [WorldClockZoneOption] = {
        var identifiers = TimeZone.knownTimeZoneIdentifiers
        if TimeZone(identifier: "UTC") != nil, !identifiers.contains("UTC") {
            identifiers.append("UTC")
        }

        return identifiers.map(option(for:)).sorted {
            $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending
        }
    }()

    static func option(for identifier: String) -> WorldClockZoneOption {
        let components = identifier.split(separator: "/").map(String.init)
        let fallbackName = (components.last ?? identifier).replacingOccurrences(of: "_", with: " ")
        let region = components.dropLast().joined(separator: " / ").replacingOccurrences(of: "_", with: " ")
        let name = preferredNames[identifier] ?? fallbackName
        let searchTerms = [name, region, identifier, aliases[identifier] ?? ""]
            .joined(separator: " ")
            .lowercased()
        return WorldClockZoneOption(id: identifier, name: name, region: region, searchTerms: searchTerms)
    }

    static func search(_ query: String, excluding excludedIDs: Set<String>, limit: Int = 10) -> [WorldClockZoneOption] {
        let words = query
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .split(whereSeparator: { $0.isWhitespace })
            .map(String.init)

        if words.isEmpty {
            let optionsByID = Dictionary(uniqueKeysWithValues: all.map { ($0.id, $0) })
            return suggestedIDs
                .compactMap { optionsByID[$0] }
                .filter { !excludedIDs.contains($0.id) }
                .prefix(max(0, limit))
                .map { $0 }
        }

        let matches = all.lazy.filter { option in
            !excludedIDs.contains(option.id)
                && words.allSatisfy { option.searchTerms.contains($0) }
        }
        return Array(matches.prefix(max(0, limit)))
    }

    static func validIdentifiers(_ identifiers: [String]) -> [String] {
        var seen = Set<String>()
        return identifiers.filter { identifier in
            TimeZone(identifier: identifier) != nil && seen.insert(identifier).inserted
        }
    }

    static func defaultIdentifiers(current: TimeZone = .current) -> [String] {
        let candidates = [current.identifier, "UTC"]
        let valid = validIdentifiers(candidates)
        return valid.isEmpty ? ["UTC"] : valid
    }
}

struct WorldClockPreferencesStore {
    private enum Keys {
        static let zoneIDs = "worldClock.zoneIDs"
        static let anchorZoneID = "worldClock.anchorZoneID"
    }

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func loadZoneIDs(current: TimeZone = .current) -> [String] {
        let stored = defaults.stringArray(forKey: Keys.zoneIDs) ?? []
        let valid = WorldClockZoneCatalog.validIdentifiers(stored)
        return valid.isEmpty ? WorldClockZoneCatalog.defaultIdentifiers(current: current) : valid
    }

    func loadAnchorZoneID(validZoneIDs: [String]) -> String {
        if let stored = defaults.string(forKey: Keys.anchorZoneID), validZoneIDs.contains(stored) {
            return stored
        }
        return validZoneIDs.first ?? "UTC"
    }

    func save(zoneIDs: [String], anchorZoneID: String) {
        defaults.set(zoneIDs, forKey: Keys.zoneIDs)
        defaults.set(anchorZoneID, forKey: Keys.anchorZoneID)
    }
}
