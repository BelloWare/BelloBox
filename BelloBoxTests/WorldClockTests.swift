import AppKit
import XCTest
@testable import BelloBox

final class WorldClockTests: XCTestCase {
    func testMeetingQualityUsesDocumentedLocalTimeBoundaries() throws {
        let singapore = try XCTUnwrap(TimeZone(identifier: "Asia/Singapore"))

        XCTAssertEqual(MeetingTimeQuality.at(try date(2026, 8, 21, 6, 59, in: singapore), in: singapore), .poor)
        XCTAssertEqual(MeetingTimeQuality.at(try date(2026, 8, 21, 7, 0, in: singapore), in: singapore), .extended)
        XCTAssertEqual(MeetingTimeQuality.at(try date(2026, 8, 21, 9, 0, in: singapore), in: singapore), .working)
        XCTAssertEqual(MeetingTimeQuality.at(try date(2026, 8, 21, 16, 59, in: singapore), in: singapore), .working)
        XCTAssertEqual(MeetingTimeQuality.at(try date(2026, 8, 21, 17, 0, in: singapore), in: singapore), .extended)
        XCTAssertEqual(MeetingTimeQuality.at(try date(2026, 8, 21, 21, 0, in: singapore), in: singapore), .poor)
    }

    func testCombinedQualityRequiresEveryLocationToBeInWorkingHours() throws {
        let singapore = try XCTUnwrap(TimeZone(identifier: "Asia/Singapore"))
        let london = try XCTUnwrap(TimeZone(identifier: "Europe/London"))
        let utc = try XCTUnwrap(TimeZone(identifier: "UTC"))

        let good = try date(2026, 8, 21, 8, 0, in: utc)
        XCTAssertEqual(MeetingTimeQuality.combined(at: good, timeZones: [singapore, london]), .working)

        let fringe = try date(2026, 8, 21, 10, 0, in: utc)
        XCTAssertEqual(MeetingTimeQuality.combined(at: fringe, timeZones: [singapore, london]), .extended)

        let poor = try date(2026, 8, 21, 20, 0, in: utc)
        XCTAssertEqual(MeetingTimeQuality.combined(at: poor, timeZones: [singapore, london]), .poor)
    }

    func testTimelineUsesActualDayLengthAcrossDaylightSavingChanges() throws {
        let losAngeles = try XCTUnwrap(TimeZone(identifier: "America/Los_Angeles"))
        let spring = WorldClockTimeline(
            containing: try date(2024, 3, 10, 12, 0, in: losAngeles),
            anchorTimeZone: losAngeles
        )
        let fall = WorldClockTimeline(
            containing: try date(2024, 11, 3, 12, 0, in: losAngeles),
            anchorTimeZone: losAngeles
        )

        XCTAssertEqual(spring.duration, 23 * 3_600, accuracy: 0.001)
        XCTAssertEqual(fall.duration, 25 * 3_600, accuracy: 0.001)
    }

    func testMovingDaysPreservesLocalWallClockTime() throws {
        let losAngeles = try XCTUnwrap(TimeZone(identifier: "America/Los_Angeles"))
        let source = try date(2024, 3, 9, 16, 30, in: losAngeles)
        let timeline = WorldClockTimeline(containing: source, anchorTimeZone: losAngeles)

        let moved = timeline.moving(byDays: 1, preservingLocalTimeOf: source)
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = losAngeles
        let components = calendar.dateComponents([.day, .hour, .minute], from: moved.date)

        XCTAssertEqual(components.day, 10)
        XCTAssertEqual(components.hour, 16)
        XCTAssertEqual(components.minute, 30)
    }

    func testCatalogFindsCommonCityAliasesAndDeduplicatesIdentifiers() {
        let sanFrancisco = WorldClockZoneCatalog.search("San Francisco", excluding: [])
        XCTAssertEqual(sanFrancisco.first?.id, "America/Los_Angeles")

        XCTAssertEqual(
            WorldClockZoneCatalog.validIdentifiers(["Asia/Singapore", "invalid", "Asia/Singapore", "UTC"]),
            ["Asia/Singapore", "UTC"]
        )
    }

    func testPreferencesDiscardInvalidAndDuplicateZones() throws {
        let suiteName = "WorldClockTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let store = WorldClockPreferencesStore(defaults: defaults)

        store.save(
            zoneIDs: ["Asia/Singapore", "bad/zone", "Asia/Singapore", "Europe/London"],
            anchorZoneID: "bad/zone"
        )
        let zones = store.loadZoneIDs(current: try XCTUnwrap(TimeZone(identifier: "UTC")))

        XCTAssertEqual(zones, ["Asia/Singapore", "Europe/London"])
        XCTAssertEqual(store.loadAnchorZoneID(validZoneIDs: zones), "Asia/Singapore")
    }

