import AppKit
import SwiftUI

@MainActor
final class CaptureOverlayController {
    private enum Purpose {
        case screenshot
        case recording(RecordingOptions, (CaptureSelection, RecordingOptions) -> Void)
    }

    private let screenCaptureService: ScreenCaptureService
    private let settings: AppSettings
    private let macOCRService: MacVisionOCRService

    private var windows: [CaptureOverlayWindow] = []
    private var overlayViews: [CaptureOverlayView] = []
    private var snapshots: [DisplaySnapshot] = []
    private var purpose: Purpose?
    private var captureTask: Task<Void, Never>?
    private var captureToken = 0
    private var refreshTask: Task<Void, Never>?
    private var keyMonitor: Any?
    private var activeScreenshotViewModel: ScreenshotPopupViewModel?
    private var onError: ((String) -> Void)?
    private var onCancel: (() -> Void)?
    private var resignActiveObserver: NSObjectProtocol?

    init(
        screenCaptureService: ScreenCaptureService,
        settings: AppSettings,
        macOCRService: MacVisionOCRService
    ) {
        self.screenCaptureService = screenCaptureService
        self.settings = settings
        self.macOCRService = macOCRService
    }

    deinit {
        // SelectionOverlayController owns and releases this UI controller on the main actor.
        // Deinit cannot call an actor-isolated method directly, but teardown must still
        // order out screen-level panels if a controller is replaced unexpectedly.
        MainActor.assumeIsolated {
            cancel()
        }
    }

    func beginScreenshot(
        onError: @escaping (String) -> Void,
        onCancel: @escaping () -> Void = {}
    ) {
        begin(
            purpose: .screenshot,
            options: CaptureOptions(
                includeCursor: settings.screenshotIncludeCursor,
                hideBelloBoxWindows: true,
                delayAfterHidingOverlays: 0.12
            ),
            onError: onError,
            onCancel: onCancel
        )
    }

    func beginRecording(
        initialOptions: RecordingOptions,
        onRecord: @escaping (CaptureSelection, RecordingOptions) -> Void,
        onError: @escaping (String) -> Void,
        onCancel: @escaping () -> Void = {}
    ) {
        begin(
            purpose: .recording(initialOptions, onRecord),
            options: CaptureOptions(includeCursor: false, hideBelloBoxWindows: true, delayAfterHidingOverlays: 0.12),
            onError: onError,
            onCancel: onCancel
        )
    }

#if DEBUG
    func beginScreenshotForTesting(
        snapshots: [DisplaySnapshot],
        initialSelection: CaptureSelection? = nil,
        onError: @escaping (String) -> Void,
        onCancel: @escaping () -> Void = {}
    ) {
        cancel()
        self.purpose = .screenshot
        self.onError = onError
        self.onCancel = onCancel
        self.snapshots = snapshots
        showOverlayWindows(snapshots: snapshots)
        if let initialSelection,
           let selectedView = overlayViews.first(where: { $0.snapshot.displayID == snapshot(for: initialSelection)?.displayID }) ?? overlayViews.first {
            handle(selection: initialSelection, in: selectedView)
        }
    }
#endif

    func cancel() {
        captureToken += 1
        captureTask?.cancel()
        captureTask = nil
        refreshTask?.cancel()
        refreshTask = nil
        let closingScreenshotViewModel = activeScreenshotViewModel
        activeScreenshotViewModel = nil
        if let keyMonitor {
            NSEvent.removeMonitor(keyMonitor)
            self.keyMonitor = nil
        }
        if let resignActiveObserver {
            NotificationCenter.default.removeObserver(resignActiveObserver)
            self.resignActiveObserver = nil
        }
        for window in windows { window.orderOut(nil) }
        windows.removeAll()
        overlayViews.removeAll()
        snapshots.removeAll()
        purpose = nil
        onError = nil
        onCancel = nil
        NSCursor.arrow.set()
        closingScreenshotViewModel?.close()
    }

