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

    func capture(_ target: CaptureTarget, options: CaptureOptions = .default) async throws -> ScreenshotDocument {
        guard ScreenCapturePermission.isTrusted else { throw CaptureError.permissionDenied }
        beforeCapture?()
        defer { afterCapture?() }
        if options.delayAfterHidingOverlays > 0 {
            try? await Task.sleep(nanoseconds: UInt64(options.delayAfterHidingOverlays * 1_000_000_000))
        }

        switch target {
        case let .display(display):
            let image = try await captureDisplay(display.displayID, includeCursor: options.includeCursor)
            try validate(image)
            return ScreenshotDocument(
                baseImage: image,
                scale: displayScale(displayID: display.displayID),
                source: .display(displayID: display.displayID)
            )
        case let .area(area):
            guard let screen = area.displayID.flatMap(screen(for:)) ?? ScreenCoordinateSpace.displayForCocoaRect(area.cocoaRect),
                  let displayID = ScreenCoordinateSpace.displayID(for: screen)
            else { throw CaptureError.noDisplayFound }
            let image = try await captureDisplay(displayID, includeCursor: options.includeCursor)
            let pixelRect = ScreenCoordinateSpace.cocoaRectToDisplayPixelRect(area.cocoaRect, on: screen)
            guard let crop = image.cropping(to: pixelRect.integral) else { throw CaptureError.captureFailed("The selected area was outside the display bounds.") }
            try validate(crop)
            return ScreenshotDocument(
                baseImage: crop,
                scale: ScreenCoordinateSpace.backingScale(for: screen),
                source: .area(rect: area.cocoaRect, displayID: displayID)
            )
        case let .window(window):
            let image = try await captureWindow(window, includeCursor: options.includeCursor)
            try validate(image)
            return ScreenshotDocument(
                baseImage: image,
                scale: 1,
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
            return CaptureWindow(
                windowID: window.windowID,
                title: window.title,
                ownerName: window.owningApplication?.applicationName,
                ownerBundleID: window.owningApplication?.bundleIdentifier,
                ownerProcessID: window.owningApplication?.processID,
                frame: window.frame
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
        let filter = SCContentFilter(display: display, excludingWindows: [])
        let configuration = SCStreamConfiguration()
        configuration.width = max(1, display.width)
        configuration.height = max(1, display.height)
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
        configuration.width = max(1, Int(window.frame.width))
        configuration.height = max(1, Int(window.frame.height))
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

    private func displayScale(displayID: CGDirectDisplayID) -> CGFloat {
        screen(for: displayID).map(ScreenCoordinateSpace.backingScale(for:)) ?? 1
    }
}
