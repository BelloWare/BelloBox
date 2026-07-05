import AppKit
import ScreenCaptureKit

struct CaptureOptions: Equatable {
    var includeCursor: Bool
    var hideBelloBoxWindows: Bool
    var delayAfterHidingOverlays: TimeInterval

    static let `default` = CaptureOptions(
        includeCursor: false,
        hideBelloBoxWindows: true,
        delayAfterHidingOverlays: 0.15
    )
}

@MainActor
protocol ScreenshotCapturing {
    func capture(_ target: CaptureTarget, options: CaptureOptions) async throws -> ScreenshotDocument
}

@MainActor
protocol CapturableWindowProviding {
    func capturableWindows() async throws -> [CaptureWindow]
}

@MainActor
final class ScreenCaptureService {
    enum CaptureError: LocalizedError, Equatable {
        case permissionDenied
        case userCancelled
        case noDisplayFound
        case noWindowFound
        case captureFailed(String)
        case protectedContent

        var errorDescription: String? {
            switch self {
            case .permissionDenied:
                return "Screen Recording permission is required to capture screenshots."
            case .userCancelled:
                return "Screenshot capture was cancelled."
            case .noDisplayFound:
                return "No display could be found for this capture."
            case .noWindowFound:
                return "No capturable window could be found."
            case let .captureFailed(message):
                return "Screenshot capture failed. \(message)"
            case .protectedContent:
                return "This content cannot be captured."
            }
        }
    }

    var beforeCapture: (() -> Void)?
    var afterCapture: (() -> Void)?
    private var cachedShareableContent: (date: Date, content: SCShareableContent)?

    func captureDisplaySnapshots(options: CaptureOptions = .default) async throws -> [DisplaySnapshot] {
        guard ScreenCapturePermission.isTrusted else { throw CaptureError.permissionDenied }
        if options.hideBelloBoxWindows {
            beforeCapture?()
        }
        defer {
            if options.hideBelloBoxWindows {
                afterCapture?()
            }
        }
        if options.delayAfterHidingOverlays > 0 {
            try await Task.sleep(nanoseconds: UInt64(options.delayAfterHidingOverlays * 1_000_000_000))
        }

        var snapshots: [DisplaySnapshot] = []
        var firstCaptureError: Error?
        for screen in NSScreen.screens {
            guard let displayID = ScreenCoordinateSpace.displayID(for: screen) else { continue }
            do {
                let image = try await captureDisplay(displayID, includeCursor: options.includeCursor)
                try validate(image)
                snapshots.append(DisplaySnapshot(
                    displayID: displayID,
                    screenFrame: screen.frame,
                    scale: ScreenCoordinateSpace.imageScale(pixelWidth: image.width, screenFrame: screen.frame),
                    image: image
                ))
            } catch {
                if firstCaptureError == nil {
                    firstCaptureError = error
                }
            }
        }
        if snapshots.isEmpty {
            throw firstCaptureError ?? CaptureError.noDisplayFound
        }
        return snapshots
    }

    func document(fromSnapshot snapshot: DisplaySnapshot, cocoaRect: CGRect, source: ScreenshotSource) throws -> ScreenshotDocument {
        let boundedRect = cocoaRect.intersection(snapshot.screenFrame).standardized
        guard boundedRect.width >= 1, boundedRect.height >= 1 else {
            throw CaptureError.captureFailed("The selected area was outside the display bounds.")
        }
        let pixelRect = ScreenCoordinateSpace.cocoaRectToImagePixelRect(
            boundedRect,
            screenFrame: snapshot.screenFrame,
            imageSize: CGSize(width: snapshot.image.width, height: snapshot.image.height)
        )
        let imageBounds = CGRect(x: 0, y: 0, width: snapshot.image.width, height: snapshot.image.height)
        let cropRect = pixelRect.intersection(imageBounds).integral
        guard cropRect.width > 0, cropRect.height > 0,
              let crop = snapshot.image.cropping(to: cropRect)
        else {
            throw CaptureError.captureFailed("The selected area was outside the display bounds.")
        }
        let scale = ScreenCoordinateSpace.imageScale(pixelWidth: snapshot.image.width, screenFrame: snapshot.screenFrame)
        try validate(crop)
        return ScreenshotDocument(
            baseImage: crop,
            scale: scale,
            source: source
        )
    }

