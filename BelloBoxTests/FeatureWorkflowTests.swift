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

    func testLateCancelledAIRequestCannotClearNewWorldClockRequest() async throws {
        let settings = AppSettings(defaults: defaults)
        settings.openAIModel = "test-model"
        settings.apiKey = "test-only-key"
        let requests = SuspendedClockRequests()
        let model = WorldClockViewModel(settings: settings, resolveRequest: { request, _ in
            try await requests.resolve(request)
        })
        model.aiRequest = "first"
        model.fillWithAI()
        await fulfillment(of: [requests.firstStarted], timeout: 2)
        model.cancelAI()
        let originalZones = model.zoneIDs
        model.aiRequest = "second"
        model.fillWithAI()
        await fulfillment(of: [requests.secondStarted], timeout: 2)
        requests.finish("first", result: .failure(WorldClockAIError.malformedResponse))
        for _ in 0..<20 { await Task.yield() }
        XCTAssertTrue(model.isResolvingAI)
        XCTAssertEqual(model.zoneIDs, originalZones)
        XCTAssertNil(model.aiMessage)
        requests.finish("second", result: .success(WorldClockAIResult(timeZoneIDs: ["UTC"], referenceDate: nil, anchorTimeZoneID: "UTC")))
        for _ in 0..<100 where model.isResolvingAI { await Task.yield() }
        XCTAssertFalse(model.isResolvingAI)
        XCTAssertEqual(model.zoneIDs, ["UTC"])
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
    private var continuations: [String: CheckedContinuation<WorldClockAIResult, Error>] = [:]

    func resolve(_ request: String) async throws -> WorldClockAIResult {
        try await withCheckedThrowingContinuation { continuation in
            continuations[request] = continuation
            (request == "first" ? firstStarted : secondStarted).fulfill()
        }
    }

    func finish(_ request: String, result: Result<WorldClockAIResult, Error>) {
        continuations.removeValue(forKey: request)?.resume(with: result)
    }
}
