import CoreGraphics
import XCTest
@testable import BelloBox

@MainActor
final class RecordingCoordinatorTests: XCTestCase {
    func testInputTrackingCanChangeDuringRecordingAndPause() async {
        let engine = MockRecordingEngine(label: "Input test")
        let coordinator = RecordingCoordinator(settings: AppSettings(defaults: temporaryDefaults()),
            makeEngine: { _, _ in engine }, permissionProvider: { _ in .grantedForTests })
        var options = RecordingOptions.default
        options.countdownSeconds = 0
        await coordinator.start(target: .display(displayID: 1), options: options)
        coordinator.updateInputOverlays(clicks: .ringsAndLabels, keys: .allKeys)
        guard case let .recording(enabled) = coordinator.state else { return XCTFail("Expected recording") }
        XCTAssertTrue(enabled.isInputOverlayEnabled)
        XCTAssertEqual(enabled.keystrokeMode, .allKeys)
        coordinator.pause()
        coordinator.updateInputOverlays(clicks: .off, keys: .off)
        guard case let .paused(disabled) = coordinator.state else { return XCTFail("Expected paused recording") }
        XCTAssertFalse(disabled.isInputOverlayEnabled)
        XCTAssertEqual(disabled.keystrokeMode, .off)
        coordinator.resume()
        guard case let .recording(resumed) = coordinator.state else { return XCTFail("Expected resumed recording") }
        XCTAssertFalse(resumed.isInputOverlayEnabled)
        XCTAssertEqual(engine.inputUpdates.count, 2)
        coordinator.cancel()
    }

    func testUnavailableInputMonitorDoesNotClaimTrackingIsEnabled() async {
        let engine = MockRecordingEngine(label: "Input test")
        engine.inputTrackingAvailable = false
        let coordinator = RecordingCoordinator(settings: AppSettings(defaults: temporaryDefaults()),
            makeEngine: { _, _ in engine }, permissionProvider: { _ in .grantedForTests })
        var options = RecordingOptions.default
        options.countdownSeconds = 0
        await coordinator.start(target: .display(displayID: 1), options: options)
        coordinator.updateInputOverlays(clicks: .off, keys: .allKeys)
        guard case let .recording(runtime) = coordinator.state else { return XCTFail("Expected recording") }
        XCTAssertFalse(runtime.isInputOverlayEnabled)
        XCTAssertEqual(runtime.keystrokeMode, .off)
        XCTAssertNotNil(runtime.inputOverlayWarning)
        coordinator.cancel()
    }

    func testCancelDuringFinishingCancelsInFlightEngine() async {
        let settings = AppSettings(defaults: temporaryDefaults())
        let engine = MockRecordingEngine(label: "Mock")
        let coordinator = RecordingCoordinator(
            settings: settings,
            makeEngine: { _, _ in engine },
            permissionProvider: { _ in .grantedForTests }
        )
        var options = RecordingOptions.default
        options.countdownSeconds = 0

        await coordinator.start(target: .display(displayID: CGDirectDisplayID(1)), options: options)
        guard case .recording = coordinator.state else {
            XCTFail("Expected recording state after mock engine start.")
            return
        }

        coordinator.stop()
        await fulfillment(of: [engine.stopStarted], timeout: 1)
        guard case .finishing = coordinator.state else {
            XCTFail("Expected finishing state while mock engine stop is suspended.")
            return
        }

        coordinator.cancel()
        XCTAssertTrue(engine.didCancel)

        engine.completeStop(with: temporaryRecordingURL())
        try? await Task.sleep(nanoseconds: 30_000_000)
        XCTAssertEqual(coordinator.state, .idle)
    }