    func capture(_ target: CaptureTarget, options: CaptureOptions = .default) async throws -> ScreenshotDocument {
        guard ScreenCapturePermission.isTrusted else { throw CaptureError.permissionDenied }
        if options.hideBelloBoxWindows {
            beforeCapture?()
        }
        defer {
            if options.hideBelloBoxWindows {
                afterCapture?()
            }
        }
        if options.delayAfterHidingOverlays > 0 {
            try await Task.sleep(nanoseconds: UInt64(options.delayAfterHidingOverlays * 1_000_000_000))
        }

        switch target {
        case let .display(display):
            let image = try await captureDisplay(display.displayID, includeCursor: options.includeCursor)
            try validate(image)
            return ScreenshotDocument(
                baseImage: image,
                scale: displayScale(displayID: display.displayID, image: image),
                source: .display(displayID: display.displayID)
            )
        case let .area(area):
            let screen: NSScreen
            let displayID: CGDirectDisplayID
            if let explicitDisplayID = area.displayID {
                guard let explicitScreen = self.screen(for: explicitDisplayID) else {
                    throw CaptureError.noDisplayFound
                }
                screen = explicitScreen
                displayID = explicitDisplayID
            } else {
                guard let resolvedScreen = ScreenCoordinateSpace.strictDisplayForCocoaRect(area.cocoaRect),
                      let resolvedDisplayID = ScreenCoordinateSpace.displayID(for: resolvedScreen)
                else {
                    throw CaptureError.noDisplayFound
                }
                screen = resolvedScreen
                displayID = resolvedDisplayID
            }
            let boundedRect = area.cocoaRect.intersection(screen.frame).standardized
            guard boundedRect.width >= 1, boundedRect.height >= 1 else {
                throw CaptureError.captureFailed("The selected area was outside the display bounds.")
            }
            let image = try await captureDisplay(displayID, includeCursor: options.includeCursor)
            let pixelRect = ScreenCoordinateSpace.cocoaRectToImagePixelRect(
                boundedRect,
                screenFrame: screen.frame,
                imageSize: CGSize(width: image.width, height: image.height)
            )
            let imageBounds = CGRect(x: 0, y: 0, width: image.width, height: image.height)
            let cropRect = pixelRect.intersection(imageBounds).integral
            guard cropRect.width > 0, cropRect.height > 0,
                  let crop = image.cropping(to: cropRect)
            else { throw CaptureError.captureFailed("The selected area was outside the display bounds.") }
            let scale = ScreenCoordinateSpace.imageScale(pixelWidth: image.width, screenFrame: screen.frame)
            try validate(crop)
            return ScreenshotDocument(
                baseImage: crop,
                scale: scale,
                source: .area(rect: area.cocoaRect, displayID: displayID)
            )
        case let .window(window):
            let image = try await captureWindow(window, includeCursor: options.includeCursor)
            try validate(image)
            return ScreenshotDocument(
                baseImage: image,
                scale: window.frame.map { Self.windowImageScale(image: image, frame: $0) } ?? 1,
                source: .window(title: window.title, ownerName: window.ownerName, windowID: window.windowID)
            )
        }
    }

    func captureScreenUnderMouse(options: CaptureOptions = .default) async throws -> ScreenshotDocument {
        let screen = ScreenCoordinateSpace.screenContainingMouse()
        guard let displayID = ScreenCoordinateSpace.displayID(for: screen) else { throw CaptureError.noDisplayFound }
        return try await capture(.display(CaptureDisplay(displayID: displayID, frame: screen.frame)), options: options)
    }

    func capturableWindows() async throws -> [CaptureWindow] {
        let content = try await shareableContent()
        let ownPID = ProcessInfo.processInfo.processIdentifier
        return content.windows.compactMap { window in
            guard window.windowID != 0 else { return nil }
            if window.owningApplication?.processID == ownPID { return nil }
            if window.frame.width <= 20 || window.frame.height <= 20 { return nil }
            let cocoaFrame = ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(window.frame)
            return CaptureWindow(
                windowID: window.windowID,
                title: window.title,
                ownerName: window.owningApplication?.applicationName,
                ownerBundleID: window.owningApplication?.bundleIdentifier,
                ownerProcessID: window.owningApplication?.processID,
                frame: cocoaFrame,
                captureMode: .independentWindow,
                allowsVisibleFrameFallback: Self.coversDisplay(cocoaFrame)
            )
        }
        .includingSystemSurfaces()
        .sorted {
            let left = "\($0.ownerName ?? "") \($0.title ?? "")"
            let right = "\($1.ownerName ?? "") \($1.title ?? "")"
            return left.localizedCaseInsensitiveCompare(right) == .orderedAscending
        }
    }

