import AppKit
import AVFoundation
import CoreImage
import CoreMedia
import Foundation
import ScreenCaptureKit

enum RecordingEngineError: LocalizedError, Equatable {
    case permissionDenied
    case noDisplayFound
    case noWindowFound
    case cannotCreateOutput
    case writerFailed(String)
    case streamFailed(String)
    case noFramesWritten

    var errorDescription: String? {
        switch self {
        case .permissionDenied:
            return "Screen Recording permission is required to record video."
        case .noDisplayFound:
            return "No display could be found for this recording."
        case .noWindowFound:
            return "No capturable window could be found for this recording."
        case .cannotCreateOutput:
            return "Could not create the recording file."
        case let .writerFailed(message):
            return "Recording writer failed. \(message)"
        case let .streamFailed(message):
            return "Screen recording failed. \(message)"
        case .noFramesWritten:
            return "The recording did not receive any video frames."
        }
    }
}

struct RecordingTargetDescriptor {
    let filter: SCContentFilter
    let configuration: SCStreamConfiguration
    let outputSettings: RecordingOutputSettings
    let sourceScreenRect: CGRect
    let targetDescription: String
}

final class RecordingEngine: NSObject, SCStreamOutput, SCStreamDelegate {
    private let target: RecordingTarget
    private let options: RecordingOptions
    private let outputURL: URL
    private let captureURL: URL
    private let sessionID = RecordingSessionID()
    private let writerQueue = DispatchQueue(label: "BelloBox.Recording.Writer")
    private let renderer = RecordingFrameRenderer()
    private let lifecycleLock = NSLock()

    private var stream: SCStream?
    private var writer: AVAssetWriter?
    private var videoInput: AVAssetWriterInput?
    private var pixelBufferAdaptor: AVAssetWriterInputPixelBufferAdaptor?
    private var systemAudioInput: AVAssetWriterInput?
    private var microphoneAudioInput: AVAssetWriterInput?
    private var microphoneCapture: MicrophoneSampleCapture?
    private var inputMonitor: RecordingInputMonitor?
    private var privacyGuard: PrivacyGuard?
    private var renderContext: RecordingFrameRenderContext?

    private var startedWriting = false
    private var startTime: CMTime?
    private var wroteVideoFrame = false
    private var finishInitiated = false
    private var discardOutputWhenFinished = false
    private var finishCompleted = false
    private var finishing = false
    private var paused = false
    private var lastSecureFieldHidden = false

    var onFailure: ((Error) -> Void)?
    var onSecureFieldHiddenChange: ((Bool) -> Void)?

    init(target: RecordingTarget, options: RecordingOptions, outputURL: URL = RecordingEngine.defaultOutputURL()) {
        self.target = target
        self.options = options
        self.outputURL = outputURL
        self.captureURL = options.audioSource == .microphoneAndSystemAudio
            ? RecordingEngine.intermediateOutputURL(for: outputURL)
            : outputURL
        super.init()
    }

