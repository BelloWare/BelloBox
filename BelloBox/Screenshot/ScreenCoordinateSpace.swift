import AppKit
import CoreGraphics

enum ScreenCoordinateSpace {
    static func displayForCocoaRect(_ rect: CGRect) -> NSScreen? {
        let screens = NSScreen.screens
        guard let frame = screenFrame(for: rect, in: screens.map(\.frame)) else {
            return NSScreen.main
        }
        return screens.first { $0.frame == frame } ?? NSScreen.main
    }

    static func strictDisplayForCocoaRect(_ rect: CGRect) -> NSScreen? {
        let screens = NSScreen.screens
        guard let frame = strictScreenFrame(for: rect, in: screens.map(\.frame)) else { return nil }
        return screens.first { $0.frame == frame }
    }

    static func screen(containingOrNearestTo point: CGPoint) -> NSScreen {
        let screens = NSScreen.screens
        if let frame = screenFrame(containingOrNearestTo: point, in: screens.map(\.frame)),
           let screen = screens.first(where: { $0.frame == frame }) {
            return screen
        }
        if let main = NSScreen.main { return main }
        precondition(!screens.isEmpty, "No screens are available.")
        return screens[0]
    }

    static func screenFrame(for rect: CGRect, in screenFrames: [CGRect]) -> CGRect? {
        guard !screenFrames.isEmpty, !rect.isNull else { return nil }
        let rect = rect.standardized
        let candidates = screenFrames
            .map { frame in (frame: frame, area: frame.intersection(rect).area) }
            .filter { $0.area > 0 }
            .sorted { $0.area > $1.area }
        if let best = candidates.first?.frame { return best }
        return screenFrame(containingOrNearestTo: CGPoint(x: rect.midX, y: rect.midY), in: screenFrames)
    }

    static func strictScreenFrame(for rect: CGRect, in screenFrames: [CGRect]) -> CGRect? {
        guard !screenFrames.isEmpty, !rect.isNull else { return nil }
        let rect = rect.standardized
        return screenFrames
            .map { frame in (frame: frame, area: frame.intersection(rect).area) }
            .filter { $0.area > 0 }
            .sorted { $0.area > $1.area }
            .first?.frame
    }

    static func screenFrame(containingOrNearestTo point: CGPoint, in screenFrames: [CGRect]) -> CGRect? {
        guard !screenFrames.isEmpty else { return nil }
        if let containing = screenFrames.first(where: { $0.contains(point) }) {
            return containing
        }
        return screenFrames.min { lhs, rhs in
            lhs.distanceSquared(to: point) < rhs.distanceSquared(to: point)
        }
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
        let x = ((rect.minX - screenFrame.minX) * scale).rounded(.down)
        let maxX = ((rect.maxX - screenFrame.minX) * scale).rounded(.up)
        let yFromTop = ((screenFrame.maxY - rect.maxY) * scale).rounded(.down)
        let maxYFromTop = ((screenFrame.maxY - rect.minY) * scale).rounded(.up)
        return CGRect(
            x: x,
            y: yFromTop,
            width: max(1, maxX - x),
            height: max(1, maxYFromTop - yFromTop)
        ).standardized
    }

    static func cocoaRectToImagePixelRect(_ rect: CGRect, screenFrame: CGRect, imageSize: CGSize) -> CGRect {
        let xScale = imageSize.width > 0 && screenFrame.width > 0 ? imageSize.width / screenFrame.width : 1
        let yScale = imageSize.height > 0 && screenFrame.height > 0 ? imageSize.height / screenFrame.height : xScale
        let x = ((rect.minX - screenFrame.minX) * xScale).rounded(.down)
        let maxX = ((rect.maxX - screenFrame.minX) * xScale).rounded(.up)
        let yFromTop = ((screenFrame.maxY - rect.maxY) * yScale).rounded(.down)
        let maxYFromTop = ((screenFrame.maxY - rect.minY) * yScale).rounded(.up)
        return CGRect(
            x: x,
            y: yFromTop,
            width: max(1, maxX - x),
            height: max(1, maxYFromTop - yFromTop)
        ).standardized
    }

    static func imageScale(pixelWidth: Int, screenFrame: CGRect) -> CGFloat {
        guard pixelWidth > 0, screenFrame.width > 0 else { return 1 }
        return CGFloat(pixelWidth) / screenFrame.width
    }

