import AppKit
import Foundation

struct WorldClockZonePresentation: Identifiable, Equatable {
    let id: String
    let name: String
    let dateText: String
    let timeText: String
    let zoneText: String
    let quality: MeetingTimeQuality
    let isAnchor: Bool
    let dayDifference: Int
}

@MainActor
final class WorldClockViewModel: ObservableObject {
    @Published private(set) var zoneIDs: [String]
    @Published private(set) var anchorZoneID: String
    @Published private(set) var timeline: WorldClockTimeline
    @Published private(set) var timelineQualities: [MeetingTimeQuality]
    @Published var selectedInstant: Date { didSet { copyMessage = nil } }
    @Published private(set) var isFollowingNow: Bool
    @Published private(set) var copyMessage: String?
    @Published var searchQuery = ""
    @Published var aiRequest = ""
    @Published private(set) var isResolvingAI = false
    @Published private(set) var aiMessage: String?
    @Published private(set) var aiMessageIsError = false

    private let settings: AppSettings
    private let preferences: WorldClockPreferencesStore
    private let resolveRequest: (String, AIConfig) async throws -> WorldClockAIResult
    private var aiTask: Task<Void, Never>?
    private var aiRunID: UUID?
    private var formatterCache: [String: ZoneFormatters] = [:]

    init(
        settings: AppSettings,
        seedDate: Date? = nil,
        preferences: WorldClockPreferencesStore = WorldClockPreferencesStore(),
        aiResolver: WorldClockAIResolver = WorldClockAIResolver(),
        resolveRequest: ((String, AIConfig) async throws -> WorldClockAIResult)? = nil
    ) {
        self.settings = settings
        self.preferences = preferences
        self.resolveRequest = resolveRequest ?? { request, config in
            try await aiResolver.resolve(request: request, config: config)
        }

        let zoneIDs = preferences.loadZoneIDs()
        let anchorZoneID = preferences.loadAnchorZoneID(validZoneIDs: zoneIDs)
        let anchorTimeZone = Self.validTimeZone(anchorZoneID)
        let selectedInstant = seedDate ?? Date()
        self.zoneIDs = zoneIDs
        self.anchorZoneID = anchorZoneID
        self.selectedInstant = selectedInstant
        self.isFollowingNow = seedDate == nil
        let timeline = WorldClockTimeline(containing: selectedInstant, anchorTimeZone: anchorTimeZone)
        self.timeline = timeline
        self.timelineQualities = Self.makeTimelineQualities(timeline: timeline, zoneIDs: zoneIDs)
    }

    deinit {
        aiTask?.cancel()
    }

    var anchorTimeZone: TimeZone { Self.validTimeZone(anchorZoneID) }
    var selectedOffset: TimeInterval {
        get { timeline.offset(for: selectedInstant) }
        set { isFollowingNow = false; selectedInstant = timeline.date(at: newValue) }
    }

    var timelineStep: TimeInterval { 15 * 60 }
    var canUseAI: Bool { settings.isConfigured }
    var canRemoveZone: Bool { zoneIDs.count > 1 }

    var searchResults: [WorldClockZoneOption] {
        WorldClockZoneCatalog.search(searchQuery, excluding: Set(zoneIDs))
    }

    var zonePresentations: [WorldClockZonePresentation] {
        zoneIDs.map { identifier in
            let zone = Self.validTimeZone(identifier)
            let option = WorldClockZoneCatalog.option(for: identifier)
            let formatters = formatters(for: zone)
            return WorldClockZonePresentation(
                id: identifier,
                name: option.name,
                dateText: formatters.date.string(from: selectedInstant),
                timeText: formatters.time.string(from: selectedInstant),
                zoneText: Self.zoneDescription(zone, at: selectedInstant),
                quality: MeetingTimeQuality.at(selectedInstant, in: zone),
                isAnchor: identifier == anchorZoneID,
                dayDifference: Self.dayDifference(at: selectedInstant, zone: zone, reference: anchorTimeZone)
            )
        }
    }

    var selectedMeetingQuality: MeetingTimeQuality {
        MeetingTimeQuality.combined(
            at: selectedInstant,
            timeZones: zoneIDs.map(Self.validTimeZone)
        )
    }

    var selectedTimeTitle: String {
        let formatters = formatters(for: anchorTimeZone)
        return "\(formatters.longDate.string(from: selectedInstant)) at \(formatters.time.string(from: selectedInstant))"
    }

    var anchorName: String {
        WorldClockZoneCatalog.option(for: anchorZoneID).name
    }