    func start() async throws -> RecordingRuntimeState {
        guard ScreenCapturePermission.isTrusted else { throw RecordingEngineError.permissionDenied }
        let descriptor = try await Self.resolve(target: target, options: options)
        try prepareWriter(descriptor: descriptor)

        self.privacyGuard = PrivacyGuard(detector: PasswordFieldDetector(), options: options)
        let inputMonitor = RecordingInputMonitor(options: options, privacyGuard: privacyGuard)
        inputMonitor.start()
        self.inputMonitor = inputMonitor
        self.renderContext = RecordingFrameRenderContext(
            sourceScreenRect: descriptor.sourceScreenRect,
            outputSize: CGSize(width: descriptor.outputSettings.width, height: descriptor.outputSettings.height),
            clickOverlayMode: options.clickOverlayMode,
            keystrokeMode: options.keystrokeMode,
            secureFieldRedactionMode: options.secureFieldRedactionMode
        )

        let stream = SCStream(filter: descriptor.filter, configuration: descriptor.configuration, delegate: self)
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: writerQueue)
        if options.audioSource.includesSystemAudio {
            try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: writerQueue)
        }
        if options.audioSource.includesMicrophone {
            if #available(macOS 15.0, *) {
                try stream.addStreamOutput(self, type: .microphone, sampleHandlerQueue: writerQueue)
            } else {
                microphoneCapture = try MicrophoneSampleCapture(deviceID: options.microphoneDeviceID, queue: writerQueue) { [weak self] sampleBuffer in
                    self?.appendAudio(sampleBuffer, to: self?.microphoneAudioInput)
                }
                microphoneCapture?.start()
            }
        }
        self.stream = stream
        try await stream.startCapture()

        let runtime = RecordingRuntimeState(
            sessionID: sessionID,
            startedAt: Date(),
            targetDescription: descriptor.targetDescription,
            elapsed: 0,
            isMicEnabled: options.audioSource.includesMicrophone,
            isSystemAudioEnabled: options.audioSource.includesSystemAudio,
            isInputOverlayEnabled: options.clickOverlayMode.isEnabled || options.keystrokeMode != .off,
            isSecureFieldHidden: false
        )
        return runtime
    }

    func setPaused(_ paused: Bool) {
        writerQueue.async { [weak self] in
            self?.paused = paused
        }
    }

    func stop() async throws -> URL {
        markFinishing()
        inputMonitor?.stop()
        microphoneCapture?.stop()
        if let stream {
            try? await stream.stopCapture()
        }
        return try await finishWriting()
    }

    func cancel() {
        markFinishing()
        inputMonitor?.stop()
        microphoneCapture?.stop()
        Task { try? await stream?.stopCapture() }
        writerQueue.async { [weak self] in
            self?.cancelOrDiscardWriter()
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        guard !isFinishing else { return }
        onFailure?(RecordingEngineError.streamFailed(error.localizedDescription))
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
        guard !isFinishing, sampleBuffer.isValid else { return }
        switch outputType {
        case .screen:
            appendVideo(sampleBuffer)
        case .audio:
            appendAudio(sampleBuffer, to: systemAudioInput)
        default:
            if #available(macOS 15.0, *), outputType == .microphone {
                appendAudio(sampleBuffer, to: microphoneAudioInput)
            }
        }
    }

    private func prepareWriter(descriptor: RecordingTargetDescriptor) throws {
        try? FileManager.default.removeItem(at: captureURL)
        try? FileManager.default.removeItem(at: outputURL)
        try FileManager.default.createDirectory(at: captureURL.deletingLastPathComponent(), withIntermediateDirectories: true)

        let writer = try AVAssetWriter(outputURL: captureURL, fileType: .mov)
        let output = descriptor.outputSettings
        let videoSettings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: output.width,
            AVVideoHeightKey: output.height,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: output.videoBitrate,
                AVVideoExpectedSourceFrameRateKey: output.framesPerSecond,
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel
            ]
        ]
        let videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: videoSettings)
        videoInput.expectsMediaDataInRealTime = true
        guard writer.canAdd(videoInput) else { throw RecordingEngineError.writerFailed("Video input was rejected.") }
        writer.add(videoInput)

        let adaptor = AVAssetWriterInputPixelBufferAdaptor(
            assetWriterInput: videoInput,
            sourcePixelBufferAttributes: [
                kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
                kCVPixelBufferWidthKey as String: output.width,
                kCVPixelBufferHeightKey as String: output.height,
                kCVPixelBufferIOSurfacePropertiesKey as String: [:]
            ]
        )

        self.systemAudioInput = options.audioSource.includesSystemAudio
            ? addAudioInput(to: writer, output: output)
            : nil
        self.microphoneAudioInput = options.audioSource.includesMicrophone
            ? addAudioInput(to: writer, output: output)
            : nil
        self.writer = writer
        self.videoInput = videoInput
        self.pixelBufferAdaptor = adaptor
        self.finishInitiated = false
        lifecycleLock.lock()
        self.discardOutputWhenFinished = false
        self.finishCompleted = false
        lifecycleLock.unlock()
    }

    private func markFinishing() {
        lifecycleLock.lock()
        finishing = true
        lifecycleLock.unlock()
    }

    private var isFinishing: Bool {
        lifecycleLock.lock()
        let value = finishing
        lifecycleLock.unlock()
        return value
    }

    private func addAudioInput(to writer: AVAssetWriter, output: RecordingOutputSettings) -> AVAssetWriterInput? {
        let settings: [String: Any] = [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: output.audioSampleRate,
            AVNumberOfChannelsKey: output.audioChannelCount,
            AVEncoderBitRateKey: 128_000
        ]
        let input = AVAssetWriterInput(mediaType: .audio, outputSettings: settings)
        input.expectsMediaDataInRealTime = true
        guard writer.canAdd(input) else { return nil }
        writer.add(input)
        return input
    }

    private func appendVideo(_ sampleBuffer: CMSampleBuffer) {
        guard isCompleteFrame(sampleBuffer),
              let videoInput,
              let adaptor = pixelBufferAdaptor,
              let pool = adaptor.pixelBufferPool,
              let sourcePixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer)
        else { return }

        let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        startWriterIfNeeded(at: pts)
        guard startedWriting, !paused, videoInput.isReadyForMoreMediaData else { return }

        var renderedPixelBuffer: CVPixelBuffer?
        guard CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault, pool, &renderedPixelBuffer) == kCVReturnSuccess,
              let renderedPixelBuffer,
              let renderContext
        else { return }

        let sensitiveState = privacyGuard?.redactionState(now: pts) ?? .notSensitive
        updateSecureFieldHiddenIfNeeded(sensitiveState.isSensitive)
        let events = inputMonitor?.eventStore.activeEvents(at: pts) ?? []
        renderer.render(
            sourcePixelBuffer: sourcePixelBuffer,
            into: renderedPixelBuffer,
            context: renderContext,
            overlayEvents: events,
            sensitiveState: sensitiveState
        )

        if adaptor.append(renderedPixelBuffer, withPresentationTime: pts) {
            wroteVideoFrame = true
        }
    }

    private func appendAudio(_ sampleBuffer: CMSampleBuffer, to input: AVAssetWriterInput?) {
        guard startedWriting, !paused, let input, input.isReadyForMoreMediaData else { return }
        let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        if let startTime, CMTimeCompare(pts, startTime) < 0 { return }
        input.append(sampleBuffer)
    }

    private func startWriterIfNeeded(at time: CMTime) {
        guard !startedWriting, let writer else { return }
        if writer.startWriting() {
            writer.startSession(atSourceTime: time)
            startedWriting = true
            startTime = time
        } else {
            onFailure?(RecordingEngineError.writerFailed(writer.error?.localizedDescription ?? "Unknown writer error."))
        }
    }

    private func finishWriting() async throws -> URL {
        let capturedURL = try await finishWriter()
        guard options.audioSource == .microphoneAndSystemAudio else { return capturedURL }
        do {
            return try await RecordingAudioMixer.mixIfNeeded(sourceURL: capturedURL, destinationURL: outputURL)
        } catch {
            if capturedURL != outputURL {
                try? FileManager.default.removeItem(at: capturedURL)
            }
            throw error
        }
    }

    private func finishWriter() async throws -> URL {
        // AVAssetWriter cannot be cancelled once finishWriting has been issued.
        // Cancellation before the completion marks the result for discard; cancellation
        // after the completion removes the finished file. Exactly one side owns cleanup.
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                writerQueue.async { [weak self] in
                    guard let self, let writer = self.writer else {
                        continuation.resume(throwing: RecordingEngineError.cannotCreateOutput)
                        return
                    }

                    switch writer.status {
                    case .cancelled:
                        self.removeOutputFiles()
                        continuation.resume(throwing: CancellationError())
                        return
                    case .failed:
                        self.removeOutputFiles()
                        continuation.resume(
                            throwing: RecordingEngineError.writerFailed(writer.error?.localizedDescription ?? "Unknown writer error.")
                        )
                        return
                    default:
                        break
                    }

                    guard self.startedWriting, self.wroteVideoFrame else {
                        if writer.status == .writing, !self.finishInitiated {
                            writer.cancelWriting()
                        }
                        self.removeOutputFiles()
                        continuation.resume(throwing: RecordingEngineError.noFramesWritten)
                        return
                    }
                    self.videoInput?.markAsFinished()
                    self.systemAudioInput?.markAsFinished()
                    self.microphoneAudioInput?.markAsFinished()
                    self.finishInitiated = true
                    writer.finishWriting {
                        self.writerQueue.async {
                            let shouldDiscard = self.markFinishCompleted()
                            switch writer.status {
                            case .completed:
                                if shouldDiscard {
                                    self.removeOutputFiles()
                                    continuation.resume(throwing: CancellationError())
                                } else {
                                    continuation.resume(returning: self.captureURL)
                                }
                            case .cancelled:
                                self.removeOutputFiles()
                                continuation.resume(throwing: CancellationError())
                            default:
                                self.removeOutputFiles()
                                continuation.resume(
                                    throwing: RecordingEngineError.writerFailed(writer.error?.localizedDescription ?? "Unknown writer error.")
                                )
                            }
                        }
                    }
                }
            }
        } onCancel: {
            writerQueue.async { [weak self] in
                self?.cancelOrDiscardWriter()
            }
        }
    }

    private func isCompleteFrame(_ sampleBuffer: CMSampleBuffer) -> Bool {
        guard let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let rawStatus = attachments.first?[SCStreamFrameInfo.status] as? Int,
              let status = SCFrameStatus(rawValue: rawStatus)
        else { return true }
        return status == .complete || status == .started
    }

    private func updateSecureFieldHiddenIfNeeded(_ isHidden: Bool) {
        guard lastSecureFieldHidden != isHidden else { return }
        lastSecureFieldHidden = isHidden
        onSecureFieldHiddenChange?(isHidden)
    }

    private static func resolve(target: RecordingTarget, options: RecordingOptions) async throws -> RecordingTargetDescriptor {
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        switch target {
        case let .display(displayID):
            guard let display = content.displays.first(where: { $0.displayID == displayID }),
                  let screen = screen(for: displayID)
            else { throw RecordingEngineError.noDisplayFound }
            let targetPixelSize = ScreenCoordinateSpace.displayPixelSize(for: displayID, fallbackScreen: screen)
            return descriptor(
                display: display,
                screen: screen,
                sourceRect: nil,
                targetPixelSize: targetPixelSize,
                sourceScreenRect: screen.frame,
                targetDescription: "Screen",
                content: content,
                options: options
            )
        case let .area(displayID, rect):
            guard let display = content.displays.first(where: { $0.displayID == displayID }),
                  let screen = screen(for: displayID)
            else { throw RecordingEngineError.noDisplayFound }
            let boundedRect = rect.intersection(screen.frame).standardized
            guard boundedRect.width >= 1, boundedRect.height >= 1 else {
                throw RecordingEngineError.streamFailed("The selected recording area was outside the display bounds.")
            }
            let localRect = RegionCaptureGeometry.globalCocoaRectToLocalFlipped(boundedRect, screenFrame: screen.frame)
            let targetPixelSize = ScreenCoordinateSpace.pixelSize(
                forCocoaSize: boundedRect.size,
                screenFrame: screen.frame,
                displayPixelSize: ScreenCoordinateSpace.displayPixelSize(for: displayID, fallbackScreen: screen)
            )
            return descriptor(
                display: display,
                screen: screen,
                sourceRect: localRect,
                targetPixelSize: targetPixelSize,
                sourceScreenRect: boundedRect,
                targetDescription: "Area",
                content: content,
                options: options
            )
        case let .window(windowID, _, frame):
            guard let window = content.windows.first(where: { $0.windowID == windowID }) else {
                throw RecordingEngineError.noWindowFound
            }
            let sourceFrame = frame ?? ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(window.frame)
            let targetSize = windowTargetPixelSize(for: sourceFrame)
            let output = RecordingOutputSettings.make(for: targetSize, quality: options.quality)
            let configuration = configuration(output: output, options: options)
            let filter = SCContentFilter(desktopIndependentWindow: window)
            return RecordingTargetDescriptor(
                filter: filter,
                configuration: configuration,
                outputSettings: output,
                sourceScreenRect: sourceFrame,
                targetDescription: "Window"
            )
        }
    }

    private static func windowTargetPixelSize(for frame: CGRect) -> CGSize {
        guard let screen = ScreenCoordinateSpace.displayForCocoaRect(frame),
              let displayID = ScreenCoordinateSpace.displayID(for: screen)
        else {
            return CGSize(width: max(2, frame.width * 2), height: max(2, frame.height * 2))
        }
        return ScreenCoordinateSpace.pixelSize(
            forCocoaSize: frame.size,
            screenFrame: screen.frame,
            displayPixelSize: ScreenCoordinateSpace.displayPixelSize(for: displayID, fallbackScreen: screen)
        )
    }

    private static func descriptor(
        display: SCDisplay,
        screen: NSScreen,
        sourceRect: CGRect?,
        targetPixelSize: CGSize,
        sourceScreenRect: CGRect,
        targetDescription: String,
        content: SCShareableContent,
        options: RecordingOptions
    ) -> RecordingTargetDescriptor {
        let output = RecordingOutputSettings.make(for: targetPixelSize, quality: options.quality)
        let configuration = configuration(output: output, options: options)
        if let sourceRect {
            configuration.sourceRect = sourceRect
        }
        let ownPID = ProcessInfo.processInfo.processIdentifier
        let excludedApplications = options.excludeBelloBoxWindows
            ? content.applications.filter { $0.processID == ownPID }
            : []
        let filter = SCContentFilter(
            display: display,
            excludingApplications: excludedApplications,
            exceptingWindows: []
        )
        return RecordingTargetDescriptor(
            filter: filter,
            configuration: configuration,
            outputSettings: output,
            sourceScreenRect: sourceScreenRect,
            targetDescription: targetDescription
        )
    }

    private static func configuration(output: RecordingOutputSettings, options: RecordingOptions) -> SCStreamConfiguration {
        let configuration = SCStreamConfiguration()
        configuration.width = output.width
        configuration.height = output.height
        configuration.pixelFormat = kCVPixelFormatType_32BGRA
        configuration.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(output.framesPerSecond))
        configuration.queueDepth = 6
        configuration.showsCursor = options.includeCursor
        configuration.capturesAudio = options.audioSource.includesSystemAudio
        configuration.sampleRate = Int(output.audioSampleRate)
        configuration.channelCount = output.audioChannelCount
        configuration.excludesCurrentProcessAudio = options.excludesCurrentProcessAudio
        if #available(macOS 14.0, *) {
            configuration.preservesAspectRatio = true
            configuration.captureResolution = .nominal
        }
        if #available(macOS 15.0, *) {
            configuration.captureMicrophone = options.audioSource.includesMicrophone
            configuration.microphoneCaptureDeviceID = options.microphoneDeviceID
        }
        return configuration
    }

    private static func screen(for displayID: CGDirectDisplayID) -> NSScreen? {
        NSScreen.screens.first { ScreenCoordinateSpace.displayID(for: $0) == displayID }
    }

    private static func defaultOutputURL() -> URL {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH.mm.ss"
        let name = "Bello Box Recording \(formatter.string(from: Date())).mov"
        return FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxRecordings", isDirectory: true)
            .appendingPathComponent(name)
    }

    private static func intermediateOutputURL(for outputURL: URL) -> URL {
        let baseName = outputURL.deletingPathExtension().lastPathComponent
        return outputURL.deletingLastPathComponent()
            .appendingPathComponent("\(baseName)-tracks-\(UUID().uuidString).mov")
    }

    private func removeOutputFiles() {
        if captureURL != outputURL {
            try? FileManager.default.removeItem(at: captureURL)
        }
        try? FileManager.default.removeItem(at: outputURL)
    }

    private func markFinishCompleted() -> Bool {
        lifecycleLock.lock()
        finishCompleted = true
        let shouldDiscard = discardOutputWhenFinished
        lifecycleLock.unlock()
        return shouldDiscard
    }

    private func requestDiscardAfterFinish() -> Bool {
        lifecycleLock.lock()
        if finishCompleted {
            lifecycleLock.unlock()
            return true
        }
        discardOutputWhenFinished = true
        lifecycleLock.unlock()
        return false
    }

    private func cancelOrDiscardWriter() {
        guard let writer else {
            removeOutputFiles()
            return
        }
        if finishInitiated {
            if requestDiscardAfterFinish() {
                removeOutputFiles()
            }
            return
        }
        if writer.status == .writing {
            writer.cancelWriting()
        }
        removeOutputFiles()
    }
}