    private func begin(
        purpose: Purpose,
        options: CaptureOptions,
        onError: @escaping (String) -> Void,
        onCancel: @escaping () -> Void
    ) {
        cancel()
        self.purpose = purpose
        self.onError = onError
        self.onCancel = onCancel

        captureToken += 1
        let token = captureToken
        captureTask = Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let snapshots = try await screenCaptureService.captureDisplaySnapshots(options: options)
                guard !Task.isCancelled, self.captureToken == token else { return }
                self.snapshots = snapshots
                self.showOverlayWindows(snapshots: snapshots)
                if self.captureToken == token {
                    self.captureTask = nil
                }
            } catch {
                guard !Task.isCancelled, self.captureToken == token else { return }
                self.cancel()
                onError(error.localizedDescription)
            }
        }
    }

    private func showOverlayWindows(snapshots: [DisplaySnapshot]) {
        let capturableWindows = CaptureWindowCatalog.currentWindows()

        for screen in NSScreen.screens {
            guard let displayID = ScreenCoordinateSpace.displayID(for: screen),
                  let snapshot = snapshots.first(where: { $0.displayID == displayID })
            else { continue }

            let window = CaptureOverlayWindow(screen: screen)
            window.onEscape = { [weak self] in self?.cancelFromUser() }
            let overlayView = CaptureOverlayView(
                screen: screen,
                snapshot: snapshot,
                windows: capturableWindows
            )
            overlayView.onSelection = { [weak self, weak overlayView] selection in
                guard let overlayView else { return }
                self?.handle(selection: selection, in: overlayView)
            }
            overlayView.onCancel = { [weak self] in
                self?.cancelFromUser()
            }
            window.contentView = overlayView
            window.orderFrontRegardless()
            windows.append(window)
            overlayViews.append(overlayView)
        }

        guard !windows.isEmpty else {
            let reportError = onError
            cancel()
            reportError?(ScreenCaptureService.CaptureError.noDisplayFound.localizedDescription)
            return
        }

        AppActivation.bringAppForward()
        installKeyMonitor()
        installResignActiveObserver()
        windows.first?.makeKeyAndOrderFront(nil)
        NSCursor.crosshair.set()
    }

    private func installKeyMonitor() {
        if let keyMonitor {
            NSEvent.removeMonitor(keyMonitor)
        }
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { [weak self] event in
            guard event.keyCode == 53 else { return event }
            Task { @MainActor in self?.cancelFromUser() }
            return nil
        }
    }

    private func installResignActiveObserver() {
        if let resignActiveObserver {
            NotificationCenter.default.removeObserver(resignActiveObserver)
        }
        resignActiveObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didResignActiveNotification,
            object: NSApp,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.cancelSelectionPhaseAfterFocusLoss()
            }
        }
    }

    private func cancelSelectionPhaseAfterFocusLoss() {
        guard purpose != nil else { return }
        guard activeScreenshotViewModel == nil else { return }
        guard overlayViews.allSatisfy({ !$0.hasLockedSelection }) else { return }
        cancelFromUser()
    }

    private func cancelFromUser() {
        guard captureTask != nil || purpose != nil || !windows.isEmpty else { return }
        onCancel?()
        cancel()
    }

    private func handle(selection: CaptureSelection, in selectedView: CaptureOverlayView) {
        guard let purpose else { return }
        selectedView.window?.makeKeyAndOrderFront(nil)

        for overlayView in overlayViews {
            overlayView.lock(selection: overlayView === selectedView ? selection : nil)
        }

        switch purpose {
        case .screenshot:
            showScreenshotEditor(for: selection, in: selectedView)
        case let .recording(initialOptions, onRecord):
            selectedView.showRecordingOptions(
                settings: settings,
                initialOptions: initialOptions,
                targetLabel: label(for: selection),
                selection: selection,
                onStart: { [weak self] options in
                    self?.cancel()
                    onRecord(selection, options)
                },
                onCancel: { [weak self] in
                    self?.cancelFromUser()
                }
            )
        }
    }

    private func showScreenshotEditor(for selection: CaptureSelection, in selectedView: CaptureOverlayView) {
        do {
            guard let snapshot = snapshot(for: selection) else {
                throw ScreenCaptureService.CaptureError.noDisplayFound
            }
            let document = try screenCaptureService.document(
                fromSnapshot: snapshot,
                cocoaRect: selection.cocoaRect,
                source: screenshotSource(for: selection)
            )
            let viewModel = ScreenshotPopupViewModel(
                document: document,
                settings: settings,
                macOCRService: macOCRService
            )
            activeScreenshotViewModel = viewModel
            viewModel.onClose = { [weak self, weak viewModel] in
                if self?.activeScreenshotViewModel === viewModel {
                    self?.activeScreenshotViewModel = nil
                }
                self?.cancelFromUser()
            }
            selectedView.showScreenshotEditor(viewModel: viewModel, selection: selection)
            refreshWindowScreenshotIfNeeded(selection: selection, viewModel: viewModel)
        } catch {
            let message = error.localizedDescription
            let reportError = onError
            cancel()
            reportError?(message)
        }
    }

    private func refreshWindowScreenshotIfNeeded(selection: CaptureSelection, viewModel: ScreenshotPopupViewModel) {
        guard case let .window(window) = selection else { return }
        let originalID = viewModel.document.id
        refreshTask?.cancel()
        refreshTask = Task { @MainActor [weak self, weak viewModel] in
            guard let self, let viewModel else { return }
            do {
                let refreshed = try await screenCaptureService.capture(
                    .window(window),
                    options: CaptureOptions(
                        includeCursor: settings.screenshotIncludeCursor,
                        hideBelloBoxWindows: false,
                        delayAfterHidingOverlays: 0
                    )
                )
                guard !Task.isCancelled else { return }
                viewModel.refreshBaseCapture(from: refreshed, expectedDocumentID: originalID)
            } catch {
                guard !Task.isCancelled else { return }
                // The frozen crop is already usable; a failed fidelity refresh
                // should not interrupt editing.
            }
        }
    }

    private func snapshot(for selection: CaptureSelection) -> DisplaySnapshot? {
        switch selection {
        case let .area(area):
            if let displayID = area.displayID,
               let snapshot = snapshots.first(where: { $0.displayID == displayID }) {
                return snapshot
            }
            return snapshot(containing: area.cocoaRect)
        case let .display(display):
            return snapshots.first(where: { $0.displayID == display.displayID })
        case let .window(window):
            guard let frame = window.frame else { return nil }
            return snapshot(containing: frame)
        }
    }

    private func snapshot(containing rect: CGRect) -> DisplaySnapshot? {
        snapshots
            .map { ($0, $0.screenFrame.intersection(rect).area) }
            .filter { $0.1 > 0 }
            .sorted { $0.1 > $1.1 }
            .first?.0
    }

    private func screenshotSource(for selection: CaptureSelection) -> ScreenshotSource {
        switch selection {
        case let .area(area):
            return .area(rect: area.cocoaRect, displayID: area.displayID)
        case let .display(display):
            return .display(displayID: display.displayID)
        case let .window(window):
            return .window(title: window.title, ownerName: window.ownerName, windowID: window.windowID)
        }
    }

    private func label(for selection: CaptureSelection) -> String {
        switch selection {
        case .area:
            return "Record Area"
        case let .display(display):
            let width = Int(display.frame.width)
            let height = Int(display.frame.height)
            return "Record Screen \(width)x\(height)"
        case let .window(window):
            let title = [window.ownerName, window.title].compactMap { $0 }.joined(separator: " - ")
            return title.isEmpty ? "Record Window" : title
        }
    }
}

