import AppKit

enum RegionCaptureResult: Equatable {
    case area(CaptureArea)
    case window(CaptureWindow)
}

enum RegionCaptureGeometry {
    static let dragThreshold: CGFloat = 6

    static func selectionRect(from start: CGPoint, to end: CGPoint) -> CGRect {
        CGRect(
            x: min(start.x, end.x),
            y: min(start.y, end.y),
            width: abs(start.x - end.x),
            height: abs(start.y - end.y)
        )
    }

    static func localFlippedPointToGlobalCocoa(_ point: CGPoint, screenFrame: CGRect) -> CGPoint {
        CGPoint(x: screenFrame.minX + point.x, y: screenFrame.maxY - point.y)
    }

    static func localFlippedRectToGlobalCocoa(_ rect: CGRect, screenFrame: CGRect) -> CGRect {
        CGRect(
            x: screenFrame.minX + rect.minX,
            y: screenFrame.maxY - rect.maxY,
            width: rect.width,
            height: rect.height
        ).standardized
    }

    static func globalCocoaRectToLocalFlipped(_ rect: CGRect, screenFrame: CGRect) -> CGRect {
        CGRect(
            x: rect.minX - screenFrame.minX,
            y: screenFrame.maxY - rect.maxY,
            width: rect.width,
            height: rect.height
        ).standardized
    }
}

@MainActor
final class RegionCaptureOverlayController {
    private var windows: [RegionOverlayWindow] = []
    private var completion: ((Result<RegionCaptureResult, ScreenCaptureService.CaptureError>) -> Void)?

    func begin(completion: @escaping (Result<RegionCaptureResult, ScreenCaptureService.CaptureError>) -> Void) {
        cancel()
        self.completion = completion
        let capturableWindows = RegionWindowCatalog.currentWindows()
        for screen in NSScreen.screens {
            let window = RegionOverlayWindow(screen: screen)
            let view = RegionOverlayView(screen: screen, windows: capturableWindows)
            view.onComplete = { [weak self] result in self?.finish(result, screen: screen) }
            view.onCancel = { [weak self] in self?.finish(.failure(.userCancelled)) }
            window.contentView = view
            window.makeKeyAndOrderFront(nil)
            windows.append(window)
        }
        NSCursor.crosshair.set()
    }

    func cancel() {
        for window in windows { window.orderOut(nil) }
        windows.removeAll()
        NSCursor.arrow.set()
        completion = nil
    }

    private func finish(_ result: RegionCaptureResult, screen: NSScreen) {
        switch result {
        case let .area(area):
            guard area.cocoaRect.width >= 8, area.cocoaRect.height >= 8 else {
                finish(.failure(.userCancelled))
                return
            }
            finish(.success(.area(area)))
        case .window:
            finish(.success(result))
        }
    }

    private func finish(_ result: Result<RegionCaptureResult, ScreenCaptureService.CaptureError>) {
        for window in windows { window.orderOut(nil) }
        windows.removeAll()
        NSCursor.arrow.set()
        let completion = completion
        self.completion = nil
        completion?(result)
    }
}

private final class RegionOverlayWindow: NSWindow {
    init(screen: NSScreen) {
        super.init(
            contentRect: screen.frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        level = .screenSaver
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        isOpaque = false
        backgroundColor = .clear
        ignoresMouseEvents = false
        acceptsMouseMovedEvents = true
        hasShadow = false
        isReleasedWhenClosed = false
    }

    override var canBecomeKey: Bool { true }
}

private final class RegionOverlayView: NSView {
    let screen: NSScreen
    let windows: [CaptureWindow]
    var onComplete: ((RegionCaptureResult) -> Void)?
    var onCancel: (() -> Void)?

    private var startPoint: CGPoint?
    private var currentPoint: CGPoint?
    private var hoveredWindow: CaptureWindow?

    init(screen: NSScreen, windows: [CaptureWindow]) {
        self.screen = screen
        self.windows = windows
        super.init(frame: CGRect(origin: .zero, size: screen.frame.size))
        wantsLayer = true
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 { onCancel?() } else { super.keyDown(with: event) }
    }

