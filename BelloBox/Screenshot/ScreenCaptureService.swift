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
        for screen in NSScreen.screens {
            guard let displayID = ScreenCoordinateSpace.displayID(for: screen) else { continue }
            let image = try await captureDisplay(displayID, includeCursor: options.includeCursor)
            try validate(image)
            snapshots.append(DisplaySnapshot(
                displayID: displayID,
                screenFrame: screen.frame,
                scale: ScreenCoordinateSpace.imageScale(pixelWidth: image.width, screenFrame: screen.frame),
                image: image
            ))
        }
        guard !snapshots.isEmpty else { throw CaptureError.noDisplayFound }
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
            guard let screen = area.displayID.flatMap(screen(for:)) ?? ScreenCoordinateSpace.displayForCocoaRect(area.cocoaRect),
                  let displayID = ScreenCoordinateSpace.displayID(for: screen)
            else { throw CaptureError.noDisplayFound }
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
                frame: cocoaFrame
            )
        }
        .sorted {
            let left = "\($0.ownerName ?? "") \($0.title ?? "")"
            let right = "\($1.ownerName ?? "") \($1.title ?? "")"
            return left.localizedCaseInsensitiveCompare(right) == .orderedAscending
        }
    }

    private func captureDisplay(_ displayID: CGDirectDisplayID, includeCursor: Bool) async throws -> CGImage {
        let content = try await shareableContent()
        guard let display = content.displays.first(where: { $0.displayID == displayID }) else {
            throw CaptureError.noDisplayFound
        }
        let pixelSize = ScreenCoordinateSpace.displayPixelSize(for: displayID, fallbackScreen: screen(for: displayID))
        let filter = SCContentFilter(display: display, excludingWindows: [])
        let configuration = SCStreamConfiguration()
        configuration.width = max(1, Int(pixelSize.width.rounded()))
        configuration.height = max(1, Int(pixelSize.height.rounded()))
        configuration.showsCursor = includeCursor
        configuration.capturesAudio = false
        return try await capture(filter: filter, configuration: configuration)
    }

    private func captureWindow(_ target: CaptureWindow, includeCursor: Bool) async throws -> CGImage {
        let content = try await shareableContent()
        guard let window = content.windows.first(where: { $0.windowID == target.windowID }) else {
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

    private func shareableContent() async throws -> SCShareableContent {
        try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
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
}

extension ScreenCaptureService: ScreenshotCapturing, CapturableWindowProviding {}
