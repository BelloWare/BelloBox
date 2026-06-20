import AppKit
import CoreGraphics

enum ScreenCoordinateSpace {
    static func displayForCocoaRect(_ rect: CGRect) -> NSScreen? {
        let candidates = NSScreen.screens
            .map { screen in (screen, screen.frame.intersection(rect).area) }
            .filter { $0.1 > 0 }
            .sorted { $0.1 > $1.1 }
        if let best = candidates.first?.0 { return best }
        let midpoint = CGPoint(x: rect.midX, y: rect.midY)
        return NSScreen.screens.first { $0.frame.contains(midpoint) } ?? NSScreen.main
    }

    static func displayID(for screen: NSScreen) -> CGDirectDisplayID? {
        if let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber {
            return CGDirectDisplayID(number.uint32Value)
        }
        return nil
    }

    static func cocoaRectToDisplayPixelRect(_ rect: CGRect, on screen: NSScreen) -> CGRect {
        cocoaRectToDisplayPixelRect(rect, screenFrame: screen.frame, scale: backingScale(for: screen))
    }

    static func cocoaRectToDisplayPixelRect(_ rect: CGRect, screenFrame: CGRect, scale: CGFloat) -> CGRect {
        let scale = scale > 0 ? scale : 1
        let x = (rect.minX - screenFrame.minX) * scale
        let yFromTop = (screenFrame.maxY - rect.maxY) * scale
        return CGRect(
            x: x.rounded(.down),
            y: yFromTop.rounded(.down),
            width: (rect.width * scale).rounded(.toNearestOrAwayFromZero),
            height: (rect.height * scale).rounded(.toNearestOrAwayFromZero)
        ).standardized
    }

    static func cocoaRectToImagePixelRect(_ rect: CGRect, screenFrame: CGRect, imageSize: CGSize) -> CGRect {
        let xScale = imageSize.width > 0 && screenFrame.width > 0 ? imageSize.width / screenFrame.width : 1
        let yScale = imageSize.height > 0 && screenFrame.height > 0 ? imageSize.height / screenFrame.height : xScale
        let x = (rect.minX - screenFrame.minX) * xScale
        let yFromTop = (screenFrame.maxY - rect.maxY) * yScale
        return CGRect(
            x: x.rounded(.down),
            y: yFromTop.rounded(.down),
            width: max(1, (rect.width * xScale).rounded(.toNearestOrAwayFromZero)),
            height: max(1, (rect.height * yScale).rounded(.toNearestOrAwayFromZero))
        ).standardized
    }

    static func imageScale(pixelWidth: Int, screenFrame: CGRect) -> CGFloat {
        guard pixelWidth > 0, screenFrame.width > 0 else { return 1 }
        return CGFloat(pixelWidth) / screenFrame.width
    }

    static func displayPixelRectToCocoaRect(_ rect: CGRect, on screen: NSScreen) -> CGRect {
        displayPixelRectToCocoaRect(rect, screenFrame: screen.frame, scale: backingScale(for: screen))
    }

    static func displayPixelRectToCocoaRect(_ rect: CGRect, screenFrame: CGRect, scale: CGFloat) -> CGRect {
        let scale = scale > 0 ? scale : 1
        let x = screenFrame.minX + rect.minX / scale
        let y = screenFrame.maxY - rect.maxY / scale
        return CGRect(x: x, y: y, width: rect.width / scale, height: rect.height / scale).standardized
    }

    static func backingScale(for screen: NSScreen) -> CGFloat {
        let scale = screen.backingScaleFactor
        return scale > 0 ? scale : 1
    }

    static func screenContainingMouse() -> NSScreen {
        let mouse = NSEvent.mouseLocation
        return NSScreen.screens.first { $0.frame.contains(mouse) } ?? NSScreen.main ?? NSScreen.screens[0]
    }
}

private extension CGRect {
    var area: CGFloat {
        guard !isNull, !isEmpty else { return 0 }
        return width * height
    }
}