    private func captureDisplay(_ displayID: CGDirectDisplayID, includeCursor: Bool) async throws -> CGImage {
        let requestedBounds = CGDisplayBounds(displayID)
        let content: SCShareableContent
        do {
            content = try await shareableContent()
        } catch {
            if error is CancellationError { throw error }
            return try legacyCaptureDisplay(
                displayID,
                reason: "shareableContentFailed error=\(error.localizedDescription)",
                requestedBounds: requestedBounds,
                originalError: error
            )
        }
        let initialCandidates = Self.displayCandidates(from: content.displays)
        logDisplayCapture(
            "displayCapture.resolve.initial",
            [
                "requestedDisplayID=\(displayID)",
                "requestedBounds=\(Self.serialize(requestedBounds))",
                "availableSCKDisplayIDs=\(Self.displayIDSummary(initialCandidates))",
            ]
        )

        let refreshedContent: SCShareableContent?
        let refreshedCandidates: [DisplayCaptureCandidate]?
        if initialCandidates.contains(where: { $0.displayID == displayID }) {
            refreshedContent = nil
            refreshedCandidates = nil
        } else {
            do {
                refreshedContent = try await shareableContent(forceRefresh: true)
                refreshedCandidates = refreshedContent.map { Self.displayCandidates(from: $0.displays) }
            } catch {
                if error is CancellationError { throw error }
                return try legacyCaptureDisplay(
                    displayID,
                    reason: "shareableContentRefreshFailed error=\(error.localizedDescription)",
                    requestedBounds: requestedBounds,
                    originalError: error
                )
            }
            logDisplayCapture(
                "displayCapture.resolve.refreshed",
                [
                    "requestedDisplayID=\(displayID)",
                    "availableSCKDisplayIDs=\(Self.displayIDSummary(refreshedCandidates ?? []))",
                ]
            )
        }

        let resolution = DisplayCaptureResolver.resolve(
            requestedDisplayID: displayID,
            requestedBounds: requestedBounds,
            initialCandidates: initialCandidates,
            refreshedCandidates: refreshedCandidates,
            legacyFallbackAvailable: true
        )
        let contentForCapture: SCShareableContent
        let candidate: DisplayCaptureCandidate
        let matchPath: DisplayCaptureResolver.MatchPath
        switch resolution {
        case let .screenCaptureKit(resolvedCandidate, source, path):
            candidate = resolvedCandidate
            contentForCapture = (source == .refreshed ? refreshedContent : content) ?? content
            matchPath = path
        case let .legacyFallback(reason):
            return try legacyCaptureDisplay(
                displayID,
                reason: "resolver=\(reason)",
                requestedBounds: requestedBounds
            )
        case .noDisplayFound:
            throw CaptureError.noDisplayFound
        }

        guard let display = contentForCapture.displays.first(where: { $0.displayID == candidate.displayID }) else {
            return try legacyCaptureDisplay(
                displayID,
                reason: "resolvedSCKDisplayMissing path=\(matchPath) resolvedDisplayID=\(candidate.displayID)",
                requestedBounds: requestedBounds
            )
        }

        let pixelSize = ScreenCoordinateSpace.displayPixelSize(for: displayID, fallbackScreen: screen(for: displayID))
        let filter = SCContentFilter(display: display, excludingApplications: [], exceptingWindows: [])
        let configuration = SCStreamConfiguration()
        configuration.width = max(1, Int(pixelSize.width.rounded()))
        configuration.height = max(1, Int(pixelSize.height.rounded()))
        configuration.showsCursor = includeCursor
        configuration.capturesAudio = false
        logDisplayCapture(
            "displayCapture.sck.begin",
            [
                "requestedDisplayID=\(displayID)",
                "resolvedDisplayID=\(display.displayID)",
                "matchPath=\(matchPath)",
                "resolvedFrame=\(Self.serialize(candidate.frame))",
                "configuration=\(configuration.width)x\(configuration.height)",
                "filter=excludingApplications",
            ]
        )
        do {
            let image = try await capture(filter: filter, configuration: configuration)
            logDisplayCapture(
                "displayCapture.sck.success",
                [
                    "requestedDisplayID=\(displayID)",
                    "resolvedDisplayID=\(display.displayID)",
                    "matchPath=\(matchPath)",
                    "outputPixels=\(image.width)x\(image.height)",
                ]
            )
            return image
        } catch {
            if error is CancellationError { throw error }
            return try legacyCaptureDisplay(
                displayID,
                reason: "sckCaptureFailed path=\(matchPath) resolvedDisplayID=\(display.displayID) error=\(error.localizedDescription)",
                requestedBounds: requestedBounds,
                originalError: error
            )
        }
    }