private final class CaptureOverlayWindow: NSPanel {
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
        hasShadow = false
        isReleasedWhenClosed = false
        ignoresMouseEvents = false
        acceptsMouseMovedEvents = true
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }

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

private final class CaptureOverlayView: NSView {
    let screen: NSScreen
    let snapshot: DisplaySnapshot
    let windows: [CaptureWindow]

    var onSelection: ((CaptureSelection) -> Void)?
    var onCancel: (() -> Void)?

    private var startPoint: CGPoint?
    private var currentPoint: CGPoint?
    private var hoveredWindow: CaptureWindow?
    private var lockedSelection: CaptureSelection?
    private var accessoryView: NSView?
    fileprivate var hasLockedSelection: Bool { lockedSelection != nil }

    init(screen: NSScreen, snapshot: DisplaySnapshot, windows: [CaptureWindow]) {
        self.screen = screen
        self.snapshot = snapshot
        self.windows = windows
        super.init(frame: CGRect(origin: .zero, size: screen.frame.size))
        wantsLayer = true
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var acceptsFirstResponder: Bool { true }
    override var isFlipped: Bool { true }

    func lock(selection: CaptureSelection?) {
        lockedSelection = selection
        startPoint = nil
        currentPoint = nil
        hoveredWindow = nil
        needsDisplay = true
    }

    func showScreenshotEditor(viewModel: ScreenshotPopupViewModel, selection: CaptureSelection) {
        installHostingView(
            CaptureScreenshotOverlaySurface(
                viewModel: viewModel,
                screenFrame: screen.frame,
                selectionFrame: selection.cocoaRect,
                onCancel: viewModel.close
            )
        )
    }

    func showRecordingOptions(
        settings: AppSettings,
        initialOptions: RecordingOptions,
        targetLabel: String,
        selection: CaptureSelection,
        onStart: @escaping (RecordingOptions) -> Void,
        onCancel: @escaping () -> Void
    ) {
        installHostingView(
            CaptureRecordingOverlaySurface(
                settings: settings,
                initialOptions: initialOptions,
                targetLabel: targetLabel,
                screenFrame: screen.frame,
                selectionFrame: selection.cocoaRect,
                onStart: onStart,
                onCancel: onCancel
            )
        )
    }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 { onCancel?() } else { super.keyDown(with: event) }
    }

