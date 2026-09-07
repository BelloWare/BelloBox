import AppKit
import Combine
import Foundation

protocol RecordingEngineControlling: AnyObject {
    var onFailure: ((Error) -> Void)? { get set }
    var onSecureFieldHiddenChange: ((Bool) -> Void)? { get set }

    func start() async throws -> RecordingRuntimeState
    func setPaused(_ paused: Bool)
    func updateInputOverlays(clicks: ClickOverlayMode, keys: KeystrokeCaptureMode) -> Bool
    func stop() async throws -> URL
    func cancel()
}

extension RecordingEngine: RecordingEngineControlling {}

@MainActor
final class RecordingCoordinator: ObservableObject {
    @Published private(set) var state: RecordingState = .idle

    /// Movie → GIF conversion with progress 0…1; injectable for tests.
    typealias GIFTranscode = (URL, URL, GIFExportOptions, @escaping GIFTranscoder.Progress) async throws -> GIFExportResult

    private let settings: AppSettings
    private let makeEngine: (RecordingTarget, RecordingOptions) -> any RecordingEngineControlling
    private let permissionProvider: (RecordingOptions) -> RecordingPermissionState
    private let transcodeGIF: GIFTranscode
    private var activeEngine: (any RecordingEngineControlling)?
    private var activeOptions: RecordingOptions?
    private var finishTask: Task<Void, Never>?
    private var conversionTask: Task<Void, Never>?
    private var startToken = UUID()
    private var latestSecureFieldHidden = false

    var onStateChange: ((RecordingState) -> Void)?

    init(
        settings: AppSettings,
        makeEngine: @escaping (RecordingTarget, RecordingOptions) -> any RecordingEngineControlling = {
            RecordingEngine(target: $0, options: $1)
        },
        permissionProvider: @escaping (RecordingOptions) -> RecordingPermissionState = RecordingPermissionState.current(options:),
        transcodeGIF: @escaping GIFTranscode = { try await GIFTranscoder.transcode(sourceURL: $0, to: $1, options: $2, progress: $3) }
    ) {
        self.settings = settings
        self.makeEngine = makeEngine
        self.permissionProvider = permissionProvider
        self.transcodeGIF = transcodeGIF
    }

    var isRecording: Bool {
        switch state {
        case .recording, .paused, .countingDown, .finishing, .convertingToGIF:
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
        guard !Task.isCancelled else { return }
        let token = UUID()
        startToken = token
        latestSecureFieldHidden = false
        finishTask?.cancel()
        finishTask = nil
        conversionTask?.cancel()
        conversionTask = nil
        activeEngine?.cancel()
        activeEngine = nil
        activeOptions = options
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
                self.startToken = UUID()
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
            var runtime = try await withTaskCancellationHandler {
                try await engine.start()
            } onCancel: {
                Task { @MainActor [weak self] in
                    guard let self, self.startToken == token else { return }
                    self.cancel()
                }
            }
            guard startToken == token else {
                engine.cancel()
                return
            }
            guard !Task.isCancelled else {
                cancel()
                return
            }
            runtime.isSecureFieldHidden = latestSecureFieldHidden
            runtime.outputFormat = options.outputFormat
            setState(.recording(runtime))
        } catch {
            guard startToken == token else { return }
            engine.cancel()
            activeEngine = nil
            startToken = UUID()
            if error is CancellationError || Task.isCancelled {
                setState(.idle)
            } else {
                setState(.failed(error.localizedDescription))
            }
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

    func updateInputOverlays(clicks: ClickOverlayMode, keys: KeystrokeCaptureMode) {
        var runtime: RecordingRuntimeState
        let paused: Bool
        switch state {
        case let .recording(value): (runtime, paused) = (value, false)
        case let .paused(value): (runtime, paused) = (value, true)
        default: return
        }
        let available = activeEngine?.updateInputOverlays(clicks: clicks, keys: keys) == true
        runtime.clickOverlayMode = available ? clicks : .off
        runtime.keystrokeMode = available ? keys : .off
        runtime.isInputOverlayEnabled = available && (clicks.isEnabled || keys != .off)
        runtime.inputOverlayWarning = available ? nil : "Input tracking could not start. Check Input Monitoring permission in System Settings."
        setState(paused ? .paused(runtime) : .recording(runtime))
    }

    func stop() {
        if case .finishing = state { return }
        if case .convertingToGIF = state { return }
        let token = UUID()
        startToken = token
        guard let engine = activeEngine else {
            setState(.idle)
            return
        }
        let options = activeOptions
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
                if let options, options.outputFormat == .gif {
                    convertToGIF(movie: url, options: options.gif, token: token)
                } else {
                    setState(.reviewing(url))
                }
            } catch let RecordingEngineError.recoverableExportFailure(url, message) {
                guard startToken == token else { return }
                self.activeEngine = nil
                finishTask = nil
                setState(.reviewing(url, warning: "The original recording was preserved because final export failed. \(message) Use Save As to keep a copy; audio may remain on separate tracks."))
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
        conversionTask?.cancel()
        conversionTask = nil
        activeEngine?.cancel()
        activeEngine = nil
        activeOptions = nil
        setState(.idle)
    }

    /// The movie is complete on disk before conversion starts, so this only ever
    /// decides what the review shows: the GIF, or the movie with a note.
    private func convertToGIF(movie: URL, options: GIFExportOptions, token: UUID) {
        setState(.convertingToGIF(progress: 0))
        let destination = movie.deletingPathExtension().appendingPathExtension("gif")
        conversionTask?.cancel()
        conversionTask = Task { [weak self] in
            guard let self else { return }
            do {
                let reporter = ProgressThrottle()
                let result = try await self.transcodeGIF(movie, destination, options) { progress in
                    guard reporter.shouldReport(progress) else { return }
                    Task { @MainActor [weak self] in
                        guard let self, self.startToken == token, case .convertingToGIF = self.state else { return }
                        self.setState(.convertingToGIF(progress: min(max(progress, 0), 1)))
                    }
                }
                guard self.startToken == token else { return }
                self.conversionTask = nil
                self.setState(.reviewing(movie, gif: result.url))
            } catch is CancellationError {
                guard self.startToken == token else { return }
                self.conversionTask = nil
                self.setState(.reviewing(movie, warning: "GIF conversion was cancelled. The movie recording is kept; you can make a GIF from it here."))
            } catch {
                guard self.startToken == token else { return }
                self.conversionTask = nil
                self.setState(.reviewing(movie, warning: "The GIF could not be written: \(error.localizedDescription) The movie recording is kept."))
            }
        }
    }

    /// Stops writing the GIF and reviews the movie instead. The movie is untouched.
    func cancelGIFConversion() {
        guard case .convertingToGIF = state, let task = conversionTask else { return }
        task.cancel()
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

/// Forwards progress only when it moved by at least a percent (or reached the
/// end), so thousands of frame callbacks do not become thousands of UI updates.
final class ProgressThrottle: @unchecked Sendable {
    private let lock = NSLock()
    private var last: Double = -1
    private let step: Double

    init(step: Double = 0.01) { self.step = step }

    func shouldReport(_ progress: Double) -> Bool {
        lock.lock(); defer { lock.unlock() }
        guard progress >= 1 || progress - last >= step else { return false }
        last = progress
        return true
    }
}