    func testStartDuringFinishingDoesNotLetPreviousEngineClobberNewRecording() async {
        let settings = AppSettings(defaults: temporaryDefaults())
        let firstEngine = MockRecordingEngine(label: "Mock 1")
        let secondEngine = MockRecordingEngine(label: "Mock 2")
        var engines = [firstEngine, secondEngine]
        let coordinator = RecordingCoordinator(
            settings: settings,
            makeEngine: { _, _ in engines.removeFirst() },
            permissionProvider: { _ in .grantedForTests }
        )
        var options = RecordingOptions.default
        options.countdownSeconds = 0

        await coordinator.start(target: .display(displayID: CGDirectDisplayID(1)), options: options)
        coordinator.stop()
        await fulfillment(of: [firstEngine.stopStarted], timeout: 1)
        guard case .finishing = coordinator.state else {
            XCTFail("Expected finishing state while first engine stop is suspended.")
            return
        }

        await coordinator.start(target: .display(displayID: CGDirectDisplayID(2)), options: options)
        guard case let .recording(runtime) = coordinator.state else {
            XCTFail("Expected a new recording to start.")
            return
        }
        XCTAssertEqual(runtime.targetDescription, "Mock 2")

        firstEngine.completeStop(with: temporaryRecordingURL())
        try? await Task.sleep(nanoseconds: 30_000_000)
        guard case let .recording(finalRuntime) = coordinator.state else {
            XCTFail("Previous engine stop should not clobber the new recording.")
            return
        }
        XCTAssertEqual(finalRuntime.targetDescription, "Mock 2")
        XCTAssertFalse(secondEngine.didCancel)
    }

    func testCancelledCountdownDoesNotStartEngine() async {
        let settings = AppSettings(defaults: temporaryDefaults())
        let engine = MockRecordingEngine(label: "Mock")
        let coordinator = RecordingCoordinator(
            settings: settings,
            makeEngine: { _, _ in engine },
            permissionProvider: { _ in .grantedForTests }
        )
        let countdownStarted = expectation(description: "countdown started")
        coordinator.onStateChange = { state in
            if case .countingDown = state {
                countdownStarted.fulfill()
            }
        }
        var options = RecordingOptions.default
        options.countdownSeconds = 3

        let task = Task { @MainActor in
            await coordinator.start(target: .display(displayID: CGDirectDisplayID(1)), options: options)
        }
        await fulfillment(of: [countdownStarted], timeout: 1)

        task.cancel()
        await task.value

        XCTAssertEqual(engine.startCallCount, 0)
        guard case .idle = coordinator.state else {
            XCTFail("Cancelled countdown should return to idle.")
            return
        }
    }

    func testFailureDuringStartupCannotReturnToRecording() async {
        let engine = MockRecordingEngine(label: "Failed startup")
        engine.suspendStart = true
        let coordinator = RecordingCoordinator(settings: AppSettings(defaults: temporaryDefaults()),
            makeEngine: { _, _ in engine }, permissionProvider: { _ in .grantedForTests })
        let failed = expectation(description: "Failure reported")
        coordinator.onStateChange = { if case .failed = $0 { failed.fulfill() } }
        var options = RecordingOptions.default
        options.countdownSeconds = 0
        let task = Task { await coordinator.start(target: .display(displayID: 1), options: options) }
        await fulfillment(of: [engine.startStarted], timeout: 1)
        engine.onFailure?(RecordingEngineError.streamFailed("Interrupted"))
        await fulfillment(of: [failed], timeout: 1)
        engine.completeStart()
        await task.value
        guard case .failed = coordinator.state else { return XCTFail("Late startup must not revive a failed recording") }
        XCTAssertTrue(engine.didCancel)
    }

    func testCancellingSuspendedStartupCancelsEngineAndIgnoresLateSuccess() async {
        let engine = MockRecordingEngine(label: "Cancelled startup")
        engine.suspendStart = true
        let coordinator = RecordingCoordinator(settings: AppSettings(defaults: temporaryDefaults()),
            makeEngine: { _, _ in engine }, permissionProvider: { _ in .grantedForTests })
        let idle = expectation(description: "Cancelled startup returned to idle")
        coordinator.onStateChange = { if case .idle = $0 { idle.fulfill() } }
        var options = RecordingOptions.default
        options.countdownSeconds = 0
        let task = Task { await coordinator.start(target: .display(displayID: 1), options: options) }
        await fulfillment(of: [engine.startStarted], timeout: 1)
        task.cancel()
        await fulfillment(of: [idle], timeout: 1)
        XCTAssertTrue(engine.didCancel)
        engine.completeStart()
        await task.value
        XCTAssertEqual(coordinator.state, .idle)
    }