    override func mouseMoved(with event: NSEvent) {
        guard lockedSelection == nil else { return }
        updateHover(at: localPoint(for: event))
    }

    override func mouseDown(with event: NSEvent) {
        guard lockedSelection == nil else { return }
        let point = localPoint(for: event)
        updateHover(at: point)
        startPoint = point
        currentPoint = point
        needsDisplay = true
    }

    override func mouseDragged(with event: NSEvent) {
        guard lockedSelection == nil else { return }
        currentPoint = localPoint(for: event)
        needsDisplay = true
    }

    override func mouseUp(with event: NSEvent) {
        guard lockedSelection == nil else { return }
        currentPoint = localPoint(for: event)
        guard let displayID = ScreenCoordinateSpace.displayID(for: screen),
              let selection = CaptureSelectionResolver.resolve(
                startLocal: startPoint,
                endLocal: currentPoint ?? localPoint(for: event),
                hoveredWindow: hoveredWindow,
                screenFrame: screen.frame,
                displayID: displayID
              )
        else {
            resetInteraction(at: currentPoint ?? localPoint(for: event))
            return
        }
        onSelection?(selection)
    }

    override func draw(_ dirtyRect: NSRect) {
        drawSnapshot()
        drawDimAndSelection()
    }

    private var activeRect: CGRect? {
        if let lockedSelection {
            return localRect(for: lockedSelection.cocoaRect)
        }
        if let selection = selectionRect,
           selection.width >= RegionCaptureGeometry.dragThreshold || selection.height >= RegionCaptureGeometry.dragThreshold {
            return selection
        }
        guard let frame = hoveredWindow?.frame else { return selectionRect }
        return RegionCaptureGeometry.globalCocoaRectToLocalFlipped(frame, screenFrame: screen.frame).intersection(bounds)
    }

    private var selectionRect: CGRect? {
        guard let startPoint, let currentPoint else { return nil }
        return RegionCaptureGeometry.clampedSelectionRect(from: startPoint, to: currentPoint, bounds: bounds)
    }

    private func drawSnapshot() {
        let image = NSImage(cgImage: snapshot.image, size: screen.frame.size)
        NSGraphicsContext.current?.imageInterpolation = .high
        image.draw(in: bounds)
    }

    private func drawDimAndSelection() {
        NSColor.black.withAlphaComponent(0.34).setFill()
        if let selection = activeRect, !selection.isEmpty {
            let dimPath = NSBezierPath(rect: bounds)
            dimPath.append(NSBezierPath(rect: selection))
            dimPath.windingRule = .evenOdd
            dimPath.fill()
        } else {
            bounds.fill()
        }

        guard let selection = activeRect, !selection.isEmpty else { return }
        NSColor(calibratedRed: 0.95, green: 0.42, blue: 0.08, alpha: 1).setStroke()
        let path = NSBezierPath(rect: selection)
        path.lineWidth = lockedSelection == nil ? 2 : 2.5
        path.stroke()

        guard lockedSelection == nil else { return }
        let scale = snapshot.scale
        let text = "\(Int(selection.width * scale)) x \(Int(selection.height * scale))"
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 12, weight: .semibold),
            .foregroundColor: NSColor.white,
            .backgroundColor: NSColor.black.withAlphaComponent(0.55),
        ]
        text.draw(at: CGPoint(x: selection.minX + 8, y: max(selection.minY - 22, 8)), withAttributes: attrs)
    }

    private func localPoint(for event: NSEvent) -> CGPoint {
        convert(event.locationInWindow, from: nil)
    }

    private func localRect(for cocoaRect: CGRect) -> CGRect {
        RegionCaptureGeometry.globalCocoaRectToLocalFlipped(cocoaRect, screenFrame: screen.frame)
            .intersection(bounds)
            .standardized
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

    private func installHostingView<Content: View>(_ content: Content) {
        accessoryView?.removeFromSuperview()
        let hostingView = NSHostingView(rootView: content)
        hostingView.frame = bounds
        hostingView.autoresizingMask = [.width, .height]
        hostingView.wantsLayer = true
        hostingView.layer?.backgroundColor = NSColor.clear.cgColor
        addSubview(hostingView)
        accessoryView = hostingView
    }
}