private final class MicrophoneSampleCapture: NSObject, AVCaptureAudioDataOutputSampleBufferDelegate {
    private let session = AVCaptureSession()
    private let onSampleBuffer: (CMSampleBuffer) -> Void

    init(deviceID: String?, queue: DispatchQueue, onSampleBuffer: @escaping (CMSampleBuffer) -> Void) throws {
        self.onSampleBuffer = onSampleBuffer
        super.init()

        guard let device = Self.device(matching: deviceID) else {
            throw RecordingEngineError.streamFailed("No microphone device is available.")
        }
        let input = try AVCaptureDeviceInput(device: device)
        guard session.canAddInput(input) else {
            throw RecordingEngineError.streamFailed("Could not add microphone input.")
        }
        session.addInput(input)

        let output = AVCaptureAudioDataOutput()
        output.setSampleBufferDelegate(self, queue: queue)
        guard session.canAddOutput(output) else {
            throw RecordingEngineError.streamFailed("Could not add microphone output.")
        }
        session.addOutput(output)
    }

    func start() {
        session.startRunning()
    }

    func stop() {
        session.stopRunning()
    }

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        onSampleBuffer(sampleBuffer)
    }

    private static func device(matching deviceID: String?) -> AVCaptureDevice? {
        RecordingMicrophoneDevices.device(matching: deviceID)
    }
}