    func testRecoverableExportFailureOpensReviewWithoutDiscardingMovie() async {
        let engine = MockRecordingEngine(label: "Recoverable recording")
        let coordinator = RecordingCoordinator(settings: AppSettings(defaults: temporaryDefaults()),
            makeEngine: { _, _ in engine }, permissionProvider: { _ in .grantedForTests })
        let reviewed = expectation(description: "Recovery review opened")
        coordinator.onStateChange = { if case .reviewing = $0 { reviewed.fulfill() } }
        var options = RecordingOptions.default
        options.countdownSeconds = 0
        await coordinator.start(target: .display(displayID: 1), options: options)
        coordinator.stop()
        await fulfillment(of: [engine.stopStarted], timeout: 1)
        let recovered = temporaryRecordingURL()
        engine.failStop(with: RecordingEngineError.recoverableExportFailure(recovered, "Audio mix failed"))
        await fulfillment(of: [reviewed], timeout: 1)
        guard case let .reviewing(url, warning) = coordinator.state else { return XCTFail("Expected recovery review") }
        XCTAssertEqual(url, recovered)
        XCTAssertTrue(warning?.contains("Audio mix failed") == true)
        XCTAssertFalse(engine.didCancel, "Recovery must not discard the playable original")
    }

    private func temporaryDefaults() -> UserDefaults {
        let suiteName = "BelloBoxTests.RecordingCoordinator.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }

    private func temporaryRecordingURL() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxTests-\(UUID().uuidString).mov")
    }
}

private final class MockRecordingEngine: RecordingEngineControlling {
    var onFailure: ((Error) -> Void)?
    var onSecureFieldHiddenChange: ((Bool) -> Void)?

    private let targetDescription: String
    var suspendStart = false
    let startStarted = XCTestExpectation(description: "start suspended")
    private var startContinuation: CheckedContinuation<Void, Never>?
    let stopStarted = XCTestExpectation(description: "stop started")
    private var stopContinuation: CheckedContinuation<URL, Error>?
    private(set) var didCancel = false
    private(set) var startCallCount = 0
    var inputTrackingAvailable = true
    private(set) var inputUpdates: [(ClickOverlayMode, KeystrokeCaptureMode)] = []

    init(label: String) {
        targetDescription = label
    }

    func start() async throws -> RecordingRuntimeState {
        startCallCount += 1
        if suspendStart {
            await withCheckedContinuation { continuation in
                startContinuation = continuation
                startStarted.fulfill()
            }
        }
        return RecordingRuntimeState(
            sessionID: RecordingSessionID(),
            startedAt: Date(),
            targetDescription: targetDescription,
            elapsed: 0,
            isMicEnabled: false,
            isSystemAudioEnabled: false,
            isInputOverlayEnabled: false,
            isSecureFieldHidden: false
        )
    }

    func setPaused(_ paused: Bool) {}

    func updateInputOverlays(clicks: ClickOverlayMode, keys: KeystrokeCaptureMode) -> Bool {
        inputUpdates.append((clicks, keys))
        return inputTrackingAvailable
    }

    func stop() async throws -> URL {
        return try await withCheckedThrowingContinuation { continuation in
            stopContinuation = continuation
            stopStarted.fulfill()
        }
    }

    func cancel() {
        didCancel = true
    }

    func completeStart() {
        startContinuation?.resume()
        startContinuation = nil
    }

    func failStop(with error: Error) {
        stopContinuation?.resume(throwing: error)
        stopContinuation = nil
    }

    func completeStop(with url: URL) {
        stopContinuation?.resume(returning: url)
        stopContinuation = nil
    }
}

private extension RecordingPermissionState {
    static let grantedForTests = RecordingPermissionState(
        screenRecording: .granted,
        microphone: .granted,
        inputMonitoring: .granted,
        accessibility: .granted,
        systemAudio: .granted
    )
}
