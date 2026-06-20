import AppKit
import Combine
import CoreGraphics

@MainActor
final class ScrollCaptureCoordinator: ObservableObject {
    @Published var session: ScrollCaptureSession
    @Published var warning: String?

    private let service: ScreenCaptureService
    private let settings: AppSettings

    var settingsMaxFrames: Int { settings.scrollingScreenshotMaxFrames }

    init(target: ScrollCaptureTarget, service: ScreenCaptureService, settings: AppSettings) {
        self.session = ScrollCaptureSession(
            target: target,
            direction: .down,
            config: StitchConfig(
                direction: .down,
                minOverlapPx: StitchConfig.default.minOverlapPx,
                maxOverlapFraction: StitchConfig.default.maxOverlapFraction,
                downsampleWidth: StitchConfig.default.downsampleWidth,
                scoreThreshold: StitchConfig.default.scoreThreshold,
                removeRepeatedHeaderFooter: settings.scrollingScreenshotAutoCompact,
                maxOutputHeightPx: StitchConfig.default.maxOutputHeightPx
            )
        )
        self.service = service
        self.settings = settings
    }

    func captureInitialFrame() async throws {
        try await captureNextFrame()
    }

    func captureNextFrame() async throws {
        guard session.frames.count < settings.scrollingScreenshotMaxFrames else {
            warning = "Maximum frame count reached."
            return
        }
        session.status = .capturing
        let document = try await service.capture(captureTarget, options: CaptureOptions(includeCursor: settings.screenshotIncludeCursor, hideBelloBoxWindows: true, delayAfterHidingOverlays: 0.1))
        let frame = ScrollCapturedFrame(image: document.baseImage, targetRect: targetRect)
        if let previous = session.frames.last, previous.image.width == frame.image.width, previous.image.height == frame.image.height {
            if ImageStitcher.appearsUnchanged(previous: previous.image, current: frame.image) {
                warning = "The new frame appears mostly unchanged. Scroll before capturing the next frame."
            } else {
                warning = nil
            }
        }
        session.frames.append(frame)
        session.status = .waitingForScroll
    }

    func finish() throws -> ScreenshotDocument {
        session.status = .stitching
        let result = try ImageStitcher.stitch(session.frames.map(\.image), config: session.config)
        session.status = .finished
        return ScreenshotDocument(
            baseImage: result.image,
            scale: 1,
            source: .scrolling(target: session.target.summary, frameCount: session.frames.count),
            ocrResults: result.warnings.isEmpty ? [] : [
                OCRResult(
                    id: UUID(),
                    engine: .appleVision(revision: nil, recognitionLevel: .accurate),
                    target: .fullImage,
                    plainText: "",
                    markdownText: nil,
                    regions: [],
                    languageHints: [],
                    imageDigest: "",
                    warnings: result.warnings,
                    createdAt: Date()
                ),
            ],
            activeOCRResultID: nil
        )
    }

    func postAutoScrollEvent() {
        activateTargetApplication()
        guard let event = CGEvent(
            scrollWheelEvent2Source: nil,
            units: .pixel,
            wheelCount: 1,
            wheel1: session.direction == .down ? -520 : 520,
            wheel2: 0,
            wheel3: 0
        ) else {
            warning = "Could not create an auto-scroll event. Use manual Capture Next."
            return
        }
        event.location = targetCenter
        event.post(tap: .cghidEventTap)
    }

    private var captureTarget: CaptureTarget {
        switch session.target {
        case let .area(area): return .area(area)
        case let .window(window): return .window(window)
        }
    }

    private var targetRect: CGRect {
        switch session.target {
        case let .area(area): return area.cocoaRect
        case .window: return .zero
        }
    }

    private var targetCenter: CGPoint {
        switch session.target {
        case let .area(area):
            return CGPoint(x: area.cocoaRect.midX, y: area.cocoaRect.midY)
        case let .window(window):
            if let frame = window.frame, !frame.isEmpty {
                return CGPoint(x: frame.midX, y: frame.midY)
            }
            return NSEvent.mouseLocation
        }
    }

    private func activateTargetApplication() {
        guard case let .window(window) = session.target else { return }
        if let processID = window.ownerProcessID,
           let application = NSRunningApplication(processIdentifier: processID) {
            application.activate(options: [.activateIgnoringOtherApps])
            return
        }
        if let bundleID = window.ownerBundleID,
           let application = NSRunningApplication.runningApplications(withBundleIdentifier: bundleID).first {
            application.activate(options: [.activateIgnoringOtherApps])
        }
    }
}
