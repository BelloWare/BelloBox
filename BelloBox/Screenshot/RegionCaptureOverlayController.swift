import AppKit

enum RegionCaptureResult: Equatable {
    case area(CaptureArea)
    case window(CaptureWindow)
}

@MainActor
final class RegionCaptureOverlayController {
    private var windows: [RegionOverlayWindow] = []
    private var completion: ((Result<RegionCaptureResult, ScreenCaptureService.CaptureError>) -> Void)?
    private var keyMonitor: Any?
    private var hasFinished = false

    deinit {
        // SelectionOverlayController owns and releases this UI controller on the main actor.
        // Deinit cannot call an actor-isolated method directly, but teardown must still
        // order out screen-level panels if a controller is replaced unexpectedly.
        MainActor.assumeIsolated {
            cancel()
        }
    }

    func begin(completion: @escaping (Result<RegionCaptureResult, ScreenCaptureService.CaptureError>) -> Void) {
        cancel()
        self.completion = completion
        hasFinished = false
        installKeyMonitor()
        let capturableWindows = CaptureWindowCatalog.currentWindows()
        AppActivation.bringAppForward()
        for screen in NSScreen.screens {
            let window = RegionOverlayWindow(screen: screen)
            window.onEscape = { [weak self] in self?.finish(.failure(.userCancelled)) }
            let view = RegionOverlayView(screen: screen, windows: capturableWindows)
            view.onComplete = { [weak self] result in self?.finish(result) }
            view.onCancel = { [weak self] in self?.finish(.failure(.userCancelled)) }
            window.contentView = view
            window.makeKeyAndOrderFront(nil)
            windows.append(window)
        }
        NSCursor.crosshair.set()
    }

    func cancel() {
        cleanup()
        completion = nil
        hasFinished = false
    }

    private func installKeyMonitor() {
        if let keyMonitor {
            NSEvent.removeMonitor(keyMonitor)
        }
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { [weak self] event in
            guard event.keyCode == 53 else { return event }
            Task { @MainActor in self?.finish(.failure(.userCancelled)) }
            return nil
        }
    }

    private func finish(_ result: RegionCaptureResult) {
        switch result {
        case let .area(area):
            guard area.cocoaRect.width >= RegionCaptureGeometry.minimumAreaSize,
                  area.cocoaRect.height >= RegionCaptureGeometry.minimumAreaSize
            else {
                finish(.failure(.userCancelled))
                return
            }
            finish(.success(.area(area)))
        case .window:
            finish(.success(result))
        }
    }

    private func finish(_ result: Result<RegionCaptureResult, ScreenCaptureService.CaptureError>) {
        guard !hasFinished else { return }
        hasFinished = true
        cleanup()
        let completion = completion
        self.completion = nil
        completion?(result)
    }

    private func cleanup() {
        if let keyMonitor {
            NSEvent.removeMonitor(keyMonitor)
            self.keyMonitor = nil
        }
        for window in windows { window.orderOut(nil) }
        windows.removeAll()
        NSCursor.arrow.set()
    }
}

private final class RegionOverlayWindow: NSWindow {
    var onEscape: (() -> Void)?

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

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 {
            onEscape?()
        } else {
            super.keyDown(with: event)
        }
    }

    override func cancelOperation(_ sender: Any?) {
        onEscape?()
    }
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
        let endPoint = currentPoint ?? localPoint(for: event)
        guard let displayID = ScreenCoordinateSpace.displayID(for: screen),
              let selection = CaptureSelectionResolver.resolve(
                startLocal: startPoint,
                endLocal: endPoint,
                hoveredWindow: hoveredWindow,
                screenFrame: screen.frame,
                displayID: displayID
              )
        else {
            resetInteraction(at: endPoint)
            return
        }

        switch selection {
        case let .area(area):
            onComplete?(.area(area))
        case let .window(window):
            onComplete?(.window(window))
        case .display:
            resetInteraction(at: endPoint)
        }
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
        return RegionCaptureGeometry.clampedSelectionRect(from: startPoint, to: currentPoint, bounds: bounds)
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

    private func resetInteraction(at point: CGPoint) {
        startPoint = nil
        currentPoint = nil
        hoveredWindow = nil
        updateHover(at: point)
        needsDisplay = true
    }
}