    static func pixelSize(forCocoaSize size: CGSize, screenFrame: CGRect, displayPixelSize: CGSize) -> CGSize {
        let xScale = displayPixelSize.width > 0 && screenFrame.width > 0 ? displayPixelSize.width / screenFrame.width : 1
        let yScale = displayPixelSize.height > 0 && screenFrame.height > 0 ? displayPixelSize.height / screenFrame.height : xScale
        return CGSize(
            width: max(1, size.width * xScale),
            height: max(1, size.height * yScale)
        )
    }

    static func displayPixelSize(for displayID: CGDirectDisplayID, fallbackScreen screen: NSScreen? = nil) -> CGSize {
        let cgPixelSize = CGSize(width: CGDisplayPixelsWide(displayID), height: CGDisplayPixelsHigh(displayID))
        guard let screen else {
            return cgPixelSize.width > 0 && cgPixelSize.height > 0 ? cgPixelSize : .zero
        }
        return resolvedDisplayPixelSize(
            cgPixelSize: cgPixelSize,
            screenFrame: screen.frame,
            backingScale: backingScale(for: screen)
        )
    }

    static func resolvedDisplayPixelSize(cgPixelSize: CGSize, screenFrame: CGRect, backingScale: CGFloat) -> CGSize {
        let scale = backingScale > 0 ? backingScale : 1
        let backingSize = CGSize(width: max(1, screenFrame.width * scale), height: max(1, screenFrame.height * scale))
        guard cgPixelSize.width > 0, cgPixelSize.height > 0 else { return backingSize }
        return CGSize(
            width: max(cgPixelSize.width, backingSize.width),
            height: max(cgPixelSize.height, backingSize.height)
        )
    }

    static func cgWindowBoundsToCocoaRect(_ bounds: CGRect, screens: [NSScreen] = NSScreen.screens) -> CGRect {
        topLeftRectToCocoaRect(bounds, screenFrames: screens.map(\.frame))
    }

    static func cgWindowBoundsToCocoaRect(_ bounds: CGRect, screenFrames: [CGRect]) -> CGRect {
        topLeftRectToCocoaRect(bounds, screenFrames: screenFrames)
    }

    static func topLeftPointToCocoaPoint(_ point: CGPoint, screens: [NSScreen] = NSScreen.screens) -> CGPoint {
        topLeftPointToCocoaPoint(point, screenFrames: screens.map(\.frame))
    }

    static func topLeftPointToCocoaPoint(_ point: CGPoint, screenFrames: [CGRect]) -> CGPoint {
        let maxY = primaryTopEdge(in: screenFrames) ?? point.y
        return CGPoint(x: point.x, y: maxY - point.y)
    }

    static func cocoaPointToTopLeftPoint(_ point: CGPoint, screens: [NSScreen] = NSScreen.screens) -> CGPoint {
        cocoaPointToTopLeftPoint(point, screenFrames: screens.map(\.frame))
    }

    static func cocoaPointToTopLeftPoint(_ point: CGPoint, screenFrames: [CGRect]) -> CGPoint {
        let maxY = primaryTopEdge(in: screenFrames) ?? point.y
        return CGPoint(x: point.x, y: maxY - point.y)
    }

    static func topLeftRectToCocoaRect(_ rect: CGRect, screens: [NSScreen] = NSScreen.screens) -> CGRect {
        topLeftRectToCocoaRect(rect, screenFrames: screens.map(\.frame))
    }

    static func topLeftRectToCocoaRect(_ rect: CGRect, screenFrames: [CGRect]) -> CGRect {
        let maxY = primaryTopEdge(in: screenFrames) ?? rect.maxY
        return CGRect(
            x: rect.minX,
            y: maxY - rect.maxY,
            width: rect.width,
            height: rect.height
        ).standardized
    }

    private static func primaryTopEdge(in screenFrames: [CGRect]) -> CGFloat? {
        let originScreen = screenFrames.first { $0.minX == 0 && $0.minY == 0 } ?? screenFrames.first
        return originScreen?.maxY
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
        screen(containingOrNearestTo: NSEvent.mouseLocation)
    }
}

private extension CGRect {
    var area: CGFloat {
        guard !isNull, !isEmpty else { return 0 }
        return width * height
    }

    func distanceSquared(to point: CGPoint) -> CGFloat {
        let dx: CGFloat
        if point.x < minX {
            dx = minX - point.x
        } else if point.x > maxX {
            dx = point.x - maxX
        } else {
            dx = 0
        }

        let dy: CGFloat
        if point.y < minY {
            dy = minY - point.y
        } else if point.y > maxY {
            dy = point.y - maxY
        } else {
            dy = 0
        }

        return dx * dx + dy * dy
    }
}
