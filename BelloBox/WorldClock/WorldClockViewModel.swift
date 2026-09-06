import AppKit
import Combine
import Foundation

struct WorldClockZonePresentation: Identifiable, Equatable {
    let id: String
    let name: String
    let dateText: String
    let timeText: String
    let zoneText: String
    /// Short zone text for narrow cards, e.g. "EDT · UTC−4" or "UTC+5:30".
    let compactZoneText: String
    let quality: MeetingTimeQuality
    let isAnchor: Bool
    let dayDifference: Int
}

@MainActor
final class WorldClockViewModel: ObservableObject {
    /// The dedicated window persists locations; the palette preview keeps
    /// everything in memory and never edits saved locations.
    enum Mode: Equatable {
        case planner
        case preview
    }

    @Published private(set) var zoneIDs: [String]
    @Published private(set) var anchorZoneID: String
    @Published private(set) var timeline: WorldClockTimeline
    @Published private(set) var timelineQualities: [MeetingTimeQuality]
    @Published var selectedInstant: Date { didSet { copyMessage = nil } }
    @Published private(set) var isFollowingNow: Bool
    @Published private(set) var copyMessage: String?
    @Published var searchQuery = ""
    /// Incremented when a handoff brings a conversation that the window should
    /// reveal; the view observes it to open the copilot panel.
    @Published private(set) var copilotRevealRequest = 0

    let mode: Mode
    let copilot: WorldClockCopilotSession
    /// The instant the planner opened with, when it was seeded from a selection.
    private(set) var seedInstant: Date?

    private let settings: AppSettings
    private let preferences: WorldClockPreferencesStore
    private let localTimeZone: TimeZone
    private var formatterCache: [String: ZoneFormatters] = [:]
    private var settingsObserver: AnyCancellable?

    init(
        settings: AppSettings,
        seedDate: Date? = nil,
        preferences: WorldClockPreferencesStore = WorldClockPreferencesStore(),
        mode: Mode = .planner,
        zoneIDs: [String]? = nil,
        anchorZoneID: String? = nil,
        localTimeZone: TimeZone = .current,
        aiResolver: WorldClockAIResolver = WorldClockAIResolver(),
        askCopilot: WorldClockCopilotSession.Responder? = nil
    ) {
        self.settings = settings
        self.preferences = preferences
        self.mode = mode
        self.localTimeZone = localTimeZone
        self.seedInstant = seedDate

        let storedZones = preferences.loadZoneIDs(current: localTimeZone)
        let validOverride = zoneIDs.map(WorldClockZoneCatalog.validIdentifiers)
        let resolvedZones = (validOverride?.isEmpty == false) ? validOverride! : storedZones
        let storedAnchor = preferences.loadAnchorZoneID(validZoneIDs: resolvedZones)
        let resolvedAnchor = anchorZoneID.flatMap { resolvedZones.contains($0) ? $0 : nil } ?? storedAnchor
        let anchorTimeZone = Self.validTimeZone(resolvedAnchor)
        let selectedInstant = seedDate ?? Date()
        self.zoneIDs = resolvedZones
        self.anchorZoneID = resolvedAnchor
        self.selectedInstant = selectedInstant
        self.isFollowingNow = seedDate == nil
        let timeline = WorldClockTimeline(containing: selectedInstant, anchorTimeZone: anchorTimeZone)
        self.timeline = timeline
        self.timelineQualities = Self.makeTimelineQualities(timeline: timeline, zoneIDs: resolvedZones)

        var responder = askCopilot ?? { request, config in try await aiResolver.ask(request, config: config) }
        var usesOfflineResponder = false
#if DEBUG
        if askCopilot == nil, let fixture = WorldClockCopilotFixture.responder() {
            responder = fixture
            usesOfflineResponder = true
        }
#endif
        copilot = WorldClockCopilotSession(responder: responder)
        copilot.contextProvider = { [weak self] in self?.copilotContext }
        copilot.configProvider = { [weak self] in self?.settings.currentConfig }
        copilot.isProviderConfigured = { [weak self] in
            guard let self else { return false }
            return usesOfflineResponder || self.settings.isConfigured
        }
        // Provider changes made in Settings must reach an open copilot at once.
        settingsObserver = settings.objectWillChange.sink { [weak self] _ in
            guard let self else { return }
            self.copilot.objectWillChange.send()
            self.objectWillChange.send()
        }
    }