    var dayStartLabel: String {
        formatters(for: anchorTimeZone).shortTime.string(from: timeline.start)
    }

    var dayEndLabel: String {
        "Next day"
    }

    func focus(on date: Date) {
        isFollowingNow = false
        selectedInstant = date
        timeline = WorldClockTimeline(containing: date, anchorTimeZone: anchorTimeZone)
        refreshTimelineQualities()
    }

    func selectDay(containing date: Date) {
        isFollowingNow = false
        let newTimeline = WorldClockTimeline(containing: date, anchorTimeZone: anchorTimeZone)
        selectedInstant = Self.date(
            on: newTimeline,
            matchingLocalTimeOf: selectedInstant
        )
        timeline = newTimeline
        refreshTimelineQualities()
    }

    func moveDay(by dayDelta: Int) {
        isFollowingNow = false
        let moved = timeline.moving(byDays: dayDelta, preservingLocalTimeOf: selectedInstant)
        timeline = moved.timeline
        selectedInstant = moved.date
        refreshTimelineQualities()
    }

    func goToNow() {
        focus(on: Date())
        isFollowingNow = true
    }

    func refreshCurrentTime(_ now: Date) {
        guard isFollowingNow else { return }
        if now < timeline.start || now >= timeline.end {
            timeline = WorldClockTimeline(containing: now, anchorTimeZone: anchorTimeZone)
            refreshTimelineQualities()
        }
        if Int(now.timeIntervalSince1970 / 60) != Int(selectedInstant.timeIntervalSince1970 / 60) {
            selectedInstant = now
        }
    }

    var meetingSummary: String {
        (["Meeting time — \(selectedTimeTitle) (\(anchorName))"] + zonePresentations.map {
            "\($0.name): \($0.dateText), \($0.timeText) (\($0.zoneText))"
        }).joined(separator: "\n")
    }

    func copyMeeting() {
        NSPasteboard.general.clearContents()
        copyMessage = NSPasteboard.general.setString(meetingSummary, forType: .string)
            ? "Meeting times copied." : "Could not copy meeting times."
    }

    private static func dayDifference(at date: Date, zone: TimeZone, reference: TimeZone) -> Int {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = reference
        let referenceDay = calendar.dateComponents([.year, .month, .day], from: date)
        calendar.timeZone = zone
        let localDay = calendar.dateComponents([.year, .month, .day], from: date)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        guard let from = calendar.date(from: referenceDay), let to = calendar.date(from: localDay) else { return 0 }
        return calendar.dateComponents([.day], from: from, to: to).day ?? 0
    }

    func addZone(_ identifier: String) {
        guard TimeZone(identifier: identifier) != nil, !zoneIDs.contains(identifier) else { return }
        zoneIDs.append(identifier)
        searchQuery = ""
        refreshTimelineQualities()
        savePreferences()
    }

    func removeZone(_ identifier: String) {
        guard zoneIDs.count > 1, let index = zoneIDs.firstIndex(of: identifier) else { return }
        zoneIDs.remove(at: index)
        if anchorZoneID == identifier {
            anchorZoneID = zoneIDs[0]
            timeline = WorldClockTimeline(containing: selectedInstant, anchorTimeZone: anchorTimeZone)
        }
        refreshTimelineQualities()
        savePreferences()
    }

    func setAnchorZone(_ identifier: String) {
        guard zoneIDs.contains(identifier), identifier != anchorZoneID else { return }
        anchorZoneID = identifier
        timeline = WorldClockTimeline(containing: selectedInstant, anchorTimeZone: anchorTimeZone)
        refreshTimelineQualities()
        savePreferences()
    }

    func fillWithAI() {
        let request = aiRequest.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !request.isEmpty else {
            showAIMessage(WorldClockAIError.emptyRequest.localizedDescription, isError: true)
            return
        }
        guard canUseAI else {
            showAIMessage("Configure an AI provider in Settings first.", isError: true)
            return
        }

        aiTask?.cancel()
        let runID = UUID()
        aiRunID = runID
        isResolvingAI = true
        aiMessage = nil
        let config = settings.currentConfig
        let resolve = resolveRequest
        aiTask = Task { [weak self] in
            do {
                let result = try await resolve(request, config)
                try Task.checkCancellation()
                guard self?.aiRunID == runID else { return }
                self?.applyAIResult(result)
            } catch is CancellationError {
                // A newer request or window close superseded this result.
            } catch {
                guard self?.aiRunID == runID else { return }
                self?.showAIMessage(error.localizedDescription, isError: true)
            }
            guard self?.aiRunID == runID else { return }
            self?.aiRunID = nil
            self?.isResolvingAI = false
            self?.aiTask = nil
        }
    }

