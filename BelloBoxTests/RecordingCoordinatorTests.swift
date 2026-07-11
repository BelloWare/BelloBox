import CoreGraphics
import XCTest
@testable import BelloBox

@MainActor
final class RecordingCoordinatorTests: XCTestCase {
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
    let stopStarted = XCTestExpectation(description: "stop started")
    private var stopContinuation: CheckedContinuation<URL, Error>?
    private(set) var didCancel = false
    private(set) var startCallCount = 0

    init(label: String) {
        targetDescription = label
    }

    func start() async throws -> RecordingRuntimeState {
        startCallCount += 1
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

    func stop() async throws -> URL {
        return try await withCheckedThrowingContinuation { continuation in
            stopContinuation = continuation
            stopStarted.fulfill()
        }
    }

    func cancel() {
        didCancel = true
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