    var anchorTimeZone: TimeZone { Self.validTimeZone(anchorZoneID) }
    var selectedOffset: TimeInterval {
        get { timeline.offset(for: selectedInstant) }
        set { isFollowingNow = false; selectedInstant = timeline.date(at: newValue) }
    }

    var timelineStep: TimeInterval { 15 * 60 }
    var canUseAI: Bool { copilot.canUseAI }
    var canRemoveZone: Bool { zoneIDs.count > 1 }
    var persistsPreferences: Bool { mode == .planner }
    /// Whether the previewed instant differs from the seed by a minute or more.
    var hasMovedFromSeed: Bool {
        guard let seedInstant else { return false }
        return abs(selectedInstant.timeIntervalSince(seedInstant)) >= 60
    }

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
                compactZoneText: Self.compactZoneDescription(zone, at: selectedInstant),
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

    /// Compact "Tue, Sep 8 · 8:00 AM" in the reference zone for tight layouts.
    var selectedTimeCompactTitle: String {
        let formatters = formatters(for: anchorTimeZone)
        return "\(formatters.date.string(from: selectedInstant)) · \(formatters.time.string(from: selectedInstant))"
    }

    func compactDescription(of instant: Date) -> String {
        let formatters = formatters(for: anchorTimeZone)
        return "\(formatters.date.string(from: instant)) \(formatters.time.string(from: instant)) (\(anchorName))"
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

    /// Spoken summary of the current comparison for assistive technology.
    var accessibilitySummary: String {
        ([selectedTimeTitle + " in \(anchorName)."] + zonePresentations.map {
            "\($0.name): \($0.timeText), \($0.dateText), \($0.zoneText), \($0.quality.label)"
        }).joined(separator: " ")
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

    /// Nudges the instant by whole steps, rolling the day over at the edges of
    /// the timeline instead of clamping.
    func nudge(by steps: Int, step: TimeInterval? = nil) {
        let step = step ?? timelineStep
        let target = selectedInstant.addingTimeInterval(Double(steps) * step)
        if target >= timeline.start && target < timeline.end {
            isFollowingNow = false
            selectedInstant = target
        } else {
            focus(on: target)
        }
    }

    /// Returns to the instant the planner opened with.
    func returnToSeed() {
        guard let seedInstant else { return }
        focus(on: seedInstant)
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

    // MARK: - Copilot

    var copilotContext: WorldClockCopilotContext {
        WorldClockCopilotContext(
            selectedInstant: selectedInstant,
            referenceZoneID: anchorZoneID,
            locations: zonePresentations.map {
                WorldClockCopilotContext.Location(
                    zoneID: $0.id, name: $0.name,
                    localDescription: "\($0.dateText), \($0.timeText) (\($0.zoneText))",
                    quality: $0.quality, isReference: $0.isAnchor
                )
            },
            now: Date(),
            localZoneID: localTimeZone.identifier,
            isFollowingNow: isFollowingNow
        )
    }

    /// What applying a message's suggestion would change right now, or nil when
    /// it is already in effect. The palette preview only ever moves the time.
    func copilotPlan(for message: WorldClockCopilotSession.Message) -> WorldClockCopilotPlan? {
        guard let suggestion = message.suggestion else { return nil }
        return suggestion.plan(
            currentZoneIDs: zoneIDs,
            currentAnchorZoneID: anchorZoneID,
            currentInstant: selectedInstant,
            allowsLocationChanges: mode == .planner,
            describeInstant: compactDescription(of:)
        )
    }

    /// The location and reference parts of a suggestion that the palette
    /// preview cannot apply itself; the dedicated window applies them after
    /// Enter. Always nil in the window.
    func deferredCopilotPlan(for message: WorldClockCopilotSession.Message) -> WorldClockCopilotPlan? {
        guard mode == .preview, let suggestion = message.suggestion else { return nil }
        return suggestion.plan(
            currentZoneIDs: zoneIDs,
            currentAnchorZoneID: anchorZoneID,
            currentInstant: selectedInstant,
            allowsTimeChanges: false,
            allowsLocationChanges: true,
            describeInstant: compactDescription(of:)
        )
    }

    /// Adopts what the palette or a selection handed over: the previewed
    /// instant, the chosen reference, and the copilot conversation. Nothing is
    /// saved; a reference outside the saved list joins the planner in memory
    /// only. A handoff with an instant is a new context, so any earlier
    /// conversation in this window is dropped before the snapshot is restored.
    func adopt(_ handoff: WorldClockHandoff) {
        if let instant = handoff.instant {
            seedInstant = instant
            focus(on: instant)
        }
        if let anchor = handoff.anchorZoneID, TimeZone(identifier: anchor) != nil, anchor != anchorZoneID {
            if !zoneIDs.contains(anchor) { zoneIDs.append(anchor) }
            anchorZoneID = anchor
            timeline = WorldClockTimeline(containing: selectedInstant, anchorTimeZone: anchorTimeZone)
            refreshTimelineQualities()
        }
        if handoff.instant != nil || handoff.copilot != nil {
            copilot.reset()
        }
        if let snapshot = handoff.copilot, !snapshot.isEmpty {
            copilot.restore(snapshot)
            copilotRevealRequest += 1
        }
    }

    /// Applies a plan after the user chose to. Locations are validated again
    /// here, so a stale plan can never introduce an unknown zone.
    func applyCopilotPlan(_ plan: WorldClockCopilotPlan, from message: WorldClockCopilotSession.Message) {
        var changed = false
        if mode == .planner, let zoneIDs = plan.zoneIDs {
            let valid = WorldClockZoneCatalog.validIdentifiers(zoneIDs)
            if !valid.isEmpty, valid != self.zoneIDs {
                self.zoneIDs = valid
                changed = true
            }
            if !self.zoneIDs.contains(anchorZoneID) { anchorZoneID = self.zoneIDs[0]; changed = true }
        }
        if mode == .planner, let anchor = plan.anchorZoneID, zoneIDs.contains(anchor), anchor != anchorZoneID {
            anchorZoneID = anchor
            changed = true
        }
        if let instant = plan.instant {
            focus(on: instant)
        } else {
            timeline = WorldClockTimeline(containing: selectedInstant, anchorTimeZone: anchorTimeZone)
            refreshTimelineQualities()
        }
        if changed { savePreferences() }
        copilot.markApplied(message.id, parts: plan.parts, summary: plan.summary)
    }

    func cancelAI() {
        copilot.cancel()
    }

    private func savePreferences() {
        copyMessage = nil
        guard persistsPreferences else { return }
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

    /// "EDT · UTC−4", "UTC+5:30", "UTC+0": short enough for a narrow card.
    nonisolated static func compactZoneDescription(_ timeZone: TimeZone, at date: Date) -> String {
        let seconds = timeZone.secondsFromGMT(for: date)
        let sign = seconds < 0 ? "−" : "+"
        let magnitude = abs(seconds)
        let hours = magnitude / 3_600
        let minutes = (magnitude % 3_600) / 60
        let offset = minutes == 0 ? "UTC\(sign)\(hours)" : String(format: "UTC%@%d:%02d", sign, hours, minutes)
        if let abbreviation = timeZone.abbreviation(for: date), !abbreviation.hasPrefix("GMT"), abbreviation != "UTC" {
            return "\(abbreviation) · \(offset)"
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

extension WorldClockViewModel {
    /// The zone list the palette shows for a timestamp: the saved locations in
    /// order, then the local zone and UTC as fallbacks, deduplicated and capped.
    nonisolated static func previewZoneIDs(saved: [String], localZone: TimeZone, limit: Int = 4) -> [String] {
        var ids: [String] = []
        for identifier in WorldClockZoneCatalog.validIdentifiers(saved) + [localZone.identifier, "UTC"] {
            let id = ["GMT", "Etc/GMT", "Etc/UTC", "UTC"].contains(identifier) ? "UTC" : identifier
            if TimeZone(identifier: id) != nil, !ids.contains(id) { ids.append(id) }
        }
        return Array(ids.prefix(max(1, limit)))
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