    private func captureWindow(_ target: CaptureWindow, includeCursor: Bool) async throws -> CGImage {
        if target.captureMode == .visibleFrame {
            return try await captureVisibleFrame(of: target, includeCursor: includeCursor)
        }

        let content = try await shareableContent()
        guard let window = content.windows.first(where: { $0.windowID == target.windowID }) else {
            if target.allowsVisibleFrameFallback {
                return try await captureVisibleFrame(of: target, includeCursor: includeCursor)
            }
            throw CaptureError.noWindowFound
        }
        let filter = SCContentFilter(desktopIndependentWindow: window)
        let configuration = SCStreamConfiguration()
        let cocoaFrame = target.frame ?? ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(window.frame)
        let pixelSize = windowPixelSize(for: cocoaFrame)
        configuration.width = max(1, Int(pixelSize.width.rounded()))
        configuration.height = max(1, Int(pixelSize.height.rounded()))
        configuration.showsCursor = includeCursor
        configuration.capturesAudio = false
        return try await capture(filter: filter, configuration: configuration)
    }

    private func captureVisibleFrame(of target: CaptureWindow, includeCursor: Bool) async throws -> CGImage {
        guard let frame = target.frame else { throw CaptureError.noWindowFound }
        guard let screen = ScreenCoordinateSpace.strictDisplayForCocoaRect(frame),
              let displayID = ScreenCoordinateSpace.displayID(for: screen)
        else { throw CaptureError.noDisplayFound }
        let boundedRect = frame.intersection(screen.frame).standardized
        guard boundedRect.width >= 1, boundedRect.height >= 1 else {
            throw CaptureError.captureFailed("The selected window was outside the display bounds.")
        }
        let image = try await captureDisplay(displayID, includeCursor: includeCursor)
        let pixelRect = ScreenCoordinateSpace.cocoaRectToImagePixelRect(
            boundedRect,
            screenFrame: screen.frame,
            imageSize: CGSize(width: image.width, height: image.height)
        )
        let imageBounds = CGRect(x: 0, y: 0, width: image.width, height: image.height)
        let cropRect = pixelRect.intersection(imageBounds).integral
        guard cropRect.width > 0, cropRect.height > 0,
              let crop = image.cropping(to: cropRect)
        else {
            throw CaptureError.captureFailed("The selected window was outside the display bounds.")
        }
        return crop
    }

