import AppKit
import Combine
import CoreGraphics

/// Drives "scroll to capture more": samples the target area on a timer, appends a frame
/// whenever the content has scrolled and settled (or scrolled far enough that waiting
/// would lose the overlap with the previous frame), optionally scrolls the content
/// itself, and stitches the frames into one tall image on demand.
@MainActor
final class ScrollCaptureEngine: ObservableObject {
    enum Phase: Equatable {
        case idle
        case watching
        case stitching
        case finished
        case failed(String)
    }

    struct Configuration: Equatable {
        /// Seconds between two samples of the target area.
        var sampleInterval: TimeInterval = 0.14
        /// Mean absolute difference (0...1) above which a sample counts as moved since the
        /// last appended frame. Low enough that sparse, mostly white pages still register.
        var changeThreshold: Double = 0.01
        /// Mean absolute difference below which two consecutive samples count as settled.
        var settleThreshold: Double = 0.004
        /// Append right away when the confident overlap with the last frame drops below
        /// this fraction of the frame height, so fast scrolling never loses continuity.
        var eagerOverlapFraction: Double = 0.45
        /// A settled sample with no confident overlap in either direction (a jump of more
        /// than a page, or content that changed in place) is only appended when it differs
        /// this much from the last frame; hover highlights and expanded widgets stay out.
        var unmatchedChangeThreshold: Double = 0.08
        /// A matched sample must add at least this many new rows below the last frame;
        /// smaller shifts (or an in-place change that aligns at zero shift) are ignored.
        var minimumNewRows: Int = 8
        /// Fraction of the area height scrolled per auto-scroll step.
        var autoScrollFraction: CGFloat = 0.6
        /// Minimum seconds to wait for a new frame after an auto-scroll step before
        /// concluding that nothing moved. The wait also requires a few processed samples
        /// with nothing pending, and never exceeds four times this value.
        var autoScrollTimeout: TimeInterval = 1.4
        /// Samples that must be processed after a step before it counts as "no progress".
        var autoScrollMinimumSamples: Int = 3
        var maxFrames: Int = 20
        /// Frames captured automatically can overlap far more than hand-picked ones, so
        /// the overlap search reaches almost the whole frame.
        var stitch: StitchConfig = {
            var config = StitchConfig.default
            // Reach almost the whole frame so even tiny scrolls are matched at their exact
            // offset instead of at a nearby, slightly misaligned candidate.
            config.maxOverlapFraction = 0.985
            return config
        }()
    }

    @Published private(set) var frames: [CGImage]
    @Published private(set) var phase: Phase = .idle
    @Published private(set) var isAutoScrolling = false
    @Published private(set) var reachedEnd = false
    @Published private(set) var message: String?

    let area: CaptureArea
    let summary: ScrollCaptureTargetSummary
    let configuration: Configuration

    private let captureSample: () async throws -> CGImage
    private let postScroll: (CGFloat) -> Void
    private let isAccessibilityTrusted: () -> Bool
    private var samplingTask: Task<Void, Never>?
    private var autoScrollTask: Task<Void, Never>?
    private var lastSample: CGImage?
    /// +1 scrolls "down" with a negative wheel delta; flipped once if that moves the
    /// content the wrong way (natural scrolling) or nothing happens.
    private var scrollDirectionSign: CGFloat = 1
    private var sawReverseScroll = false
    private var samplesSincePost = 0
    /// A sample after the last auto-scroll step showed new content that is still
    /// waiting to settle, so the step must not be judged yet.
    private var pendingMoveSincePost = false
#if DEBUG
    /// Chronological trace of decisions, for e2e markers and debugging.
    private(set) var debugEvents: [String] = []
#endif

    private func trace(_ event: @autoclosure () -> String) {
#if DEBUG
        debugEvents.append(event())
#endif
    }

    /// `initialFrame` is normally the frozen crop the editor shows; pass nil to let the
    /// first live sample become the first frame (e.g. when the region had to be clamped
    /// to the display and no matching frozen crop exists).
    init(
        area: CaptureArea,
        summary: ScrollCaptureTargetSummary,
        initialFrame: CGImage?,
        configuration: Configuration = Configuration(),
        captureSample: @escaping () async throws -> CGImage,
        postScroll: @escaping (CGFloat) -> Void,
        isAccessibilityTrusted: @escaping () -> Bool = { AccessibilityService.isTrusted }
    ) {
        self.area = area
        self.summary = summary
        self.frames = initialFrame.map { [$0] } ?? []
        self.configuration = configuration
        self.captureSample = captureSample
        self.postScroll = postScroll
        self.isAccessibilityTrusted = isAccessibilityTrusted
    }