struct CaptureOverlayAccessoryLayout {
    static func frame(
        selection: CGRect,
        bounds: CGRect,
        preferredSize: CGSize,
        gap: CGFloat = 10,
        inset: CGFloat = 12
    ) -> CGRect {
        let width = min(preferredSize.width, max(280, bounds.width - inset * 2))
        let height = min(preferredSize.height, max(52, bounds.height - inset * 2))
        let x = min(max(selection.minX, inset), bounds.maxX - width - inset)
        var y = selection.minY - height - gap
        if y < bounds.minY + inset {
            y = selection.maxY + gap
        }
        y = min(max(y, bounds.minY + inset), bounds.maxY - height - inset)
        return CGRect(x: x, y: y, width: width, height: height)
    }
}

private struct CaptureScreenshotOverlaySurface: View {
    @ObservedObject var viewModel: ScreenshotPopupViewModel
    var screenFrame: CGRect
    var selectionFrame: CGRect
    var onCancel: () -> Void

    var body: some View {
        GeometryReader { geometry in
            let bounds = CGRect(origin: .zero, size: geometry.size)
            let selected = localSelectionFrame(in: bounds)
            let toolbar = CaptureOverlayAccessoryLayout.frame(
                selection: selected,
                bounds: bounds,
                preferredSize: CGSize(width: 900, height: 54)
            )

            ZStack(alignment: .topLeading) {
                Color.clear

                AnnotationCanvasView(viewModel: viewModel)
                    .frame(width: max(selected.width, 1), height: max(selected.height, 1))
                    .clipShape(Rectangle())
                    .overlay(Rectangle().strokeBorder(BoxTheme.accent, lineWidth: 2))
                    .position(x: selected.midX, y: selected.midY)

                AnnotationToolbarView(viewModel: viewModel, showExportActions: true, onClose: onCancel)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
                    .frame(width: toolbar.width, height: toolbar.height)
                    .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(.regularMaterial))
                    .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(.white.opacity(0.14), lineWidth: 1))
                    .shadow(color: .black.opacity(0.28), radius: 16, y: 8)
                    .position(x: toolbar.midX, y: toolbar.midY)

                if let message = viewModel.errorMessage {
                    errorLabel(message)
                        .frame(width: min(toolbar.width, bounds.width - 24), alignment: .leading)
                        .position(x: toolbar.midX, y: min(toolbar.maxY + 28, bounds.maxY - 28))
                }
            }
            .onExitCommand(perform: onCancel)
        }
    }

    private func localSelectionFrame(in bounds: CGRect) -> CGRect {
        RegionCaptureGeometry
            .globalCocoaRectToLocalFlipped(selectionFrame, screenFrame: screenFrame)
            .intersection(bounds)
            .standardized
    }

    private func errorLabel(_ message: String) -> some View {
        Label(message, systemImage: "exclamationmark.triangle.fill")
            .font(.caption)
            .foregroundStyle(.orange)
            .lineLimit(2)
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(.regularMaterial))
    }
}

private struct CaptureRecordingOverlaySurface: View {
    @ObservedObject var settings: AppSettings
    var initialOptions: RecordingOptions
    var targetLabel: String
    var screenFrame: CGRect
    var selectionFrame: CGRect
    var onStart: (RecordingOptions) -> Void
    var onCancel: () -> Void

    var body: some View {
        GeometryReader { geometry in
            let bounds = CGRect(origin: .zero, size: geometry.size)
            let selected = localSelectionFrame(in: bounds)
            let bar = CaptureOverlayAccessoryLayout.frame(
                selection: selected,
                bounds: bounds,
                preferredSize: CGSize(width: 780, height: 330)
            )

            ZStack(alignment: .topLeading) {
                Color.clear

                RecordingOptionsBar(
                    settings: settings,
                    targetLabel: targetLabel,
                    initialOptions: initialOptions,
                    onStart: onStart,
                    onCancel: onCancel
                )
                .frame(width: bar.width, height: bar.height, alignment: .topLeading)
                .position(x: bar.midX, y: bar.midY)
            }
            .onExitCommand(perform: onCancel)
        }
    }

    private func localSelectionFrame(in bounds: CGRect) -> CGRect {
        RegionCaptureGeometry
            .globalCocoaRectToLocalFlipped(selectionFrame, screenFrame: screenFrame)
            .intersection(bounds)
            .standardized
    }
}

private extension CGRect {
    var area: CGFloat {
        guard !isNull, !isEmpty else { return 0 }
        return width * height
    }
}
