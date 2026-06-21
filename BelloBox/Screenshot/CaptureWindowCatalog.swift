import AppKit

enum CaptureWindowCatalog {
    static func currentWindows() -> [CaptureWindow] {
        let ownPID = ProcessInfo.processInfo.processIdentifier
        guard let info = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else {
            return []
        }

        return info.compactMap { entry in
            guard
                let windowNumber = entry[kCGWindowNumber as String] as? NSNumber,
                let ownerPID = entry[kCGWindowOwnerPID as String] as? NSNumber,
                ownerPID.int32Value != ownPID,
                let layer = entry[kCGWindowLayer as String] as? NSNumber,
                layer.intValue == 0,
                let alpha = entry[kCGWindowAlpha as String] as? NSNumber,
                alpha.doubleValue > 0.01,
                let boundsDictionary = entry[kCGWindowBounds as String] as? [String: Any],
                let bounds = CGRect(dictionaryRepresentation: boundsDictionary as CFDictionary),
                bounds.width > 20,
                bounds.height > 20
            else { return nil }

            return CaptureWindow(
                windowID: windowNumber.uint32Value,
                title: entry[kCGWindowName as String] as? String,
                ownerName: entry[kCGWindowOwnerName as String] as? String,
                ownerBundleID: nil,
                ownerProcessID: ownerPID.int32Value,
                frame: ScreenCoordinateSpace.cgWindowBoundsToCocoaRect(bounds)
            )
        }
    }
}
