import AppKit

@MainActor
final class RegionCaptureOverlayController {
    private var windows: [RegionOverlayWindow] = []
    private var completion: ((Result<CaptureArea, ScreenCaptureService.CaptureError>) -> Void)?

    func begin(completion: @escaping (Result<CaptureArea, ScreenCaptureService.CaptureError>) -> Void) {
        cancel()
        self.completion = completion
        for screen in NSScreen.screens {
            let window = RegionOverlayWindow(screen: screen)
            let view = RegionOverlayView(screen: screen)
            view.onComplete = { [weak self] rect in self?.finish(rect: rect, screen: screen) }
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

    private func finish(rect: CGRect, screen: NSScreen) {
        guard rect.width >= 8, rect.height >= 8 else {
            finish(.failure(.userCancelled))
            return
        }
        let area = CaptureArea(cocoaRect: rect.standardized, displayID: ScreenCoordinateSpace.displayID(for: screen))
        finish(.success(area))
    }

    private func finish(_ result: Result<CaptureArea, ScreenCaptureService.CaptureError>) {
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
    var onComplete: ((CGRect) -> Void)?
    var onCancel: (() -> Void)?

    private var startPoint: CGPoint?
    private var currentPoint: CGPoint?

    init(screen: NSScreen) {
        self.screen = screen
        super.init(frame: CGRect(origin: .zero, size: screen.frame.size))
        wantsLayer = true
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 { onCancel?() } else { super.keyDown(with: event) }
    }

    override func mouseDown(with event: NSEvent) {
        startPoint = event.locationInWindow
        currentPoint = event.locationInWindow
        needsDisplay = true
    }

    override func mouseDragged(with event: NSEvent) {
        currentPoint = event.locationInWindow
        needsDisplay = true
    }

    override func mouseUp(with event: NSEvent) {
        currentPoint = event.locationInWindow
        guard let startPoint, let currentPoint else {
            onCancel?()
            return
        }
        let localRect = CGRect(
            x: min(startPoint.x, currentPoint.x),
            y: min(startPoint.y, currentPoint.y),
            width: abs(startPoint.x - currentPoint.x),
            height: abs(startPoint.y - currentPoint.y)
        )
        let cocoa = localFlippedRectToGlobalCocoa(localRect)
        onComplete?(cocoa)
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.black.withAlphaComponent(0.34).setFill()
        if let selection = selectionRect {
            let dimPath = NSBezierPath(rect: bounds)
            dimPath.append(NSBezierPath(rect: selection))
            dimPath.windingRule = .evenOdd
            dimPath.fill()
        } else {
            bounds.fill()
        }

        guard let selection = selectionRect else { return }
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
        return CGRect(
            x: min(startPoint.x, currentPoint.x),
            y: min(startPoint.y, currentPoint.y),
            width: abs(startPoint.x - currentPoint.x),
            height: abs(startPoint.y - currentPoint.y)
        )
    }

    private func localFlippedRectToGlobalCocoa(_ rect: CGRect) -> CGRect {
        CGRect(
            x: screen.frame.minX + rect.minX,
            y: screen.frame.maxY - rect.maxY,
            width: rect.width,
            height: rect.height
        )
    }
}
