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

    func testCopilotParserAcceptsFencedJSONAndNormalizesDuplicates() throws {
        let reply = try WorldClockCopilotResponseParser().parse(
            """
            ```json
            {"answer":"2:30 PM in Singapore is 7:30 AM in London.","suggestion":{"referenceDate":"2026-08-25T14:30:00+08:00","timeZoneIDs":["Asia/Singapore","Europe/London","Asia/Singapore"],"replaceLocations":true,"anchorTimeZoneID":"Asia/Singapore"}}
            ```
            """
        )

        XCTAssertEqual(reply.answer, "2:30 PM in Singapore is 7:30 AM in London.")
        let suggestion = try XCTUnwrap(reply.suggestion)
        XCTAssertEqual(suggestion.zoneIDs, ["Asia/Singapore", "Europe/London"])
        XCTAssertEqual(suggestion.anchorZoneID, "Asia/Singapore")
        XCTAssertTrue(suggestion.replacesLocations)
        XCTAssertEqual(suggestion.instant, ISO8601DateFormatter().date(from: "2026-08-25T14:30:00+08:00"))
        XCTAssertNil(reply.suggestionIssue)
    }

    func testCopilotParserKeepsPlainAnswersAndDropsInvalidSuggestionParts() throws {
        let parser = WorldClockCopilotResponseParser()

        let plain = try parser.parse("It is 9 PM in Tokyo, which is late for a call.")
        XCTAssertEqual(plain.answer, "It is 9 PM in Tokyo, which is late for a call.")
        XCTAssertNil(plain.suggestion)

        let mixed = try parser.parse(
            "{\"answer\":\"Try tomorrow.\",\"suggestion\":{\"referenceDate\":\"tomorrow\",\"timeZoneIDs\":[\"Mars/Olympus\",\"UTC\"],\"anchorTimeZoneID\":\"Mars/Olympus\"}}"
        )
        XCTAssertEqual(mixed.answer, "Try tomorrow.")
        XCTAssertEqual(mixed.suggestion?.zoneIDs, ["UTC"])
        XCTAssertNil(mixed.suggestion?.instant)
        XCTAssertNil(mixed.suggestion?.anchorZoneID)
        let issue = try XCTUnwrap(mixed.suggestionIssue)
        XCTAssertTrue(issue.contains("Mars/Olympus"))
        XCTAssertTrue(issue.contains("tomorrow"))

        XCTAssertThrowsError(try parser.parse("   ")) { error in
            XCTAssertEqual(error as? WorldClockCopilotError, .emptyAnswer)
        }
        XCTAssertThrowsError(try parser.parse("{\"answer\":\"\",\"suggestion\":null}"))
        let long = try parser.parse(String(repeating: "x", count: 5_000))
        XCTAssertLessThanOrEqual(long.answer.count, WorldClockCopilotResponseParser.answerLimit + 1)
    }

    /// Historical and far-future instants are legitimate planning inputs; only
    /// values outside the finite calendar range are rejected.
    func testCopilotParserAcceptsAnyInstantInsideTheSupportedCalendarRange() throws {
        let parser = WorldClockCopilotResponseParser()
        func suggested(_ value: String) throws -> WorldClockCopilotReply {
            try parser.parse("{\"answer\":\"ok\",\"suggestion\":{\"referenceDate\":\"\(value)\",\"timeZoneIDs\":[]}}")
        }
        for value in ["2000-03-01T10:00:00Z", "2040-06-15T09:00:00+02:00", "1969-12-31T23:00:00Z",
                      "1900-01-01T00:00:00Z", "2100-07-04T12:00:00-07:00", "9999-12-31T23:59:59Z"] {
            let reply = try suggested(value)
            XCTAssertEqual(reply.suggestion?.instant, ISO8601DateFormatter().date(from: value), value)
            XCTAssertNil(reply.suggestionIssue, value)
        }
        let oneHourLater = try suggested("2000-03-01T11:00:00Z")
        XCTAssertEqual(oneHourLater.suggestion?.instant?.timeIntervalSince1970, 951_908_400)

        for value in ["0000-06-01T00:00:00Z", "10000-01-01T00:00:00Z", "-0100-01-01T00:00:00Z"] {
            let reply = try suggested(value)
            XCTAssertNil(reply.suggestion?.instant, value)
            XCTAssertNotNil(reply.suggestionIssue, value)
        }
        // Foundation's distantPast is exactly 0001-01-01 and distantFuture is
        // year 4001; both sit inside the finite range. Anything beyond it does not.
        XCTAssertTrue(WorldClockCopilotResponseParser.isSupportedInstant(.distantPast))
        XCTAssertTrue(WorldClockCopilotResponseParser.isSupportedInstant(.distantFuture))
        XCTAssertFalse(WorldClockCopilotResponseParser.isSupportedInstant(Date.distantPast.addingTimeInterval(-1)))
        XCTAssertFalse(WorldClockCopilotResponseParser.isSupportedInstant(Date(timeIntervalSince1970: -70_000_000_000)))
        XCTAssertFalse(WorldClockCopilotResponseParser.isSupportedInstant(Date(timeIntervalSince1970: 253_402_300_800)), "Year 10000")
        XCTAssertFalse(WorldClockCopilotResponseParser.isSupportedInstant(Date(timeIntervalSince1970: .infinity)))
        XCTAssertTrue(WorldClockCopilotResponseParser.isSupportedInstant(Date(timeIntervalSince1970: -62_000_000_000)))
        XCTAssertTrue(WorldClockCopilotResponseParser.isSupportedInstant(Date(timeIntervalSince1970: 253_402_300_799)), "End of year 9999")
    }

    @MainActor
    func testCopilotDraftSurvivesRejectionAndRetryNeverResendsAnOlderQuestion() async throws {
        let suiteName = "WorldClockCopilotDraftTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let settings = AppSettings(defaults: defaults)
        settings.providerKind = .anthropic
        settings.apiKey = ""
        let requests = RecordedRequests()
        let model = WorldClockViewModel(settings: settings, preferences: WorldClockPreferencesStore(defaults: defaults),
            askCopilot: { request, config in
                requests.record(request, config: config)
                if request.question == "boom" { throw AIError.http(status: 500, message: "boom") }
                return WorldClockCopilotReply(answer: "fine", suggestion: nil, suggestionIssue: nil)
            })
        let copilot = model.copilot

        copilot.draft = "hello"
        copilot.send()
        XCTAssertEqual(copilot.draft, "hello", "An unavailable provider must not eat the draft")
        XCTAssertEqual(copilot.errorMessage, WorldClockCopilotError.providerNotConfigured.localizedDescription)
        XCTAssertFalse(copilot.canRetry)
        XCTAssertTrue(copilot.messages.isEmpty)

        settings.providerKind = .openAI
        settings.openAIModel = "test-model"
        copilot.draft = "boom"
        copilot.send()
        XCTAssertEqual(copilot.draft, "", "An accepted question clears the draft")
        for _ in 0..<200 where copilot.isBusy { await Task.yield() }
        XCTAssertEqual(copilot.errorMessage, "The provider returned HTTP 500. boom")
        XCTAssertTrue(copilot.canRetry)

        let tooLong = String(repeating: "x", count: WorldClockCopilotRequest.questionLimit + 1)
        copilot.draft = tooLong
        copilot.send()
        XCTAssertEqual(copilot.draft, tooLong, "A rejected question keeps the draft for editing")
        XCTAssertEqual(copilot.errorMessage, WorldClockCopilotError.questionTooLong.localizedDescription)
        XCTAssertFalse(copilot.canRetry, "Rejecting a new question drops the older retry intent")
        copilot.retry()
        XCTAssertFalse(copilot.isBusy)
        XCTAssertEqual(requests.all.map(\.question), ["boom"], "Retry never resends an unrelated older question")

        let combining = "e" + String(repeating: "\u{301}", count: 5_000)
        XCTAssertEqual(combining.count, 1, "Graphemes hide the size of a combining sequence")
        XCTAssertGreaterThan(combining.utf8.count, WorldClockCopilotRequest.questionByteLimit)
        copilot.draft = combining
        copilot.send()
        XCTAssertEqual(copilot.draft, combining)
        XCTAssertTrue(copilot.errorMessage?.contains("8 KB") == true)
        XCTAssertEqual(requests.all.count, 1)

        copilot.draft = "ok"
        copilot.send()
        XCTAssertEqual(copilot.draft, "")
        for _ in 0..<200 where copilot.isBusy { await Task.yield() }
        XCTAssertEqual(copilot.outcome, .answered)
        XCTAssertEqual(requests.all.map(\.question), ["boom", "ok"])
        XCTAssertEqual(copilot.messages.map(\.text), ["boom", "ok", "fine"])
    }

    @MainActor
    func testReturnWhileBusyKeepsTheDraftAndSnapshotRecordsTheUnansweredQuestion() async throws {
        let suiteName = "WorldClockCopilotBusyTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        let started = expectation(description: "request started")
        let requests = RecordedRequests()
        let model = WorldClockViewModel(settings: settings, preferences: WorldClockPreferencesStore(defaults: defaults),
            askCopilot: { request, config in
                requests.record(request, config: config)
                started.fulfill()
                try await Task.sleep(nanoseconds: 5_000_000_000)
                return WorldClockCopilotReply(answer: "late", suggestion: nil, suggestionIssue: nil)
            })
        let copilot = model.copilot
        copilot.draft = "first"
        copilot.send()
        await fulfillment(of: [started], timeout: 2)
        XCTAssertTrue(copilot.isBusy)
        copilot.draft = "second"
        copilot.send()
        XCTAssertEqual(copilot.draft, "second", "Return while busy keeps the draft")
        XCTAssertEqual(copilot.messages.map(\.text), ["first"])
        XCTAssertEqual(requests.all.count, 1)
        XCTAssertFalse(copilot.ask("third"))
        XCTAssertTrue(copilot.isBusy, "Sending while busy changes nothing")

        let snapshot = copilot.snapshot()
        XCTAssertEqual(snapshot.outcome, .cancelled, "An in-flight request cannot travel; it is recorded as unanswered")
        XCTAssertEqual(snapshot.pendingQuestion, "first")
        XCTAssertEqual(snapshot.draft, "second")
        copilot.cancel()
        XCTAssertEqual(copilot.draft, "second")
        XCTAssertTrue(copilot.canRetry)
    }

    @MainActor
    func testOpenCopilotReactsWhenProviderConfigurationChanges() throws {
        let suiteName = "WorldClockCopilotSettingsTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let settings = AppSettings(defaults: defaults)
        settings.providerKind = .anthropic
        settings.apiKey = ""
        let model = WorldClockViewModel(settings: settings, preferences: WorldClockPreferencesStore(defaults: defaults),
            askCopilot: { _, _ in WorldClockCopilotReply(answer: "", suggestion: nil, suggestionIssue: nil) })
        XCTAssertFalse(model.copilot.canUseAI)
        var copilotChanges = 0
        var modelChanges = 0
        let observers = [
            model.copilot.objectWillChange.sink { _ in copilotChanges += 1 },
            model.objectWillChange.sink { _ in modelChanges += 1 },
        ]
        defer { observers.forEach { $0.cancel() } }

        settings.providerKind = .openAI
        settings.openAIModel = "test-model"
        XCTAssertTrue(model.copilot.canUseAI)
        XCTAssertGreaterThan(copilotChanges, 0, "The copilot view must re-render when the provider becomes usable")
        XCTAssertGreaterThan(modelChanges, 0)

        let before = copilotChanges
        settings.openAIModel = ""
        XCTAssertFalse(model.copilot.canUseAI)
        XCTAssertGreaterThan(copilotChanges, before)
    }

    @MainActor
    func testHandoffAdoptsInstantReferenceAndConversationWithoutSaving() throws {
        let suiteName = "WorldClockHandoffTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let preferences = WorldClockPreferencesStore(defaults: defaults)
        preferences.save(zoneIDs: ["Asia/Singapore", "Europe/London"], anchorZoneID: "Asia/Singapore")
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        let model = WorldClockViewModel(settings: settings, preferences: preferences,
            askCopilot: { _, _ in WorldClockCopilotReply(answer: "later", suggestion: nil, suggestionIssue: nil) })
        XCTAssertTrue(model.isFollowingNow)
        XCTAssertEqual(model.copilotRevealRequest, 0)

        let instant = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-09-08T12:15:00Z"))
        let question = WorldClockCopilotSession.Message(id: UUID(), role: .user, text: "Add Tokyo")
        let reply = WorldClockCopilotSession.Message(id: UUID(), role: .assistant, text: "Tokyo would be 9:15 PM.",
            suggestion: WorldClockCopilotSuggestion(instant: nil, zoneIDs: ["Asia/Tokyo"], replacesLocations: false, anchorZoneID: nil), issue: nil)
        let snapshot = WorldClockCopilotSnapshot(messages: [question, reply], draft: "and Berlin?", appliedParts: [:],
            pendingQuestion: "Add Tokyo", outcome: .cancelled)
        model.adopt(WorldClockHandoff(instant: instant, anchorZoneID: "Europe/London", copilot: snapshot))

        XCTAssertEqual(model.selectedInstant, instant)
        XCTAssertFalse(model.isFollowingNow)
        XCTAssertEqual(model.anchorZoneID, "Europe/London", "The previewed reference carries over")
        XCTAssertEqual(model.timeline.anchorTimeZone.identifier, "Europe/London")
        XCTAssertEqual(preferences.loadAnchorZoneID(validZoneIDs: ["Asia/Singapore", "Europe/London"]), "Asia/Singapore",
                       "Adopting a handoff never writes preferences")
        XCTAssertEqual(model.copilot.messages, [question, reply])
        XCTAssertEqual(model.copilot.draft, "and Berlin?")
        XCTAssertTrue(model.copilot.statusMessage?.contains("Not answered") == true)
        XCTAssertTrue(model.copilot.canRetry, "An unanswered question can be asked again in the window")
        XCTAssertEqual(model.copilotRevealRequest, 1)
        let plan = try XCTUnwrap(model.copilotPlan(for: reply), "The window can apply the location proposal")
        XCTAssertEqual(plan.zoneIDs, ["Asia/Singapore", "Europe/London", "Asia/Tokyo"])
        XCTAssertNil(model.deferredCopilotPlan(for: reply), "Nothing is deferred in the window")

        // A later handoff for another selection replaces the conversation and
        // may bring a reference that was only a preview fallback.
        let next = instant.addingTimeInterval(86_400)
        model.adopt(WorldClockHandoff(instant: next, anchorZoneID: "UTC", copilot: nil))
        XCTAssertEqual(model.selectedInstant, next)
        XCTAssertEqual(model.anchorZoneID, "UTC")
        XCTAssertEqual(model.zoneIDs, ["Asia/Singapore", "Europe/London", "UTC"])
        XCTAssertEqual(preferences.loadZoneIDs(), ["Asia/Singapore", "Europe/London"])
        XCTAssertTrue(model.copilot.messages.isEmpty, "A new selection context drops the earlier conversation")
        XCTAssertEqual(model.copilot.draft, "")
        XCTAssertEqual(model.copilotRevealRequest, 1)

        // Opening from the menu hands over nothing and changes nothing.
        model.copilot.draft = "typed here"
        model.adopt(WorldClockHandoff())
        XCTAssertEqual(model.copilot.draft, "typed here")
        XCTAssertEqual(model.anchorZoneID, "UTC")
    }

    /// A suggestion that moves the time and adds a location: the palette
    /// applies the time, the handoff carries only that completion, and the
    /// window still offers the location. Preferences change only at the end.
    @MainActor
    func testMixedSuggestionKeepsItsLocationPartActionableAcrossTheHandoff() async throws {
        let suiteName = "WorldClockMixedSuggestionTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let preferences = WorldClockPreferencesStore(defaults: defaults)
        preferences.save(zoneIDs: ["Asia/Singapore", "Europe/London"], anchorZoneID: "Asia/Singapore")
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        let seed = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-09-08T12:00:00Z"))
        let mixed: WorldClockCopilotSession.Responder = { request, _ in
            WorldClockCopilotReply(
                answer: "One hour later and Tokyo added.",
                suggestion: WorldClockCopilotSuggestion(instant: request.context.selectedInstant.addingTimeInterval(3_600),
                                                        zoneIDs: ["Asia/Tokyo"], replacesLocations: false, anchorZoneID: nil),
                suggestionIssue: nil)
        }
        let preview = WorldClockViewModel(settings: settings, seedDate: seed, preferences: preferences, mode: .preview,
            zoneIDs: ["Asia/Singapore", "Europe/London", "UTC"], anchorZoneID: "Europe/London", askCopilot: mixed)
        preview.copilot.draft = "Add Tokyo and move one hour later"
        preview.copilot.send()
        for _ in 0..<200 where preview.copilot.isBusy { await Task.yield() }
        let reply = try XCTUnwrap(preview.copilot.messages.last)
        XCTAssertEqual(reply.suggestion?.parts, [.time, .locations])

        let timePlan = try XCTUnwrap(preview.copilotPlan(for: reply))
        XCTAssertEqual(timePlan.parts, .time)
        XCTAssertNil(timePlan.zoneIDs)
        let deferred = try XCTUnwrap(preview.deferredCopilotPlan(for: reply))
        XCTAssertEqual(deferred.parts, .locations)
        XCTAssertEqual(deferred.zoneIDs, ["Asia/Singapore", "Europe/London", "UTC", "Asia/Tokyo"])

        preview.applyCopilotPlan(timePlan, from: reply)
        XCTAssertEqual(preview.selectedInstant, seed.addingTimeInterval(3_600))
        XCTAssertEqual(preview.copilot.appliedParts(for: reply), .time)
        XCTAssertTrue(preview.copilot.isApplied(reply))
        XCTAssertFalse(preview.copilot.isFullyApplied(reply), "The location part is still open")
        XCTAssertNil(preview.copilotPlan(for: reply), "The time is now in effect")
        XCTAssertNotNil(preview.deferredCopilotPlan(for: reply), "The palette keeps pointing at the window for Tokyo")
        XCTAssertFalse(preview.zoneIDs.contains("Asia/Tokyo"))
        XCTAssertEqual(preferences.loadZoneIDs(), ["Asia/Singapore", "Europe/London"])

        let handoff = WorldClockHandoff(instant: preview.selectedInstant, anchorZoneID: preview.anchorZoneID,
                                        copilot: preview.copilot.snapshot())
        XCTAssertEqual(handoff.copilot?.appliedParts[reply.id], .time, "Only the applied part travels")

        let window = WorldClockViewModel(settings: settings, preferences: preferences, askCopilot: mixed)
        window.adopt(handoff)
        XCTAssertEqual(window.selectedInstant, seed.addingTimeInterval(3_600))
        XCTAssertEqual(window.anchorZoneID, "Europe/London")
        XCTAssertEqual(window.copilot.appliedParts(for: reply), .time)
        XCTAssertNil(window.deferredCopilotPlan(for: reply))
        let locationPlan = try XCTUnwrap(window.copilotPlan(for: reply), "The window still offers the location part")
        XCTAssertEqual(locationPlan.parts, .locations)
        XCTAssertNil(locationPlan.instant, "The time part is already in effect and is not re-applied")
        XCTAssertEqual(locationPlan.zoneIDs, ["Asia/Singapore", "Europe/London", "Asia/Tokyo"])
        XCTAssertEqual(preferences.loadZoneIDs(), ["Asia/Singapore", "Europe/London"], "Nothing saved before the explicit Apply")

        window.applyCopilotPlan(locationPlan, from: reply)
        XCTAssertEqual(window.zoneIDs, ["Asia/Singapore", "Europe/London", "Asia/Tokyo"])
        XCTAssertEqual(preferences.loadZoneIDs(), ["Asia/Singapore", "Europe/London", "Asia/Tokyo"], "Saved only at the last step")
        XCTAssertEqual(preferences.loadAnchorZoneID(validZoneIDs: window.zoneIDs), "Europe/London",
                       "The carried reference is saved along with the explicit location change")
        XCTAssertEqual(window.copilot.appliedParts(for: reply), [.time, .locations])
        XCTAssertTrue(window.copilot.isFullyApplied(reply))
        XCTAssertNil(window.copilotPlan(for: reply))
    }

    func testScriptedFixtureProposesMixedTimeAndLocationChanges() throws {
        let utc = try XCTUnwrap(TimeZone(identifier: "UTC"))
        let instant = try date(2026, 9, 8, 12, 0, in: utc)
        let context = WorldClockCopilotContext(selectedInstant: instant, referenceZoneID: "UTC",
            locations: [.init(zoneID: "UTC", name: "UTC", localDescription: "Tue Sep 8, 12:00 PM (UTC+00:00)", quality: .working, isReference: true)],
            now: instant, localZoneID: "UTC", isFollowingNow: false)
        func reply(_ question: String) -> WorldClockCopilotReply {
            WorldClockCopilotFixture.scriptedReply(for: WorldClockCopilotRequest(question: question, context: context, history: []))
        }
        let mixed = reply("Add Tokyo and move one hour later")
        XCTAssertEqual(mixed.suggestion?.parts, [.time, .locations])
        XCTAssertEqual(mixed.suggestion?.instant, instant.addingTimeInterval(3_600))
        XCTAssertEqual(mixed.suggestion?.zoneIDs, ["Asia/Tokyo"])
        XCTAssertFalse(mixed.answer.contains("daytime"), "Canned text never asserts conclusions it did not compute")
        XCTAssertEqual(reply("add Tokyo").suggestion?.parts, .locations)
        XCTAssertEqual(reply("Is this a good time?").suggestion?.parts, .time)
        XCTAssertTrue(WorldClockAIResolver.systemPrompt.contains("remove a location"))
        XCTAssertTrue(WorldClockAIResolver.systemPrompt.contains("every zone that should remain"))
    }

    func testCompactZoneDescriptionFitsNarrowCards() throws {
        let summer = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-07-07T12:00:00Z"))
        let winter = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-01-07T12:00:00Z"))
        XCTAssertEqual(WorldClockViewModel.compactZoneDescription(try XCTUnwrap(TimeZone(identifier: "America/New_York")), at: summer), "EDT · UTC−4")
        XCTAssertEqual(WorldClockViewModel.compactZoneDescription(try XCTUnwrap(TimeZone(identifier: "America/New_York")), at: winter), "EST · UTC−5")
        // macOS reports only "GMT+5:30" style abbreviations for these zones, so
        // the concise offset stands alone rather than repeating it.
        XCTAssertEqual(WorldClockViewModel.compactZoneDescription(try XCTUnwrap(TimeZone(identifier: "Asia/Kolkata")), at: summer), "UTC+5:30")
        XCTAssertEqual(WorldClockViewModel.compactZoneDescription(try XCTUnwrap(TimeZone(identifier: "UTC")), at: summer), "UTC+0")
        XCTAssertEqual(WorldClockViewModel.compactZoneDescription(try XCTUnwrap(TimeZone(identifier: "Australia/Adelaide")), at: winter), "UTC+10:30")
    }

    func testCopilotPlanRespectsHostCapabilitiesAndReportsOnlyRealChanges() throws {
        let utc = try XCTUnwrap(TimeZone(identifier: "UTC"))
        let instant = try date(2026, 8, 21, 12, 0, in: utc)
        let suggestion = WorldClockCopilotSuggestion(
            instant: instant.addingTimeInterval(3_600), zoneIDs: ["Asia/Tokyo", "UTC", "Nope/Zone"],
            replacesLocations: false, anchorZoneID: "Asia/Tokyo")
        let describe: (Date) -> String = { _ in "1:00 PM" }

        let planner = try XCTUnwrap(suggestion.plan(currentZoneIDs: ["UTC"], currentAnchorZoneID: "UTC",
            currentInstant: instant, allowsLocationChanges: true, describeInstant: describe))
        XCTAssertEqual(planner.instant, instant.addingTimeInterval(3_600))
        XCTAssertEqual(planner.zoneIDs, ["UTC", "Asia/Tokyo"], "Existing locations stay; invalid ones are dropped")
        XCTAssertEqual(planner.anchorZoneID, "Asia/Tokyo")
        XCTAssertEqual(planner.summary, "Set time to 1:00 PM · Add Tokyo · Reference: Tokyo")

        let preview = try XCTUnwrap(suggestion.plan(currentZoneIDs: ["UTC"], currentAnchorZoneID: "UTC",
            currentInstant: instant, allowsLocationChanges: false, describeInstant: describe))
        XCTAssertNil(preview.zoneIDs, "The palette preview never changes saved locations")
        XCTAssertNil(preview.anchorZoneID)
        XCTAssertEqual(preview.summary, "Set time to 1:00 PM")

        let replacing = WorldClockCopilotSuggestion(instant: nil, zoneIDs: ["Europe/Berlin"], replacesLocations: true, anchorZoneID: nil)
        let replaced = try XCTUnwrap(replacing.plan(currentZoneIDs: ["UTC", "Asia/Tokyo"], currentAnchorZoneID: "UTC",
            currentInstant: instant, allowsLocationChanges: true, describeInstant: describe))
        XCTAssertEqual(replaced.zoneIDs, ["Europe/Berlin"])
        XCTAssertEqual(replaced.anchorZoneID, "Europe/Berlin", "The reference follows the list when it would vanish")

        let noop = WorldClockCopilotSuggestion(instant: instant.addingTimeInterval(20), zoneIDs: ["UTC"], replacesLocations: false, anchorZoneID: "UTC")
        XCTAssertNil(noop.plan(currentZoneIDs: ["UTC"], currentAnchorZoneID: "UTC", currentInstant: instant,
            allowsLocationChanges: true, describeInstant: describe))
    }

    func testCopilotPromptCarriesPlannerContextAndBoundedHistory() throws {
        let utc = try XCTUnwrap(TimeZone(identifier: "UTC"))
        let instant = try date(2026, 8, 21, 12, 0, in: utc)
        let context = WorldClockCopilotContext(
            selectedInstant: instant, referenceZoneID: "Europe/London",
            locations: [
                .init(zoneID: "Europe/London", name: "London", localDescription: "Fri Aug 21, 1:00 PM (BST - UTC+01:00)", quality: .working, isReference: true),
                .init(zoneID: "Asia/Tokyo", name: "Tokyo", localDescription: "Fri Aug 21, 9:00 PM (JST - UTC+09:00)", quality: .poor, isReference: false),
            ],
            now: instant.addingTimeInterval(-86_400), localZoneID: "Asia/Singapore", isFollowingNow: false)
        let history = (0..<10).map { WorldClockCopilotTurn(role: $0 % 2 == 0 ? .user : .assistant, text: "turn \($0) " + String(repeating: "x", count: 700)) }
        let prompt = WorldClockAIResolver.userPrompt(for: WorldClockCopilotRequest(question: "  Is Tokyo awake?  ", context: context, history: history))

        XCTAssertTrue(prompt.contains("Selected instant: 2026-08-21T12:00:00Z (planning time)"))
        XCTAssertTrue(prompt.contains("Reference location: London (Europe/London)"))
        XCTAssertTrue(prompt.contains("Tokyo (Asia/Tokyo): Fri Aug 21, 9:00 PM (JST - UTC+09:00) — late night / early morning"))
        XCTAssertTrue(prompt.contains("[reference]"))
        XCTAssertTrue(prompt.contains("user's local zone Asia/Singapore"))
        XCTAssertTrue(prompt.hasSuffix("Question: Is Tokyo awake?"))
        XCTAssertFalse(prompt.contains("turn 3 "), "Only the most recent turns are sent")
        XCTAssertTrue(prompt.contains("turn 4 "))
        XCTAssertTrue(prompt.contains("turn 9 "))
        for line in prompt.components(separatedBy: "\n") where line.hasPrefix("User:") || line.hasPrefix("Assistant:") {
            XCTAssertLessThanOrEqual(line.count, WorldClockCopilotRequest.historyTurnLimit + 20)
        }
        XCTAssertTrue(WorldClockAIResolver.systemPrompt.contains("\"suggestion\""))
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
    func testPreviewModePlannerNeverWritesPreferencesAndRollsDaysAcrossDST() throws {
        let suiteName = "WorldClockPreviewTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let preferences = WorldClockPreferencesStore(defaults: defaults)
        preferences.save(zoneIDs: ["America/New_York", "Europe/London"], anchorZoneID: "Europe/London")
        let newYork = try XCTUnwrap(TimeZone(identifier: "America/New_York"))
        let seed = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-03-07T20:00:00Z")) // Sat 3:00 PM EST
        let model = WorldClockViewModel(
            settings: AppSettings(defaults: defaults), seedDate: seed, preferences: preferences, mode: .preview,
            zoneIDs: ["America/New_York", "Europe/London", "UTC", "bad/zone"], anchorZoneID: "America/New_York",
            askCopilot: { _, _ in XCTFail("The preview must never call the provider on its own"); throw AIError.emptyResponse })
        XCTAssertEqual(model.zoneIDs, ["America/New_York", "Europe/London", "UTC"])
        XCTAssertEqual(model.anchorZoneID, "America/New_York")
        XCTAssertFalse(model.isFollowingNow)
        XCTAssertFalse(model.hasMovedFromSeed)
        XCTAssertFalse(model.persistsPreferences)

        model.setAnchorZone("UTC")
        model.addZone("Asia/Tokyo")
        XCTAssertEqual(model.zoneIDs.last, "Asia/Tokyo")
        XCTAssertEqual(preferences.loadZoneIDs(), ["America/New_York", "Europe/London"], "Preview edits stay in memory")
        XCTAssertEqual(preferences.loadAnchorZoneID(validZoneIDs: ["America/New_York", "Europe/London"]), "Europe/London")
        model.removeZone("Asia/Tokyo")
        model.setAnchorZone("America/New_York")

        model.focus(on: try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-03-08T04:50:00Z"))) // 11:50 PM EST
        XCTAssertEqual(model.zonePresentations.map(\.dayDifference), [0, 1, 1])
        let dayEnd = model.timeline.end
        model.nudge(by: 1)
        XCTAssertEqual(model.selectedInstant, try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-03-08T05:05:00Z")))
        XCTAssertEqual(model.timeline.start, dayEnd, "Stepping past midnight rolls into the next reference day")
        XCTAssertEqual(model.timeline.duration, 23 * 3_600, accuracy: 0.001, "March 8 2026 loses an hour in New York")
        XCTAssertEqual(model.zonePresentations.map(\.dayDifference), [0, 0, 0])
        XCTAssertTrue(model.hasMovedFromSeed)

        model.returnToSeed()
        XCTAssertEqual(model.selectedInstant, seed)
        XCTAssertFalse(model.hasMovedFromSeed)
        model.moveDay(by: 1)
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = newYork
        let local = calendar.dateComponents([.day, .hour, .minute], from: model.selectedInstant)
        XCTAssertEqual([local.day, local.hour, local.minute], [8, 15, 0], "Local wall time survives the DST change")
        XCTAssertTrue(model.accessibilitySummary.contains("New York"))
    }

    @MainActor
    func testCopilotSessionSendsOnlyOnExplicitActionWithPlannerContext() async throws {
        let suiteName = "WorldClockCopilotTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let preferences = WorldClockPreferencesStore(defaults: defaults)
        preferences.save(zoneIDs: ["UTC", "Asia/Tokyo"], anchorZoneID: "UTC")
        let settings = AppSettings(defaults: defaults)
        settings.providerKind = .anthropic
        settings.apiKey = ""
        let requests = RecordedRequests()
        let seed = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-08-21T12:00:00Z"))
        let model = WorldClockViewModel(settings: settings, seedDate: seed, preferences: preferences, mode: .preview,
            localTimeZone: try XCTUnwrap(TimeZone(identifier: "Europe/Berlin")), askCopilot: { request, config in
                requests.record(request, config: config)
                return WorldClockCopilotReply(answer: "Tokyo is at 9:00 PM.", suggestion: nil, suggestionIssue: nil)
            })
        let copilot = model.copilot
        model.nudge(by: 3)
        XCTAssertEqual(requests.all.count, 0, "Opening and scrubbing never send anything")

        XCTAssertFalse(copilot.canUseAI, "Anthropic without a key is not configured")
        copilot.draft = "Is Tokyo awake?"
        XCTAssertFalse(copilot.canSend)
        copilot.send()
        XCTAssertEqual(copilot.outcome, .failed(WorldClockCopilotError.providerNotConfigured.localizedDescription))
        XCTAssertTrue(copilot.messages.isEmpty)
        XCTAssertEqual(requests.all.count, 0)

        settings.providerKind = .openAI
        settings.openAIModel = "test-model"
        XCTAssertTrue(copilot.canUseAI)
        copilot.draft = String(repeating: "x", count: WorldClockCopilotRequest.questionLimit + 1)
        copilot.send()
        XCTAssertEqual(copilot.outcome, .failed(WorldClockCopilotError.questionTooLong.localizedDescription))
        XCTAssertEqual(requests.all.count, 0)

        copilot.draft = "  Is Tokyo awake?  "
        XCTAssertTrue(copilot.canSend)
        copilot.send()
        XCTAssertEqual(copilot.draft, "")
        XCTAssertTrue(copilot.isBusy)
        XCTAssertEqual(copilot.messages.map(\.role), [.user])
        for _ in 0..<200 where copilot.isBusy { await Task.yield() }
        XCTAssertFalse(copilot.isBusy)
        XCTAssertEqual(copilot.outcome, .answered)
        XCTAssertEqual(copilot.messages.map(\.text), ["Is Tokyo awake?", "Tokyo is at 9:00 PM."])
        let request = try XCTUnwrap(requests.all.first)
        XCTAssertEqual(request.question, "Is Tokyo awake?")
        XCTAssertEqual(request.context.selectedInstant, seed.addingTimeInterval(45 * 60), "Context reflects the scrubbed time")
        XCTAssertEqual(request.context.locations.map(\.zoneID), ["UTC", "Asia/Tokyo"])
        XCTAssertEqual(request.context.referenceZoneID, "UTC")
        XCTAssertEqual(request.context.localZoneID, "Europe/Berlin")
        XCTAssertEqual(request.context.locations.last?.quality, .poor)
        XCTAssertTrue(request.history.isEmpty)
        XCTAssertEqual(requests.configs.first?.kind, .openAI)

        copilot.ask("And Berlin?")
        for _ in 0..<200 where copilot.isBusy { await Task.yield() }
        XCTAssertEqual(requests.all.last?.history.map(\.role), [.user, .assistant], "Follow-ups carry the transcript")
        XCTAssertEqual(copilot.messages.count, 4)
        copilot.clear()
        XCTAssertTrue(copilot.messages.isEmpty)
        XCTAssertEqual(copilot.outcome, .none)
    }

    @MainActor
    func testCopilotRetryAfterFailureReusesTheQuestionWithoutDuplicatingIt() async throws {
        let suiteName = "WorldClockCopilotRetryTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        let attempts = RecordedRequests()
        let model = WorldClockViewModel(settings: settings, preferences: WorldClockPreferencesStore(defaults: defaults),
            askCopilot: { request, config in
                attempts.record(request, config: config)
                if attempts.all.count == 1 { throw AIError.http(status: 500, message: "boom") }
                return WorldClockCopilotReply(answer: "Second time works.", suggestion: nil, suggestionIssue: nil)
            })
        let copilot = model.copilot
        copilot.draft = "Best slot?"
        copilot.send()
        for _ in 0..<200 where copilot.isBusy { await Task.yield() }
        XCTAssertEqual(copilot.errorMessage, "The provider returned HTTP 500. boom")
        XCTAssertTrue(copilot.canRetry)
        XCTAssertEqual(copilot.messages.map(\.role), [.user])

        copilot.retry()
        XCTAssertTrue(copilot.isBusy)
        for _ in 0..<200 where copilot.isBusy { await Task.yield() }
        XCTAssertEqual(attempts.all.count, 2)
        XCTAssertEqual(attempts.all.last?.question, "Best slot?")
        XCTAssertTrue(attempts.all.last?.history.isEmpty == true, "The failed question is not replayed as history")
        XCTAssertEqual(copilot.messages.map(\.text), ["Best slot?", "Second time works."])
        XCTAssertNil(copilot.errorMessage)
        XCTAssertFalse(copilot.canRetry)
    }

    func testTimelineScrollAccumulatorTurnsTrackpadDeltasIntoSteps() {
        var accumulator = TimelineScrollAccumulator()
        XCTAssertTrue(accumulator.isHorizontal(deltaX: -5, deltaY: 2))
        XCTAssertFalse(accumulator.isHorizontal(deltaX: 1, deltaY: -8), "Vertical scrolling reaches the list instead")
        XCTAssertFalse(accumulator.isHorizontal(deltaX: 0, deltaY: 0))
        XCTAssertEqual(accumulator.consume(deltaX: -6), 0)
        XCTAssertEqual(accumulator.consume(deltaX: -6), 0)
        XCTAssertEqual(accumulator.consume(deltaX: -4), 1, "Scrolling forward moves later once a step accumulates")
        XCTAssertEqual(accumulator.consume(deltaX: 40), -2)
        accumulator.reset()
        XCTAssertEqual(accumulator.consume(deltaX: -TimelineScrollAccumulator.pixelsPerStep), 1)
    }

    /// Real scroll-wheel `NSEvent`s, sent through `NSApp.sendEvent` exactly as
    /// AppKit dispatches them, must be claimed by the scrubber's window monitor
    /// when horizontal and inside the timeline, and left alone otherwise.
    @MainActor
    func testTimelineScrubberClaimsHorizontalWheelEventsInsideItsVisibleRectOnly() throws {
        let window = NSWindow(contentRect: NSRect(x: 240, y: 240, width: 320, height: 120), styleMask: [.borderless],
                              backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let scrollView = NSScrollView(frame: NSRect(x: 0, y: 0, width: 320, height: 120))
        let document = NSView(frame: NSRect(x: 0, y: 0, width: 320, height: 120))
        let scrubber = TimelineScrubberView(frame: NSRect(x: 20, y: 50, width: 200, height: 14))
        document.addSubview(scrubber)
        scrollView.documentView = document
        window.contentView = scrollView
        window.orderFront(nil)
        XCTAssertTrue(scrubber.isWheelMonitorInstalled, "The monitor exists only while the view is in a window")
        var steps: [Int] = []
        scrubber.onStep = { steps.append($0) }

        func screenPoint(_ local: NSPoint) -> NSPoint {
            window.convertPoint(toScreen: scrubber.convert(local, to: nil))
        }
        let inside = screenPoint(NSPoint(x: 100, y: 7))
        let outside = screenPoint(NSPoint(x: 100, y: 40))
        let horizontal = try wheelEvent(deltaX: -30, deltaY: 2, at: inside)
        XCTAssertEqual(TimelineScrubberView.deltas(for: horizontal).x, -30, accuracy: 0.5, "Synthesized events carry precise deltas")
        var expected = TimelineScrollAccumulator()
        let expectedSteps = expected.consume(deltaX: TimelineScrubberView.deltas(for: horizontal).x)
        XCTAssertGreaterThan(expectedSteps, 0)

        XCTAssertNil(scrubber.routeMonitoredWheel(horizontal), "A horizontal event over the timeline is consumed")
        XCTAssertEqual(steps, [expectedSteps])
        XCTAssertNotNil(scrubber.routeMonitoredWheel(try wheelEvent(deltaX: 3, deltaY: -40, at: inside)), "Vertical scrolling reaches the list")
        XCTAssertNotNil(scrubber.routeMonitoredWheel(try wheelEvent(deltaX: -30, deltaY: 0, at: outside)), "Nothing outside the timeline is hijacked")
        XCTAssertEqual(steps, [expectedSteps])

        // Through AppKit's own dispatch, which is what invokes local monitors.
        NSApp.sendEvent(try wheelEvent(deltaX: -30, deltaY: 0, at: inside))
        XCTAssertEqual(steps.count, 2, "The installed monitor claims the event before any scroll view")
        NSApp.sendEvent(try wheelEvent(deltaX: 0, deltaY: -30, at: inside))
        NSApp.sendEvent(try wheelEvent(deltaX: -30, deltaY: 0, at: outside))
        XCTAssertEqual(steps.count, 2)

        scrubber.removeFromSuperview()
        XCTAssertFalse(scrubber.isWheelMonitorInstalled, "Detaching removes the monitor")
        NSApp.sendEvent(try wheelEvent(deltaX: -30, deltaY: 0, at: inside))
        XCTAssertEqual(steps.count, 2, "A detached scrubber never steps again")
        XCTAssertNotNil(scrubber.routeMonitoredWheel(horizontal))
    }

    private func wheelEvent(deltaX: Int32, deltaY: Int32, at screenPoint: NSPoint) throws -> NSEvent {
        let event = try XCTUnwrap(CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2,
                                          wheel1: deltaY, wheel2: deltaX, wheel3: 0))
        // Core Graphics measures from the top-left of the primary display.
        let primaryHeight = try XCTUnwrap(NSScreen.screens.first).frame.height
        event.location = CGPoint(x: screenPoint.x, y: primaryHeight - screenPoint.y)
        return try XCTUnwrap(NSEvent(cgEvent: event))
    }

    @MainActor
    func testTimelineScrubberMapsClicksToTheDayFraction() throws {
        let scrubber = TimelineScrubberView(frame: NSRect(x: 0, y: 0, width: 110, height: 14))
        var fractions: [CGFloat] = []
        scrubber.onScrub = { fractions.append($0) }
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 110, height: 14), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = scrubber
        XCTAssertTrue(scrubber.acceptsFirstMouse(for: nil), "A first click on the palette must scrub, not just focus")
        XCTAssertFalse(scrubber.acceptsFirstResponder, "The scrubber never takes keyboard focus from the search field")
        let click = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDown, location: NSPoint(x: 60, y: 7), modifierFlags: [],
            timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        scrubber.mouseDown(with: click)
        let drag = try XCTUnwrap(NSEvent.mouseEvent(with: .leftMouseDragged, location: NSPoint(x: 400, y: 7), modifierFlags: [],
            timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
        scrubber.mouseDragged(with: drag)
        XCTAssertEqual(fractions.count, 2)
        XCTAssertEqual(fractions[0], 0.55, accuracy: 0.001)
        XCTAssertEqual(fractions[1], 1, "Dragging past the end clamps to the end of the day")
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

    private final class RecordedRequests: @unchecked Sendable {
        private let lock = NSLock()
        private(set) var all: [WorldClockCopilotRequest] = []
        private(set) var configs: [AIConfig] = []
        func record(_ request: WorldClockCopilotRequest, config: AIConfig) {
            lock.lock(); defer { lock.unlock() }
            all.append(request)
            configs.append(config)
        }
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