    /// Production wiring: samples only the selected region and scrolls with synthetic
    /// wheel events posted at the centre of the area.
    convenience init(
        area: CaptureArea,
        summary: ScrollCaptureTargetSummary,
        initialFrame: CGImage?,
        pixelSize: CGSize,
        service: ScreenCaptureService,
        settings: AppSettings
    ) {
        var configuration = Configuration()
        configuration.maxFrames = max(2, settings.scrollingScreenshotMaxFrames)
        configuration.stitch.removeRepeatedHeaderFooter = settings.scrollingScreenshotAutoCompact
        let center = CGPoint(x: area.cocoaRect.midX, y: area.cocoaRect.midY)
        self.init(
            area: area,
            summary: summary,
            initialFrame: initialFrame,
            configuration: configuration,
            captureSample: { try await service.captureRegionImage(area, pixelSize: pixelSize) },
            postScroll: { points in Self.postScrollWheel(points: points, at: center) }
        )
    }

    var canFinish: Bool { !frames.isEmpty && (phase == .watching || phase == .idle) }

    func start() {
        guard phase == .idle || phase == .watching else { return }
        phase = .watching
        guard samplingTask == nil else { return }
        samplingTask = Task { [weak self] in
            await self?.sampleLoop()
        }
    }

    func stop() {
        stopAutoScroll()
        samplingTask?.cancel()
        samplingTask = nil
        if phase == .watching {
            phase = .idle
        }
    }

    /// Resumes watching after a failed stitch so the user can try again or cancel.
    func resumeWatching() {
        guard case .failed = phase else { return }
        phase = .idle
        start()
    }

    func toggleAutoScroll() {
        if isAutoScrolling {
            stopAutoScroll()
        } else {
            startAutoScroll()
        }
    }

    /// Stops sampling and stitches every frame captured so far. If the result would be
    /// taller than the stitcher allows, trailing frames are dropped until it fits.
    func finish() async throws -> ScreenshotDocument {
        stopAutoScroll()
        samplingTask?.cancel()
        samplingTask = nil
        phase = .stitching
        var frames = self.frames
        let config = configuration.stitch
        do {
            var droppedFrames = 0
            while true {
                let attempt = frames
                let stitchTask = Task.detached(priority: .userInitiated) {
                    try ImageStitcher.stitch(attempt, config: config)
                }
                do {
                    var result = try await withTaskCancellationHandler {
                        try await stitchTask.value
                    } onCancel: {
                        stitchTask.cancel()
                    }
                    try Task.checkCancellation()
                    if droppedFrames > 0 {
                        result.warnings.append("The last \(droppedFrames) frame\(droppedFrames == 1 ? " was" : "s were") left out to keep the screenshot within the maximum height.")
                    }
                    phase = .finished
                    return Self.makeDocument(from: result, target: summary, frameCount: attempt.count)
                } catch StitchError.outputTooTall where frames.count > 1 {
                    frames.removeLast()
                    droppedFrames += 1
                }
            }
        } catch {
            phase = .failed(error.localizedDescription)
            throw error
        }
    }

