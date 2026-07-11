import AppKit
import Combine
import Foundation

protocol RecordingEngineControlling: AnyObject {
    var onFailure: ((Error) -> Void)? { get set }
    var onSecureFieldHiddenChange: ((Bool) -> Void)? { get set }

    func start() async throws -> RecordingRuntimeState
    func setPaused(_ paused: Bool)
    func stop() async throws -> URL
    func cancel()
}

extension RecordingEngine: RecordingEngineControlling {}

@MainActor
final class RecordingCoordinator: ObservableObject {
    @Published private(set) var state: RecordingState = .idle

    private let settings: AppSettings
    private let makeEngine: (RecordingTarget, RecordingOptions) -> any RecordingEngineControlling
    private let permissionProvider: (RecordingOptions) -> RecordingPermissionState
    private var activeEngine: (any RecordingEngineControlling)?
    private var finishTask: Task<Void, Never>?
    private var startToken = UUID()
    private var latestSecureFieldHidden = false

    var onStateChange: ((RecordingState) -> Void)?

    init(
        settings: AppSettings,
        makeEngine: @escaping (RecordingTarget, RecordingOptions) -> any RecordingEngineControlling = {
            RecordingEngine(target: $0, options: $1)
        },
        permissionProvider: @escaping (RecordingOptions) -> RecordingPermissionState = RecordingPermissionState.current(options:)
    ) {
        self.settings = settings
        self.makeEngine = makeEngine
        self.permissionProvider = permissionProvider
    }

    var isRecording: Bool {
        switch state {
        case .recording, .paused, .countingDown, .finishing:
            return true
        case .idle, .requestingPermissions, .choosingTarget, .reviewing, .failed:
            return false
        }
    }

    func showRecordingChooser(anchor: CGRect?) {
        setState(.choosingTarget)
    }

    func permissionState(options: RecordingOptions) -> RecordingPermissionState {
        permissionProvider(options)
    }

    func start(target: RecordingTarget, options: RecordingOptions) async {
        let token = UUID()
        startToken = token
        latestSecureFieldHidden = false
        finishTask?.cancel()
        finishTask = nil
        activeEngine?.cancel()
        activeEngine = nil
        let permissionState = permissionProvider(options)
        guard permissionState.canRecordVideo else {
            setState(.failed("Screen Recording permission is required to record video."))
            return
        }

        let seconds = max(0, options.countdownSeconds)
        if seconds > 0 {
            for value in stride(from: seconds, through: 1, by: -1) {
                guard startToken == token, !Task.isCancelled else { return }
                setState(.countingDown(value))
                do {
                    try await Task.sleep(nanoseconds: 1_000_000_000)
                } catch {
                    if startToken == token {
                        setState(.idle)
                    }
                    return
                }
            }
        }
        guard startToken == token, !Task.isCancelled else { return }

        let engine = makeEngine(target, options)
        engine.onFailure = { [weak self] error in
            Task { @MainActor in
                guard let self, self.startToken == token else { return }
                self.activeEngine?.cancel()
                self.activeEngine = nil
                self.setState(.failed(error.localizedDescription))
            }
        }
        engine.onSecureFieldHiddenChange = { [weak self] hidden in
            Task { @MainActor in
                guard let self, self.startToken == token else { return }
                self.updateSecureFieldHidden(hidden)
            }
        }
        activeEngine = engine

        do {
            var runtime = try await engine.start()
            guard startToken == token else {
                engine.cancel()
                return
            }
            runtime.isSecureFieldHidden = latestSecureFieldHidden
            setState(.recording(runtime))
        } catch {
            guard startToken == token else { return }
            engine.cancel()
            activeEngine = nil
            setState(.failed(error.localizedDescription))
        }
    }

    func pause() {
        guard case var .recording(runtime) = state else { return }
        runtime.elapsed = max(runtime.elapsed, Date().timeIntervalSince(runtime.startedAt))
        activeEngine?.setPaused(true)
        setState(.paused(runtime))
    }

    func resume() {
        guard case var .paused(runtime) = state else { return }
        runtime.startedAt = Date().addingTimeInterval(-runtime.elapsed)
        activeEngine?.setPaused(false)
        setState(.recording(runtime))
    }

    func stop() {
        if case .finishing = state { return }
        let token = UUID()
        startToken = token
        guard let engine = activeEngine else {
            setState(.idle)
            return
        }
        setState(.finishing)
        finishTask?.cancel()
        finishTask = Task {
            do {
                let url = try await engine.stop()
                guard startToken == token else { return }
                if let activeEngine, activeEngine === engine {
                    self.activeEngine = nil
                }
                finishTask = nil
                setState(.reviewing(url))
            } catch {
                guard startToken == token else { return }
                engine.cancel()
                if let activeEngine, activeEngine === engine {
                    self.activeEngine = nil
                }
                finishTask = nil
                setState(.failed(error.localizedDescription))
            }
        }
    }

    func cancel() {
        startToken = UUID()
        latestSecureFieldHidden = false
        finishTask?.cancel()
        finishTask = nil
        activeEngine?.cancel()
        activeEngine = nil
        setState(.idle)
    }

    private func setState(_ newState: RecordingState) {
        state = newState
        onStateChange?(newState)
    }

    private func updateSecureFieldHidden(_ hidden: Bool) {
        latestSecureFieldHidden = hidden
        switch state {
        case var .recording(runtime):
            guard runtime.isSecureFieldHidden != hidden else { return }
            runtime.isSecureFieldHidden = hidden
            setState(.recording(runtime))
        case var .paused(runtime):
            guard runtime.isSecureFieldHidden != hidden else { return }
            runtime.isSecureFieldHidden = hidden
            setState(.paused(runtime))
        default:
            break
        }
    }
}
