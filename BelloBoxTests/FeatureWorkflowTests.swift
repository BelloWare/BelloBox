import AppKit
import XCTest
@testable import BelloBox

@MainActor
final class FeatureWorkflowTests: XCTestCase {
    private var defaults: UserDefaults!
    private var suite: String!

    override func setUp() {
        suite = "FeatureWorkflowTests.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)!
        defaults.set(false, forKey: "screenshotAutoCopy")
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suite)
    }

    func testLiveClockCrossesMidnightButScrubbingStopsAutomaticUpdates() {
        let preferences = WorldClockPreferencesStore(defaults: defaults)
        preferences.save(zoneIDs: ["UTC"], anchorZoneID: "UTC")
        let model = WorldClockViewModel(settings: AppSettings(defaults: defaults), preferences: preferences)
        let nextDay = model.timeline.end.addingTimeInterval(60)
        model.refreshCurrentTime(nextDay)
        XCTAssertEqual(model.selectedInstant, nextDay)
        XCTAssertEqual(model.timeline.start, nextDay.addingTimeInterval(-60))
        XCTAssertTrue(model.isFollowingNow)
        model.selectedOffset = 3_600
        let planned = model.selectedInstant
        model.refreshCurrentTime(nextDay.addingTimeInterval(600))
        XCTAssertEqual(model.selectedInstant, planned)
        XCTAssertFalse(model.isFollowingNow)
        model.goToNow()
        XCTAssertTrue(model.isFollowingNow)
    }

    func testMeetingComparisonIncludesDateChangesAndUTCOffsets() throws {
        let preferences = WorldClockPreferencesStore(defaults: defaults)
        preferences.save(zoneIDs: ["Asia/Tokyo", "America/Los_Angeles"], anchorZoneID: "Asia/Tokyo")
        let instant = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-09-05T00:30:00+09:00"))
        let model = WorldClockViewModel(settings: AppSettings(defaults: defaults), seedDate: instant, preferences: preferences)
        XCTAssertEqual(model.zonePresentations.map(\.dayDifference), [0, -1])
        XCTAssertTrue(model.meetingSummary.contains("UTC+09:00"))
        XCTAssertTrue(model.meetingSummary.contains("UTC-07:00"))
        model.setAnchorZone("America/Los_Angeles")
        XCTAssertEqual(model.selectedInstant, instant)
        XCTAssertEqual(model.zonePresentations.map(\.dayDifference), [1, 0])
    }

    func testLateCancelledCopilotRequestCannotAnswerOrFailANewerQuestion() async throws {
        let preferences = WorldClockPreferencesStore(defaults: defaults)
        preferences.save(zoneIDs: ["UTC"], anchorZoneID: "UTC")
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        settings.apiKey = "test-only-key"
        let requests = SuspendedClockRequests()
        let model = WorldClockViewModel(settings: settings, preferences: preferences, askCopilot: { request, _ in
            try await requests.resolve(request.question)
        })
        let copilot = model.copilot
        copilot.draft = "first"
        copilot.send()
        await fulfillment(of: [requests.firstStarted], timeout: 2)
        XCTAssertTrue(copilot.isBusy)
        model.cancelAI()
        XCTAssertFalse(copilot.isBusy)
        XCTAssertEqual(copilot.outcome, .cancelled)
        XCTAssertTrue(copilot.canRetry, "A cancelled question can be asked again")
        let originalZones = model.zoneIDs
        copilot.draft = "second"
        copilot.send()
        await fulfillment(of: [requests.secondStarted], timeout: 2)
        requests.finish("first", result: .failure(AIError.emptyResponse))
        for _ in 0..<20 { await Task.yield() }
        XCTAssertTrue(copilot.isBusy, "A stale failure must not end the newer question")
        XCTAssertNil(copilot.errorMessage)
        XCTAssertEqual(model.zoneIDs, originalZones)
        requests.finish("second", result: .success(WorldClockCopilotReply(
            answer: "Tokyo would be 9 PM.",
            suggestion: WorldClockCopilotSuggestion(instant: nil, zoneIDs: ["Asia/Tokyo"], replacesLocations: false, anchorZoneID: nil),
            suggestionIssue: nil)))
        for _ in 0..<100 where copilot.isBusy { await Task.yield() }
        XCTAssertFalse(copilot.isBusy)
        XCTAssertEqual(copilot.outcome, .answered)
        XCTAssertEqual(copilot.messages.map(\.role), [.user, .user, .assistant])
        XCTAssertEqual(model.zoneIDs, originalZones, "Suggestions are never applied automatically")
        let reply = try XCTUnwrap(copilot.messages.last)
        let plan = try XCTUnwrap(model.copilotPlan(for: reply))
        XCTAssertEqual(plan.zoneIDs, ["UTC", "Asia/Tokyo"])
        model.applyCopilotPlan(plan, from: reply)
        XCTAssertEqual(model.zoneIDs, ["UTC", "Asia/Tokyo"])
        XCTAssertEqual(preferences.loadZoneIDs(), ["UTC", "Asia/Tokyo"], "The dedicated window saves applied locations")
        XCTAssertTrue(copilot.isApplied(reply))
        XCTAssertNil(model.copilotPlan(for: reply), "An applied suggestion offers nothing further")
    }

    func testResetCropPreservesAnnotationsAndCanBeUndone() {
        let annotation = ScreenshotAnnotation(kind: .rectangle(CGRect(x: 10, y: 10, width: 20, height: 20)))
        let initialCrop = CGRect(x: 20, y: 20, width: 200, height: 140)
        let model = ScreenshotPopupViewModel(document: ScreenshotDocument(
            baseImage: ScreenshotTestHelpers.image(width: 320, height: 200), scale: 1,
            source: .importedClipboard, annotations: [annotation], cropRect: initialCrop), settings: AppSettings(defaults: defaults))
        model.applyVisibleCrop(CGRect(x: 5, y: 5, width: 100, height: 80))
        let editedCrop = model.document.cropRect
        XCTAssertTrue(model.canResetCrop)
        model.resetCrop()
        XCTAssertEqual(model.document.cropRect, initialCrop)
        XCTAssertEqual(model.document.annotations, [annotation])
        model.undo()
        XCTAssertEqual(model.document.cropRect, editedCrop)
        model.redo()
        XCTAssertEqual(model.document.cropRect, initialCrop)
    }

    func testStandaloneTextToolsCannotReplaceAndCanChainThenResetInput() {
        let model = TextToolsPopupViewModel(selection: TextSelection(text: "", anchorRect: nil, appName: nil, bundleID: nil, pid: nil),
            settings: AppSettings(defaults: defaults), accessibility: AccessibilityService())
        var closed = false
        model.onClose = { closed = true }
        model.input = "hello"
        XCTAssertEqual(model.primaryOutput, "HELLO")
        model.input = model.primaryOutput!
        model.category = .encode
        XCTAssertEqual(model.primaryOutput, "SEVMTE8=")
        model.replace("replacement")
        XCTAssertFalse(model.canReplaceSelection)
        XCTAssertFalse(closed)
        model.resetInput()
        XCTAssertEqual(model.input, "")
        XCTAssertFalse(model.canResetInput)
    }
}

@MainActor
private final class SuspendedClockRequests {
    let firstStarted = XCTestExpectation(description: "First clock request")
    let secondStarted = XCTestExpectation(description: "Second clock request")
    private var continuations: [String: CheckedContinuation<WorldClockCopilotReply, Error>] = [:]

    func resolve(_ request: String) async throws -> WorldClockCopilotReply {
        try await withCheckedThrowingContinuation { continuation in
            continuations[request] = continuation
            (request == "first" ? firstStarted : secondStarted).fulfill()
        }
    }

    func finish(_ request: String, result: Result<WorldClockCopilotReply, Error>) {
        continuations.removeValue(forKey: request)?.resume(with: result)
    }
}