    func cancelAI() {
        aiRunID = nil
        if isResolvingAI { showAIMessage("Cancelled. Your locations and time were kept.", isError: false) }
        aiTask?.cancel()
        aiTask = nil
        isResolvingAI = false
    }

    private func applyAIResult(_ result: WorldClockAIResult) {
        isFollowingNow = false
        zoneIDs = result.timeZoneIDs
        anchorZoneID = result.anchorTimeZoneID
        if let date = result.referenceDate {
            selectedInstant = date
        }
        timeline = WorldClockTimeline(containing: selectedInstant, anchorTimeZone: anchorTimeZone)
        formatterCache.removeAll(keepingCapacity: true)
        refreshTimelineQualities()
        savePreferences()
        showAIMessage("Updated \(zoneIDs.count) locations.", isError: false)
    }

    private func showAIMessage(_ message: String, isError: Bool) {
        aiMessage = message
        aiMessageIsError = isError
    }

    private func savePreferences() {
        copyMessage = nil
        preferences.save(zoneIDs: zoneIDs, anchorZoneID: anchorZoneID)
    }

    private func refreshTimelineQualities() {
        timelineQualities = Self.makeTimelineQualities(timeline: timeline, zoneIDs: zoneIDs)
    }

    private func formatters(for timeZone: TimeZone) -> ZoneFormatters {
        if let cached = formatterCache[timeZone.identifier] { return cached }
        let created = ZoneFormatters(timeZone: timeZone)
        formatterCache[timeZone.identifier] = created
        return created
    }

    private static func validTimeZone(_ identifier: String) -> TimeZone {
        guard let timeZone = TimeZone(identifier: identifier) else {
            preconditionFailure("World-clock identifiers are validated before storage: \(identifier)")
        }
        return timeZone
    }

    private static func date(on timeline: WorldClockTimeline, matchingLocalTimeOf source: Date) -> Date {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = timeline.anchorTimeZone
        let components = calendar.dateComponents([.hour, .minute, .second], from: source)
        var matching = DateComponents()
        matching.hour = components.hour
        matching.minute = components.minute
        matching.second = components.second
        let searchStart = timeline.start.addingTimeInterval(-1)
        let date = calendar.nextDate(
            after: searchStart,
            matching: matching,
            matchingPolicy: .nextTime,
            repeatedTimePolicy: .first,
            direction: .forward
        ) ?? timeline.start
        return min(date, timeline.end)
    }

    private static func zoneDescription(_ timeZone: TimeZone, at date: Date) -> String {
        let seconds = timeZone.secondsFromGMT(for: date)
        let sign = seconds < 0 ? "-" : "+"
        let magnitude = abs(seconds)
        let hours = magnitude / 3_600
        let minutes = (magnitude % 3_600) / 60
        let offset = String(format: "UTC%@%02d:%02d", sign, hours, minutes)
        if let abbreviation = timeZone.abbreviation(for: date), !abbreviation.hasPrefix("GMT") {
            return "\(abbreviation) - \(offset)"
        }
        return offset
    }

    private static func makeTimelineQualities(
        timeline: WorldClockTimeline,
        zoneIDs: [String]
    ) -> [MeetingTimeQuality] {
        let step: TimeInterval = 15 * 60
        let intervalCount = max(1, Int(ceil(timeline.duration / step)))
        let zones = zoneIDs.map(validTimeZone)
        return (0..<intervalCount).map { index in
            let midpoint = min(
                (Double(index) + 0.5) * step,
                max(0, timeline.duration - 1)
            )
            return MeetingTimeQuality.combined(at: timeline.date(at: midpoint), timeZones: zones)
        }
    }
}

private final class ZoneFormatters {
    let date: DateFormatter
    let longDate: DateFormatter
    let time: DateFormatter
    let shortTime: DateFormatter

    init(timeZone: TimeZone, locale: Locale = .current) {
        date = DateFormatter()
        date.locale = locale
        date.timeZone = timeZone
        date.setLocalizedDateFormatFromTemplate("EEE MMM d")

        longDate = DateFormatter()
        longDate.locale = locale
        longDate.timeZone = timeZone
        longDate.dateStyle = .full
        longDate.timeStyle = .none

        time = DateFormatter()
        time.locale = locale
        time.timeZone = timeZone
        time.dateStyle = .none
        time.timeStyle = .short

        shortTime = DateFormatter()
        shortTime.locale = locale
        shortTime.timeZone = timeZone
        shortTime.setLocalizedDateFormatFromTemplate("jm")
    }
}