    /// Feeds one sample of the target area. Exposed so tests can drive the engine
    /// without a timer.
    func processSample(_ sample: CGImage) {
        guard phase == .watching, frames.count < configuration.maxFrames else { return }
        samplesSincePost += 1
        guard let last = frames.last else {
            // No frozen crop to start from: the first live sample opens the sequence.
            lastSample = sample
            append(sample)
            return
        }
        guard sample.width == last.width, sample.height == last.height else {
            message = "The capture area changed size; scrolling capture stopped."
            stop()
            return
        }
        defer { lastSample = sample }

        // Still showing the last appended frame: nothing to do.
        if ImageStitcher.appearsUnchanged(previous: last, current: sample, threshold: configuration.changeThreshold) {
            trace("unchanged")
            return
        }

        let settled = lastSample.map {
            ImageStitcher.appearsUnchanged(previous: $0, current: sample, threshold: configuration.settleThreshold)
        } ?? false
        // A sticky header repeats at the top of every frame; skip it when matching.
        let header = configuration.stitch.removeRepeatedHeaderFooter
            ? (frames.first.flatMap { ImageStitcher.repeatedHeaderHeight(first: $0, current: sample) } ?? 0)
            : 0
        let match = ImageStitcher.bestOverlap(previous: last, current: sample, config: configuration.stitch, skippingTopRows: header)
        let shouldAppend: Bool
        if let match {
            let newRows = sample.height - header - match.overlap
            guard newRows >= configuration.minimumNewRows else {
                // Aligned at (almost) zero shift: an in-place change or a negligible
                // scroll. Not worth a frame; keep comparing against the last frame.
                pendingMoveSincePost = false
                trace("negligible overlap=\(match.overlap) newRows=\(newRows)")
                return
            }
            // The content scrolled and can be placed: append once it settles, or right
            // away if waiting longer risks losing the overlap with the last frame.
            let overlapFraction = Double(match.overlap) / Double(max(1, sample.height))
            shouldAppend = settled || overlapFraction < configuration.eagerOverlapFraction
            pendingMoveSincePost = !shouldAppend
            trace("moved overlap=\(match.overlap) settled=\(settled) append=\(shouldAppend)")
        } else if ImageStitcher.bestOverlap(previous: sample, current: last, config: configuration.stitch, skippingTopRows: header) != nil {
            // The content moved the other way. Frames must progress downward, so ignore
            // it; auto-scroll uses this to flip its direction.
            trace("reverse")
            sawReverseScroll = true
            if !isAutoScrolling {
                message = "Scroll down to capture more."
            }
            return
        } else {
            // No confident overlap (scrolled more than a page, or the content changed in
            // place): only accept it once it stops changing and differs substantially.
            let difference = ImageStitcher.meanAbsoluteDifference(previous: last, current: sample) ?? 0
            shouldAppend = settled && difference >= configuration.unmatchedChangeThreshold
            pendingMoveSincePost = !settled
            trace("noOverlap settled=\(settled) diff=\(String(format: "%.3f", difference)) append=\(shouldAppend)")
        }
        guard shouldAppend else { return }
        append(sample)
    }

    private func append(_ frame: CGImage) {
        frames.append(frame)
        pendingMoveSincePost = false
        trace("append#\(frames.count)")
        message = nil
        if frames.count >= configuration.maxFrames {
            message = "Maximum of \(configuration.maxFrames) frames reached. Press Done to stitch."
            stopAutoScroll()
            samplingTask?.cancel()
            samplingTask = nil
        }
    }

    private func sampleLoop() async {
        while !Task.isCancelled {
            try? await Task.sleep(nanoseconds: UInt64(configuration.sampleInterval * 1_000_000_000))
            guard !Task.isCancelled, phase == .watching else { return }
            let sample: CGImage
            do {
                sample = try await captureSample()
            } catch is CancellationError {
                return
            } catch {
                guard !Task.isCancelled else { return }
                message = error.localizedDescription
                continue
            }
            guard !Task.isCancelled, phase == .watching else { return }
            processSample(sample)
        }
    }