    func testAIParserAcceptsFencedJSONAndNormalizesDuplicates() throws {
        let result = try WorldClockAIResponseParser().parse(
            """
            ```json
            {"timeZoneIDs":["Asia/Singapore","Europe/London","Asia/Singapore"],"referenceDate":"2026-08-25T14:30:00+08:00","anchorTimeZoneID":"Asia/Singapore"}
            ```
            """
        )

        XCTAssertEqual(result.timeZoneIDs, ["Asia/Singapore", "Europe/London"])
        XCTAssertEqual(result.anchorTimeZoneID, "Asia/Singapore")
        XCTAssertNotNil(result.referenceDate)
    }

    func testAIParserRejectsUnknownZonesAndMalformedDates() {
        XCTAssertThrowsError(
            try WorldClockAIResponseParser().parse(
                "{\"timeZoneIDs\":[\"Mars/Olympus\"],\"referenceDate\":null,\"anchorTimeZoneID\":\"Mars/Olympus\"}"
            )
        ) { error in
            XCTAssertEqual(error as? WorldClockAIError, .invalidTimeZones(["Mars/Olympus"]))
        }

        XCTAssertThrowsError(
            try WorldClockAIResponseParser().parse(
                "{\"timeZoneIDs\":[\"UTC\"],\"referenceDate\":\"tomorrow\",\"anchorTimeZoneID\":\"UTC\"}"
            )
        ) { error in
            XCTAssertEqual(error as? WorldClockAIError, .invalidReferenceDate("tomorrow"))
        }
    }

    @MainActor
    func testViewModelKeepsCachedBandsWhileScrubbingAndRefreshesForZones() throws {
        let suiteName = "WorldClockViewModelTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let preferences = WorldClockPreferencesStore(defaults: defaults)
        preferences.save(zoneIDs: ["UTC"], anchorZoneID: "UTC")
        let settings = AppSettings(defaults: defaults)
        let utc = try XCTUnwrap(TimeZone(identifier: "UTC"))
        let viewModel = WorldClockViewModel(
            settings: settings,
            seedDate: try date(2026, 8, 21, 12, 0, in: utc),
            preferences: preferences
        )
        let originalBands = viewModel.timelineQualities

        viewModel.selectedOffset = 14 * 3_600
        XCTAssertEqual(viewModel.timelineQualities, originalBands)
        XCTAssertEqual(viewModel.dayEndLabel, "Next day")

        viewModel.addZone("Asia/Singapore")
        XCTAssertEqual(viewModel.zoneIDs, ["UTC", "Asia/Singapore"])
        XCTAssertNotEqual(viewModel.timelineQualities, originalBands)
    }

    @MainActor
    func testFloatingTooltipPanelIsNonInteractiveAndSizesToItsText() {
        let panel = FloatingTooltipPanel()
        panel.update(text: "Capture and annotate a screenshot")

        XCTAssertTrue(panel.ignoresMouseEvents)
        XCTAssertFalse(panel.canBecomeKey)
        XCTAssertTrue(panel.collectionBehavior.contains(.fullScreenAuxiliary))
        XCTAssertGreaterThan(panel.frame.width, 100)
        XCTAssertGreaterThan(panel.frame.height, 10)
    }

    @MainActor
    func testWorldClockPanelStaysVisibleAcrossSpacesAndAppDeactivation() {
        let panel = WorldClockPanel(contentViewController: NSViewController())

        XCTAssertTrue(panel.isFloatingPanel)
        XCTAssertEqual(panel.level, .floating)
        XCTAssertFalse(panel.hidesOnDeactivate)
        XCTAssertTrue(panel.collectionBehavior.contains(.canJoinAllSpaces))
        XCTAssertTrue(panel.collectionBehavior.contains(.fullScreenAuxiliary))
    }

    private func date(
        _ year: Int,
        _ month: Int,
        _ day: Int,
        _ hour: Int,
        _ minute: Int,
        in timeZone: TimeZone
    ) throws -> Date {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = timeZone
        return try XCTUnwrap(
            calendar.date(from: DateComponents(
                year: year,
                month: month,
                day: day,
                hour: hour,
                minute: minute
            ))
        )
    }
}
