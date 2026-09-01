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
    private var localMouseMoveMonitor: Any?
    private var globalMouseMoveMonitor: Any?
    private var hasFinished = false

#if DEBUG
    var debugOverlayWindows: [NSWindow] { windows.map { $0 as NSWindow } }
    /// Dim band frames per overlay view, in each view's unflipped coordinates.
    var debugDimBandFrames: [[CGRect]] { windows.compactMap { ($0.contentView as? RegionOverlayView)?.dimBandFrames } }
#endif

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
        installMouseMoveMonitors()
        let capturableWindows = CaptureWindowCatalog.currentWindows()
        for screen in NSScreen.screens {
            let window = RegionOverlayWindow(screen: screen)
            window.setFrame(screen.frame, display: true)
            window.onEscape = { [weak self] in self?.finish(.failure(.userCancelled)) }
            let windowsForScreen = capturableWindows.filter { window in
                guard let frame = window.frame else { return false }
                return frame.intersects(screen.frame)
            }
            let view = RegionOverlayView(screen: screen, windows: windowsForScreen)
            view.onComplete = { [weak self] result in self?.finish(result) }
            view.onCancel = { [weak self] in self?.finish(.failure(.userCancelled)) }
            window.contentView = view
            window.orderFrontRegardless()
            windows.append(window)
        }
        (window(containing: NSEvent.mouseLocation) ?? windows.first)?.makeKeyAndOrderFront(nil)
        updateHoverForCurrentMouseLocation()
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

    private func installMouseMoveMonitors() {
        if let localMouseMoveMonitor {
            NSEvent.removeMonitor(localMouseMoveMonitor)
        }
        if let globalMouseMoveMonitor {
            NSEvent.removeMonitor(globalMouseMoveMonitor)
        }
        localMouseMoveMonitor = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved]) { [weak self] event in
            self?.updateHoverForCurrentMouseLocation()
            return event
        }
        globalMouseMoveMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.mouseMoved]) { [weak self] _ in
            Task { @MainActor in
                self?.updateHoverForCurrentMouseLocation()
            }
        }
    }

    private func updateHoverForCurrentMouseLocation() {
        let point = NSEvent.mouseLocation
        for window in windows {
            (window.contentView as? RegionOverlayView)?.updateHoverFromGlobalMouseLocation(point)
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
        if let localMouseMoveMonitor {
            NSEvent.removeMonitor(localMouseMoveMonitor)
            self.localMouseMoveMonitor = nil
        }
        if let globalMouseMoveMonitor {
            NSEvent.removeMonitor(globalMouseMoveMonitor)
            self.globalMouseMoveMonitor = nil
        }
        for window in windows { window.orderOut(nil) }
        windows.removeAll()
        NSCursor.arrow.set()
    }

    private func window(containing point: CGPoint) -> RegionOverlayWindow? {
        windows.first { $0.frame.contains(point) }
    }
}

private final class RegionOverlayWindow: NSPanel {
    var onEscape: (() -> Void)?

    init(screen: NSScreen) {
        super.init(
            contentRect: screen.frame,
            styleMask: CaptureOverlayPanelConfiguration.styleMask,
            backing: .buffered,
            defer: false
        )
        CaptureOverlayPanelConfiguration.apply(to: self)
        setFrame(screen.frame, display: true)
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

    private let dimmingView: CaptureDimmingView
    private var startPoint: CGPoint?
    private var currentPoint: CGPoint?
    private var hoveredWindow: CaptureWindow?
    private var trackingArea: NSTrackingArea?
    fileprivate var dimBandFrames: [CGRect] { dimmingView.dimBandFrames }

    init(screen: NSScreen, windows: [CaptureWindow]) {
        self.screen = screen
        self.windows = windows
        let frame = CGRect(origin: .zero, size: screen.frame.size)
        dimmingView = CaptureDimmingView(frame: frame, contentsScale: ScreenCoordinateSpace.backingScale(for: screen))
        super.init(frame: frame)
        wantsLayer = true
        addSubview(dimmingView)
        refreshChrome()
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        window?.invalidateCursorRects(for: self)
    }

    override func resetCursorRects() {
        super.resetCursorRects()
        addCursorRect(bounds, cursor: .crosshair)
    }

    override func cursorUpdate(with event: NSEvent) {
        NSCursor.crosshair.set()
    }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 { onCancel?() } else { super.keyDown(with: event) }
    }

    override func mouseMoved(with event: NSEvent) {
        updateHover(at: localPoint(for: event))
    }

    override func mouseExited(with event: NSEvent) {
        if hoveredWindow != nil {
            hoveredWindow = nil
            refreshChrome()
        }
    }

    override func updateTrackingAreas() {
        if let trackingArea {
            removeTrackingArea(trackingArea)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.activeAlways, .mouseMoved, .mouseEnteredAndExited, .cursorUpdate, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        trackingArea = area
        super.updateTrackingAreas()
    }

    override func mouseDown(with event: NSEvent) {
        let point = localPoint(for: event)
        updateHover(at: point)
        startPoint = point
        currentPoint = point
        refreshChrome()
    }

    override func mouseDragged(with event: NSEvent) {
        currentPoint = localPoint(for: event)
        refreshChrome()
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
                displayID: displayID,
                policy: .areaOrWindow
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

    /// Pushes the current selection into the layer-backed chrome. Cheap enough to call on
    /// every mouse event.
    private func refreshChrome() {
        let selection = activeRect.flatMap { $0.isNull || $0.isEmpty ? nil : $0 }
        let label = selection.map {
            CaptureDimmingView.sizeLabel(for: $0, scale: ScreenCoordinateSpace.backingScale(for: screen))
        }
        dimmingView.update(
            selection: selection.map { CaptureDimmingView.flip($0, height: bounds.height) },
            borderWidth: 2,
            label: label
        )
    }

    private func localPoint(for event: NSEvent) -> CGPoint {
        convert(event.locationInWindow, from: nil)
    }

    private func updateHover(at point: CGPoint) {
        guard startPoint == nil else { return }
        let cocoa = RegionCaptureGeometry.localFlippedPointToGlobalCocoa(point, screenFrame: screen.frame)
        let previousHover = hoveredWindow
        hoveredWindow = windows.first { window in
            guard let frame = window.frame else { return false }
            return frame.contains(cocoa)
        }
        if previousHover != hoveredWindow {
            refreshChrome()
        }
    }

    fileprivate func updateHoverFromGlobalMouseLocation(_ point: CGPoint) {
        let local = RegionCaptureGeometry.globalCocoaPointToLocalFlipped(point, screenFrame: screen.frame)
        guard bounds.contains(local) else {
            if hoveredWindow != nil {
                hoveredWindow = nil
                refreshChrome()
            }
            return
        }
        updateHover(at: local)
    }

    private func resetInteraction(at point: CGPoint) {
        startPoint = nil
        currentPoint = nil
        hoveredWindow = nil
        updateHover(at: point)
        refreshChrome()
    }
}
