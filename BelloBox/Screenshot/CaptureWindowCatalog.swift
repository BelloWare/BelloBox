import AppKit

enum CaptureWindowCatalog {
    static func currentWindows() -> [CaptureWindow] {
        let ownPID = ProcessInfo.processInfo.processIdentifier
        guard let info = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else {
            return []
        }

        return windows(from: info, ownPID: ownPID, screenFrames: NSScreen.screens.map(\.frame))
    }

    static func windows(from entries: [[String: Any]], ownPID: pid_t, screenFrames: [CGRect]) -> [CaptureWindow] {
        return entries.compactMap { entry in
            guard
                let windowNumber = entry[kCGWindowNumber as String] as? NSNumber,
                let ownerPID = entry[kCGWindowOwnerPID as String] as? NSNumber,
                ownerPID.int32Value != ownPID,
                let layer = entry[kCGWindowLayer as String] as? NSNumber,
                isSelectableLayer(layer.intValue),
                let alpha = entry[kCGWindowAlpha as String] as? NSNumber,
                alpha.doubleValue > 0.01,
                let boundsDictionary = entry[kCGWindowBounds as String] as? [String: Any],
                let bounds = CGRect(dictionaryRepresentation: boundsDictionary as CFDictionary),
                isSelectableBounds(bounds)
            else { return nil }

            let cocoaFrame = ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(bounds, screenFrames: screenFrames)
            let mode = captureMode(forLayer: layer.intValue)
            return CaptureWindow(
                windowID: windowNumber.uint32Value,
                title: entry[kCGWindowName as String] as? String,
                ownerName: entry[kCGWindowOwnerName as String] as? String,
                ownerBundleID: nil,
                ownerProcessID: ownerPID.int32Value,
                frame: cocoaFrame,
                captureMode: mode,
                layer: layer.intValue,
                allowsVisibleFrameFallback: mode == .visibleFrame || coversDisplay(cocoaFrame, screenFrames: screenFrames)
            )
        }
    }

    private static func isSelectableLayer(_ layer: Int) -> Bool {
        if layer == CGWindowLevelForKey(.normalWindow) { return true }
        return layer == CGWindowLevelForKey(.floatingWindow)
            || layer == CGWindowLevelForKey(.modalPanelWindow)
            || layer == CGWindowLevelForKey(.mainMenuWindow)
            || layer == CGWindowLevelForKey(.statusWindow)
            || layer == CGWindowLevelForKey(.popUpMenuWindow)
    }

    private static func captureMode(forLayer layer: Int) -> CaptureWindowCaptureMode {
        layer == CGWindowLevelForKey(.normalWindow) ? .independentWindow : .visibleFrame
    }

    private static func isSelectableBounds(_ bounds: CGRect) -> Bool {
        let width = bounds.width.rounded(.down)
        let height = bounds.height.rounded(.down)
        return width >= 8 && height >= 8 && width * height >= 96
    }

    private static func coversDisplay(_ rect: CGRect, screenFrames: [CGRect]) -> Bool {
        screenFrames.contains { screen in
            let inset: CGFloat = 2
            return abs(rect.minX - screen.minX) <= inset
                && abs(rect.minY - screen.minY) <= inset
                && abs(rect.width - screen.width) <= inset * 2
                && abs(rect.height - screen.height) <= inset * 2
        }
    }
}
