import AppKit
import Combine
import Foundation

@MainActor
final class RecordingCoordinator: ObservableObject {
    @Published private(set) var state: RecordingState = .idle

    private let settings: AppSettings
    private var activeEngine: RecordingEngine?
    private var startToken = UUID()

    var onStateChange: ((RecordingState) -> Void)?

    init(settings: AppSettings) {
        self.settings = settings
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
        RecordingPermissionState.current(options: options)
    }

    func start(target: RecordingTarget, options: RecordingOptions) async {
        let token = UUID()
        startToken = token
        activeEngine?.cancel()
        activeEngine = nil
        let permissionState = RecordingPermissionState.current(options: options)
        guard permissionState.canRecordVideo else {
            setState(.failed("Screen Recording permission is required to record video."))
            return
        }

        let seconds = max(0, options.countdownSeconds)
        if seconds > 0 {
            for value in stride(from: seconds, through: 1, by: -1) {
                guard startToken == token else { return }
                setState(.countingDown(value))
                try? await Task.sleep(nanoseconds: 1_000_000_000)
            }
        }
        guard startToken == token else { return }

        let engine = RecordingEngine(target: target, options: options)
        engine.onFailure = { [weak self] error in
            Task { @MainActor in
                self?.activeEngine?.cancel()
                self?.activeEngine = nil
                self?.setState(.failed(error.localizedDescription))
            }
        }
        activeEngine = engine

        do {
            let runtime = try await engine.start()
            guard startToken == token else {
                engine.cancel()
                return
            }
            setState(.recording(runtime))
        } catch {
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
        startToken = UUID()
        guard let engine = activeEngine else {
            setState(.idle)
            return
        }
        setState(.finishing)
        activeEngine = nil
        Task {
            do {
                let url = try await engine.stop()
                setState(.reviewing(url))
            } catch {
                engine.cancel()
                setState(.failed(error.localizedDescription))
            }
        }
    }

    func cancel() {
        startToken = UUID()
        activeEngine?.cancel()
        activeEngine = nil
        setState(.idle)
    }

    private func setState(_ newState: RecordingState) {
        state = newState
        onStateChange?(newState)
    }
}