    override func mouseMoved(with event: NSEvent) {
        updateHover(at: localPoint(for: event))
    }

    override func mouseDown(with event: NSEvent) {
        let point = localPoint(for: event)
        updateHover(at: point)
        startPoint = point
        currentPoint = point
        needsDisplay = true
    }

    override func mouseDragged(with event: NSEvent) {
        currentPoint = localPoint(for: event)
        needsDisplay = true
    }

    override func mouseUp(with event: NSEvent) {
        currentPoint = localPoint(for: event)
        guard let startPoint, let currentPoint else {
            onCancel?()
            return
        }
        let localRect = RegionCaptureGeometry.selectionRect(from: startPoint, to: currentPoint)
        if localRect.width < RegionCaptureGeometry.dragThreshold,
           localRect.height < RegionCaptureGeometry.dragThreshold,
           let hoveredWindow {
            onComplete?(.window(hoveredWindow))
            return
        }

        let cocoa = RegionCaptureGeometry.localFlippedRectToGlobalCocoa(localRect, screenFrame: screen.frame)
        onComplete?(.area(CaptureArea(cocoaRect: cocoa, displayID: ScreenCoordinateSpace.displayID(for: screen))))
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.black.withAlphaComponent(0.34).setFill()
        if let selection = activeRect {
            let dimPath = NSBezierPath(rect: bounds)
            dimPath.append(NSBezierPath(rect: selection))
            dimPath.windingRule = .evenOdd
            dimPath.fill()
        } else {
            bounds.fill()
        }

        guard let selection = activeRect else { return }
        NSColor(calibratedRed: 0.95, green: 0.42, blue: 0.08, alpha: 1).setStroke()
        let path = NSBezierPath(rect: selection)
        path.lineWidth = 2
        path.stroke()

        let scale = ScreenCoordinateSpace.backingScale(for: screen)
        let text = "\(Int(selection.width * scale)) × \(Int(selection.height * scale))"
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 12, weight: .semibold),
            .foregroundColor: NSColor.white,
            .backgroundColor: NSColor.black.withAlphaComponent(0.55),
        ]
        text.draw(at: CGPoint(x: selection.minX + 8, y: max(selection.minY - 22, 8)), withAttributes: attrs)
    }

    private var selectionRect: CGRect? {
        guard let startPoint, let currentPoint else { return nil }
        return RegionCaptureGeometry.selectionRect(from: startPoint, to: currentPoint)
    }

    private var activeRect: CGRect? {
        if let selection = selectionRect,
           selection.width >= RegionCaptureGeometry.dragThreshold || selection.height >= RegionCaptureGeometry.dragThreshold {
            return selection
        }
        guard let frame = hoveredWindow?.frame else { return selectionRect }
        let local = RegionCaptureGeometry.globalCocoaRectToLocalFlipped(frame, screenFrame: screen.frame)
        return local.intersection(bounds)
    }

    private func localPoint(for event: NSEvent) -> CGPoint {
        convert(event.locationInWindow, from: nil)
    }

    private func updateHover(at point: CGPoint) {
        guard startPoint == nil else { return }
        let cocoa = RegionCaptureGeometry.localFlippedPointToGlobalCocoa(point, screenFrame: screen.frame)
        hoveredWindow = windows.first { window in
            guard let frame = window.frame else { return false }
            return frame.contains(cocoa)
        }
        needsDisplay = true
    }
}

private enum RegionWindowCatalog {
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
                frame: cgWindowBoundsToCocoaRect(bounds)
            )
        }
    }

    static func cgWindowBoundsToCocoaRect(_ bounds: CGRect) -> CGRect {
        let union = NSScreen.screens.reduce(CGRect.null) { partial, screen in
            partial.union(screen.frame)
        }
        let maxY = union.isNull ? bounds.maxY : union.maxY
        return CGRect(
            x: bounds.minX,
            y: maxY - bounds.maxY,
            width: bounds.width,
            height: bounds.height
        ).standardized
    }
}