    private func startAutoScroll() {
        guard phase == .watching, !isAutoScrolling, frames.count < configuration.maxFrames else { return }
        guard isAccessibilityTrusted() else {
            message = "Auto-scroll needs Accessibility permission. Scroll the content yourself instead."
            return
        }
        isAutoScrolling = true
        reachedEnd = false
        message = nil
        autoScrollTask = Task { [weak self] in
            guard let self else { return }
            let step = self.area.cocoaRect.height * self.configuration.autoScrollFraction
            var flippedDirection = false
            while !Task.isCancelled, self.isAutoScrolling, self.phase == .watching, self.frames.count < self.configuration.maxFrames {
                let before = self.frames.count
                let postedSign = self.scrollDirectionSign
                self.sawReverseScroll = false
                self.samplesSincePost = 0
                self.pendingMoveSincePost = false
                self.trace("autoScroll step=\(Int(step)) sign=\(Int(postedSign))")
                self.postScroll(step * postedSign)
                await self.waitForStepOutcome(framesBefore: before)
                if Task.isCancelled || self.phase != .watching { break }
                if self.frames.count > before { continue }
                if !flippedDirection {
                    // Either the wheel direction is inverted (natural scrolling) or the
                    // content was already at its start: try the other way once.
                    self.trace(self.sawReverseScroll ? "flip afterReverse" : "flip afterTimeout")
                    flippedDirection = true
                    self.scrollDirectionSign = -self.scrollDirectionSign
                    if self.sawReverseScroll {
                        // Undo the step that went the wrong way before carrying on.
                        await self.undoStep(step, postedSign: postedSign)
                    }
                    continue
                }
                if self.sawReverseScroll {
                    // The probe in the other direction moved the content back up; put it
                    // where the user left it before reporting the end.
                    await self.undoStep(step, postedSign: postedSign)
                }
                self.trace("end")
                self.reachedEnd = true
                self.message = "Reached the end of the content. Press Done to stitch."
                break
            }
            self.isAutoScrolling = false
            self.autoScrollTask = nil
        }
    }

    /// Returns once the step produced a frame, moved the content backwards, or clearly
    /// did nothing: enough samples were processed with nothing pending after the soft
    /// timeout. A hard ceiling protects against stalled sampling.
    private func waitForStepOutcome(framesBefore: Int) async {
        let start = Date()
        let soft = start.addingTimeInterval(configuration.autoScrollTimeout)
        let hard = start.addingTimeInterval(configuration.autoScrollTimeout * 4)
        while !Task.isCancelled, frames.count == framesBefore, !sawReverseScroll, phase == .watching {
            let now = Date()
            if now >= hard { break }
            if now >= soft, samplesSincePost >= configuration.autoScrollMinimumSamples, !pendingMoveSincePost { break }
            try? await Task.sleep(nanoseconds: 60_000_000)
        }
    }

    /// Scrolls one step opposite to the step that was posted with `postedSign` and waits
    /// until the content shows the last frame again, or gives up after a few samples.
    private func undoStep(_ step: CGFloat, postedSign: CGFloat) async {
        trace("undo")
        samplesSincePost = 0
        pendingMoveSincePost = false
        sawReverseScroll = false
        postScroll(step * -postedSign)
        let hard = Date().addingTimeInterval(configuration.autoScrollTimeout * 2)
        while !Task.isCancelled, phase == .watching, Date() < hard {
            if samplesSincePost >= 2, !pendingMoveSincePost { break }
            try? await Task.sleep(nanoseconds: 60_000_000)
        }
        sawReverseScroll = false
    }

    private func stopAutoScroll() {
        autoScrollTask?.cancel()
        autoScrollTask = nil
        isAutoScrolling = false
    }

    /// Posts a scroll-wheel event of `points` (positive scrolls the content down) at a
    /// screen point in Cocoa coordinates.
    static func postScrollWheel(points: CGFloat, at cocoaPoint: CGPoint) {
        guard let event = CGEvent(
            scrollWheelEvent2Source: nil,
            units: .pixel,
            wheelCount: 1,
            wheel1: Int32(-points.rounded()),
            wheel2: 0,
            wheel3: 0
        ) else { return }
        event.location = ScreenCoordinateSpace.cocoaPointToTopLeftPoint(cocoaPoint)
        event.post(tap: .cghidEventTap)
    }

    nonisolated static func makeDocument(
        from result: StitchResult,
        target: ScrollCaptureTargetSummary,
        frameCount: Int,
        createdAt: Date = Date()
    ) -> ScreenshotDocument {
        let warningResult = result.warnings.isEmpty ? nil : OCRResult(
            id: UUID(),
            engine: .appleVision(revision: nil, recognitionLevel: .accurate),
            target: .fullImage,
            plainText: "",
            markdownText: nil,
            regions: [],
            languageHints: [],
            imageDigest: "",
            warnings: result.warnings,
            createdAt: createdAt
        )
        return ScreenshotDocument(
            baseImage: result.image,
            scale: 1,
            source: .scrolling(target: target, frameCount: frameCount),
            ocrResults: warningResult.map { [$0] } ?? [],
            activeOCRResultID: warningResult?.id,
            createdAt: createdAt
        )
    }
}