    private func capture(filter: SCContentFilter, configuration: SCStreamConfiguration) async throws -> CGImage {
        do {
            if #available(macOS 14.0, *) {
                return try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: configuration)
            }
            return try await ScreenCaptureFrameGrabber().capture(filter: filter, configuration: configuration)
        } catch {
            if error is CancellationError {
                throw error
            }
            if let captureError = error as? CaptureError {
                throw captureError
            }
            throw CaptureError.captureFailed(error.localizedDescription)
        }
    }

    private func shareableContent(maxAge: TimeInterval = 0.35, forceRefresh: Bool = false) async throws -> SCShareableContent {
        if forceRefresh {
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            cachedShareableContent = (Date(), content)
            return content
        }
        if let cachedShareableContent,
           Date().timeIntervalSince(cachedShareableContent.date) <= maxAge {
            return cachedShareableContent.content
        }
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        cachedShareableContent = (Date(), content)
        return content
    }

    private func validate(_ image: CGImage) throws {
        guard image.width > 0, image.height > 0 else { throw CaptureError.captureFailed("The captured image was empty.") }
    }

    private func screen(for displayID: CGDirectDisplayID) -> NSScreen? {
        NSScreen.screens.first { ScreenCoordinateSpace.displayID(for: $0) == displayID }
    }

    private func displayScale(displayID: CGDirectDisplayID, image: CGImage) -> CGFloat {
        guard let screen = screen(for: displayID) else { return 1 }
        return ScreenCoordinateSpace.imageScale(pixelWidth: image.width, screenFrame: screen.frame)
    }

    private func windowPixelSize(for cocoaFrame: CGRect) -> CGSize {
        guard let screen = ScreenCoordinateSpace.displayForCocoaRect(cocoaFrame),
              let displayID = ScreenCoordinateSpace.displayID(for: screen)
        else {
            return CGSize(width: max(1, cocoaFrame.width), height: max(1, cocoaFrame.height))
        }
        return ScreenCoordinateSpace.pixelSize(
            forCocoaSize: cocoaFrame.size,
            screenFrame: screen.frame,
            displayPixelSize: ScreenCoordinateSpace.displayPixelSize(for: displayID, fallbackScreen: screen)
        )
    }

    private static func windowImageScale(image: CGImage, frame: CGRect) -> CGFloat {
        guard frame.width > 0 else { return 1 }
        return CGFloat(image.width) / frame.width
    }

    private static func coversDisplay(_ rect: CGRect) -> Bool {
        NSScreen.screens.contains { screen in
            let inset: CGFloat = 2
            return abs(rect.minX - screen.frame.minX) <= inset
                && abs(rect.minY - screen.frame.minY) <= inset
                && abs(rect.width - screen.frame.width) <= inset * 2
                && abs(rect.height - screen.frame.height) <= inset * 2
        }
    }

    private func legacyCaptureDisplay(
        _ displayID: CGDirectDisplayID,
        reason: String,
        requestedBounds: CGRect,
        originalError: Error? = nil
    ) throws -> CGImage {
        logDisplayCapture(
            "displayCapture.legacy.begin",
            [
                "requestedDisplayID=\(displayID)",
                "requestedBounds=\(Self.serialize(requestedBounds))",
                "reason=\(reason)",
            ]
        )
        guard let image = CGDisplayCreateImage(displayID) else {
            logDisplayCapture(
                "displayCapture.legacy.failure",
                [
                    "requestedDisplayID=\(displayID)",
                    "reason=\(reason)",
                    "originalError=\(originalError?.localizedDescription ?? "none")",
                ]
            )
            if let originalError {
                throw CaptureError.captureFailed("ScreenCaptureKit failed and the legacy display capture fallback was unavailable. \(originalError.localizedDescription)")
            }
            throw CaptureError.noDisplayFound
        }
        logDisplayCapture(
            "displayCapture.legacy.success",
            [
                "requestedDisplayID=\(displayID)",
                "reason=\(reason)",
                "outputPixels=\(image.width)x\(image.height)",
            ]
        )
        return image
    }

    private func logDisplayCapture(_ event: String, _ details: [String]) {
        CaptureDiagnostics.log(event, enabled: true, details: details)
    }

    private static func displayCandidates(from displays: [SCDisplay]) -> [DisplayCaptureCandidate] {
        displays.map { DisplayCaptureCandidate(displayID: $0.displayID, frame: $0.frame) }
    }

    private static func displayIDSummary(_ candidates: [DisplayCaptureCandidate]) -> String {
        candidates
            .map { "\($0.displayID)@\(serialize($0.frame))" }
            .joined(separator: ";")
    }

    private static func serialize(_ rect: CGRect) -> String {
        "\(Int(rect.origin.x.rounded())),\(Int(rect.origin.y.rounded())),\(Int(rect.size.width.rounded())),\(Int(rect.size.height.rounded()))"
    }
}

extension ScreenCaptureService: ScreenshotCapturing, CapturableWindowProviding {}

private extension Array where Element == CaptureWindow {
    func includingSystemSurfaces() -> [CaptureWindow] {
        let existingIDs = Set(map(\.windowID))
        let systemSurfaces = CaptureWindowCatalog.currentWindows().filter { window in
            window.captureMode == .visibleFrame && !existingIDs.contains(window.windowID)
        }
        return self + systemSurfaces
    }
}
