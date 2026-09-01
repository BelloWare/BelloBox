import AppKit
import SwiftUI

@MainActor
final class CaptureOverlayController {
    private enum Purpose {
        case screenshot(CaptureSelectionPolicy)
        case recording(RecordingOptions, (CaptureSelection, RecordingOptions) -> Void)

        var diagnosticsName: String {
            switch self {
            case let .screenshot(policy): return "screenshot(\(policy))"
            case .recording: return "recording"
            }
        }
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
    private var localMouseMoveMonitor: Any?
    private var globalMouseMoveMonitor: Any?
    private var activeScreenshotViewModel: ScreenshotPopupViewModel?
    private var onError: ((String) -> Void)?
    private var onCancel: (() -> Void)?
    private var resignActiveObserver: NSObjectProtocol?
    private var overlayTiming: CaptureTiming?

    init(
        screenCaptureService: ScreenCaptureService,
        settings: AppSettings,
        macOCRService: MacVisionOCRService
    ) {
        self.screenCaptureService = screenCaptureService
        self.settings = settings
        self.macOCRService = macOCRService
    }

#if DEBUG
    var debugOverlayWindowCount: Int { windows.count }
    var debugOverlayWindows: [NSWindow] { windows.map { $0 as NSWindow } }
    /// Dim band frames per overlay view, in each view's unflipped coordinates.
    var debugDimBandFrames: [[CGRect]] { overlayViews.map(\.dimBandFrames) }
    /// The visible selection cut-out per overlay view, in flipped local coordinates.
    var debugSelectionFrames: [CGRect?] { overlayViews.map(\.visibleSelectionFrame) }
    var debugActiveScreenshotViewModel: ScreenshotPopupViewModel? { activeScreenshotViewModel }
#endif

    deinit {
        // SelectionOverlayController owns and releases this UI controller on the main actor.
        // Deinit cannot call an actor-isolated method directly, but teardown must still
        // order out screen-level panels if a controller is replaced unexpectedly.
        MainActor.assumeIsolated {
            cancel()
        }
    }

    func beginScreenshot(
        policy: CaptureSelectionPolicy = .any,
        onError: @escaping (String) -> Void,
        onCancel: @escaping () -> Void = {}
    ) {
        begin(
            purpose: .screenshot(policy),
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
            onError: onError,
            onCancel: onCancel
        )
    }

#if DEBUG
    func beginScreenshotForTesting(
        snapshots: [DisplaySnapshot],
        initialSelection: CaptureSelection? = nil,
        policy: CaptureSelectionPolicy = .any,
        onError: @escaping (String) -> Void,
        onCancel: @escaping () -> Void = {}
    ) {
        cancel()
        self.purpose = .screenshot(policy)
        self.onError = onError
        self.onCancel = onCancel
        self.snapshots = snapshots
        showOverlayWindows(snapshots: snapshots)
        if let initialSelection,
           let selectedView = overlayViews.first(where: { $0.snapshot?.displayID == snapshot(for: initialSelection)?.displayID }) ?? overlayViews.first {
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
        if let localMouseMoveMonitor {
            NSEvent.removeMonitor(localMouseMoveMonitor)
            self.localMouseMoveMonitor = nil
        }
        if let globalMouseMoveMonitor {
            NSEvent.removeMonitor(globalMouseMoveMonitor)
            self.globalMouseMoveMonitor = nil
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
        overlayTiming = nil
        NSCursor.arrow.set()
        closingScreenshotViewModel?.close()
    }

    private func begin(
        purpose: Purpose,
        onError: @escaping (String) -> Void,
        onCancel: @escaping () -> Void
    ) {
        cancel()
        self.purpose = purpose
        self.onError = onError
        self.onCancel = onCancel
        logDiagnostics(
            "overlay.begin",
            [
                "purpose=\(purpose.diagnosticsName)",
                "mouse=\(Self.serialize(NSEvent.mouseLocation))",
                "screens=\(Self.screenSummary())",
            ]
        )

        captureToken += 1
        overlayTiming = CaptureTiming("overlay.ready")
        snapshots = []
        showOverlayWindows(snapshots: [])
    }

    private func showOverlayWindows(snapshots: [DisplaySnapshot]) {
        let capturableWindows = CaptureWindowCatalog.currentWindows()
        let policy = selectionPolicy(for: purpose)
        var snapshotsByDisplayID: [CGDirectDisplayID: DisplaySnapshot] = [:]
        for snapshot in snapshots {
            snapshotsByDisplayID[snapshot.displayID] = snapshot
        }

        logDiagnostics(
            "overlay.createWindows",
            [
                "screenCount=\(NSScreen.screens.count)",
                "capturableWindowCount=\(capturableWindows.count)",
            ]
        )

        for screen in NSScreen.screens {
            let displayID = ScreenCoordinateSpace.displayID(for: screen)
            let snapshot = displayID.flatMap { snapshotsByDisplayID[$0] }
            let windowsForScreen = capturableWindows.filter { window in
                guard let frame = window.frame else { return false }
                return frame.intersects(screen.frame)
            }
            if snapshot == nil {
                logDiagnostics(
                    "overlay.missingSnapshot",
                    [
                        "displayID=\(displayID.map { String($0) } ?? "nil")",
                        "frame=\(Self.serialize(screen.frame))",
                    ]
                )
            }

            let window = CaptureOverlayWindow(screen: screen)
            window.setFrame(screen.frame, display: true)
            window.onEscape = { [weak self] in self?.cancelFromUser() }
            let overlayView = CaptureOverlayView(
                screen: screen,
                snapshot: snapshot,
                windows: windowsForScreen,
                policy: policy
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

        installKeyMonitor()
        installMouseMoveMonitors()
        installResignActiveObserver()
        orderOverlayWindowsFront(keyWindow: window(containing: NSEvent.mouseLocation) ?? windows.first)
        updateHoverForCurrentMouseLocation()
        logDiagnostics(
            "overlay.windowsReady",
            [
                "appActive=\(NSApp.isActive)",
                "activationPolicy=\(NSApp.activationPolicy().rawValue)",
                "separateSpaces=\(NSScreen.screensHaveSeparateSpaces)",
                "windows=\(Self.overlayWindowSummary(windows))",
            ]
        )
        overlayTiming?.finish(
            [
                "windowCount=\(windows.count)",
                "screenCount=\(NSScreen.screens.count)",
            ],
            enabled: settings.captureDiagnosticsEnabled
        )
        overlayTiming = nil
        NSCursor.crosshair.set()
    }

    private func selectionPolicy(for purpose: Purpose?) -> CaptureSelectionPolicy {
        switch purpose {
        case let .screenshot(policy):
            return policy
        case .recording, nil:
            return .any
        }
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
        for overlayView in overlayViews {
            overlayView.updateHoverFromGlobalMouseLocation(point)
        }
        // Cursor rects only apply to the key window, so keep the crosshair on every
        // display while the user is still choosing a selection.
        if activeScreenshotViewModel == nil,
           overlayViews.allSatisfy({ !$0.hasLockedSelection }),
           window(containing: point) != nil {
            NSCursor.crosshair.set()
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
        logDiagnostics("selection.lock", ["selection=\(Self.describe(selection))"])

        for overlayView in overlayViews {
            overlayView.lock(selection: overlayView === selectedView ? selection : nil)
        }
        NSCursor.arrow.set()

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
                captureSelectedScreenshot(selection: selection, in: selectedView)
                return
            }
            let document: ScreenshotDocument
            switch selection {
            case .area, .display:
                // Keep the whole display so the selection can be resized in the editor.
                document = try screenCaptureService.displayDocument(
                    fromSnapshot: snapshot,
                    selectionCocoaRect: selection.cocoaRect,
                    source: screenshotSource(for: selection)
                )
            case .window:
                document = try screenCaptureService.document(
                    fromSnapshot: snapshot,
                    cocoaRect: selection.cocoaRect,
                    source: screenshotSource(for: selection)
                )
            }
            let viewModel = installScreenshotEditor(document: document, selection: selection, in: selectedView)
            refreshWindowScreenshotIfNeeded(selection: selection, viewModel: viewModel)
        } catch {
            let message = error.localizedDescription
            let reportError = onError
            cancel()
            reportError?(message)
        }
    }

    private func captureSelectedScreenshot(selection: CaptureSelection, in selectedView: CaptureOverlayView) {
        let selectedWindow = selectedView.window
        logDiagnostics("capture.selected.begin", ["selection=\(Self.describe(selection))"])
        let timing = CaptureTiming("capture.selected.done")
        captureTask?.cancel()
        captureTask = Task { @MainActor [weak self, weak selectedView, weak selectedWindow] in
            guard let self, let selectedView else { return }
            do {
                // Only the display being captured needs its overlay hidden. The other
                // displays stay dimmed, so the overlay never flashes screen-wide.
                self.orderOverlayWindowsOut(intersecting: selection.cocoaRect)
                let options = CaptureOptions(
                    includeCursor: self.settings.screenshotIncludeCursor,
                    hideBelloBoxWindows: false,
                    delayAfterHidingOverlays: 0.05
                )
                let document: ScreenshotDocument
                switch selection {
                case let .area(area):
                    // Capture the whole display and keep the selection as the crop, so the
                    // editor can grow or move it without capturing again.
                    document = try await self.screenCaptureService.captureAreaWithinDisplay(area, options: options)
                case let .display(display):
                    document = try await self.screenCaptureService.capture(.display(display), options: options)
                case let .window(window):
                    document = try await self.screenCaptureService.capture(.window(window), options: options)
                }
                guard !Task.isCancelled else { return }
                self.orderOverlayWindowsFront(keyWindow: selectedWindow)
                _ = self.installScreenshotEditor(document: document, selection: selection, in: selectedView)
                timing.finish(
                    [
                        "selection=\(Self.describe(selection))",
                        "image=\(document.baseImage.width)x\(document.baseImage.height)",
                        "crop=\(document.cropRect.map { Self.serialize($0) } ?? "none")",
                    ],
                    enabled: self.settings.captureDiagnosticsEnabled
                )
                self.captureTask = nil
            } catch {
                guard !Task.isCancelled else { return }
                self.logDiagnostics("capture.selected.error", ["error=\(error.localizedDescription)"])
                let reportError = self.onError
                self.cancel()
                reportError?(error.localizedDescription)
            }
        }
    }

    @discardableResult
    private func installScreenshotEditor(
        document: ScreenshotDocument,
        selection: CaptureSelection,
        in selectedView: CaptureOverlayView
    ) -> ScreenshotPopupViewModel {
        let viewModel = ScreenshotPopupViewModel(
            document: document,
            settings: settings,
            macOCRService: macOCRService,
            allowsSelectionAdjustment: Self.selectionIsAdjustable(selection)
        )
        activeScreenshotViewModel = viewModel
        viewModel.onClose = { [weak self, weak viewModel] in
            if self?.activeScreenshotViewModel === viewModel {
                self?.activeScreenshotViewModel = nil
            }
            self?.cancelFromUser()
        }
        selectedView.showScreenshotEditor(viewModel: viewModel, selection: selection)
#if DEBUG
        writeE2EOverlayExportIfNeeded(viewModel: viewModel, selection: selection)
#endif
        logDiagnostics(
            "editor.inline",
            [
                "selection=\(Self.describe(selection))",
                "image=\(document.baseImage.width)x\(document.baseImage.height)",
            ]
        )
        return viewModel
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

    /// Area and screen captures keep the entire display image, so their selection can be
    /// resized or moved afterwards. Window captures are occlusion-free window images that
    /// cannot grow beyond the window.
    private static func selectionIsAdjustable(_ selection: CaptureSelection) -> Bool {
        switch selection {
        case .area, .display:
            return true
        case .window:
            return false
        }
    }

    /// Hides overlay windows. With `rect`, only windows whose display intersects the
    /// selection are hidden; the remaining displays keep their dim.
    private func orderOverlayWindowsOut(intersecting rect: CGRect? = nil) {
        for window in windows {
            if let rect, !rect.isEmpty, !window.frame.intersects(rect) { continue }
            window.orderOut(nil)
        }
    }

    private func orderOverlayWindowsFront(keyWindow: NSWindow?) {
        for window in windows {
            window.orderFrontRegardless()
        }
        keyWindow?.makeKeyAndOrderFront(nil)
    }

    private func window(containing point: CGPoint) -> CaptureOverlayWindow? {
        windows.first { $0.frame.contains(point) }
    }

    private func logDiagnostics(_ event: String, _ details: [String] = []) {
        CaptureDiagnostics.log(event, enabled: settings.captureDiagnosticsEnabled, details: details)
    }

    private static func screenSummary() -> String {
        NSScreen.screens.map { screen in
            let displayID = ScreenCoordinateSpace.displayID(for: screen).map { String($0) } ?? "nil"
            return "id=\(displayID),frame=\(serialize(screen.frame)),scale=\(ScreenCoordinateSpace.backingScale(for: screen))"
        }
        .joined(separator: ";")
    }

    private static func snapshotSummary(_ snapshots: [DisplaySnapshot]) -> String {
        snapshots.map { snapshot in
            "id=\(snapshot.displayID),frame=\(serialize(snapshot.screenFrame)),scale=\(snapshot.scale),image=\(snapshot.image.width)x\(snapshot.image.height)"
        }
        .joined(separator: ";")
    }

    private static func overlayWindowSummary(_ windows: [CaptureOverlayWindow]) -> String {
        windows.map { window in
            let displayID = window.screen.flatMap(ScreenCoordinateSpace.displayID(for:)).map(String.init) ?? "nil"
            return [
                "displayID=\(displayID)",
                "frame=\(serialize(window.frame))",
                "visible=\(window.isVisible)",
                "key=\(window.isKeyWindow)",
                "activeSpace=\(window.isOnActiveSpace)",
                "level=\(window.level.rawValue)",
                "style=\(window.styleMask.rawValue)",
                "collection=\(window.collectionBehavior.rawValue)",
            ].joined(separator: ",")
        }
        .joined(separator: ";")
    }

    private static func describe(_ selection: CaptureSelection) -> String {
        switch selection {
        case let .area(area):
            return "area(displayID=\(area.displayID.map { String($0) } ?? "nil"),rect=\(serialize(area.cocoaRect)))"
        case let .display(display):
            return "display(id=\(display.displayID),frame=\(serialize(display.frame)))"
        case let .window(window):
            return "window(id=\(window.windowID),owner=\(window.ownerName ?? ""),title=\(window.title ?? ""),mode=\(window.captureMode),layer=\(window.layer.map { String($0) } ?? "nil"),frame=\(window.frame.map { serialize($0) } ?? "nil"))"
        }
    }

    private static func serialize(_ point: CGPoint) -> String {
        "\(Int(point.x.rounded())),\(Int(point.y.rounded()))"
    }

    private static func serialize(_ rect: CGRect) -> String {
        "\(Int(rect.origin.x.rounded())),\(Int(rect.origin.y.rounded())),\(Int(rect.size.width.rounded())),\(Int(rect.size.height.rounded()))"
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

#if DEBUG
    private func writeE2EOverlayExportIfNeeded(viewModel: ScreenshotPopupViewModel, selection: CaptureSelection) {
        let env = ProcessInfo.processInfo.environment
        let outputPath = env["BELLOBOX_E2E_CAPTURE_OVERLAY_OUTPUT"]
        let markerPath = env["BELLOBOX_E2E_CAPTURE_OVERLAY_MARKER"]
        guard outputPath?.isEmpty == false || markerPath?.isEmpty == false else { return }

        do {
            let size = viewModel.visibleImageSize
            let rect = CGRect(
                x: max(4, size.width * 0.08),
                y: max(4, size.height * 0.08),
                width: max(20, min(96, size.width * 0.35)),
                height: max(18, min(64, size.height * 0.25))
            )
            viewModel.addVisibleAnnotation(.rectangle(rect))
            viewModel.beginTextAnnotation(atVisiblePoint: CGPoint(x: rect.minX + 6, y: min(size.height - 34, rect.maxY + 8)))
            viewModel.updateEditingText("E2E")
            viewModel.endTextEditing()

            let rendered = try AnnotationRenderer.render(viewModel.document)
            if let outputPath, !outputPath.isEmpty {
                try Self.writePNG(rendered, to: outputPath)
            }
            let visibleColorTag = Self.colorTag(for: viewModel.basePreviewImage())
            let annotationCount = viewModel.document.annotations.count
            Self.writeE2EMarker(
                markerPath,
                lines: [
                    "kind=capture-overlay-screenshot",
                    "status=success",
                    "selection=\(Self.serialize(selection.cocoaRect))",
                    "selectionDisplayID=\(Self.selectionDisplayID(selection).map { String($0) } ?? "nil")",
                    "documentSource=\(Self.describe(viewModel.document.source))",
                    "baseImageColorTag=\(visibleColorTag)",
                    "imageWidth=\(rendered.width)",
                    "imageHeight=\(rendered.height)",
                    "annotationCount=\(annotationCount)",
                    "fileSize=\(Self.fileSize(at: outputPath))",
                ] + e2eSelectionAdjustmentLines(viewModel: viewModel)
            )
        } catch {
            Self.writeE2EMarker(
                markerPath,
                lines: [
                    "kind=capture-overlay-screenshot",
                    "status=failure",
                    "error=\(error.localizedDescription)",
                ]
            )
        }
        e2eQuitIfRequested()
    }

    /// Exercises the post-capture resize path the same way the handles do: shrink the
    /// selection by 20 px on the right and bottom, render, then undo and render again.
    private func e2eSelectionAdjustmentLines(viewModel: ScreenshotPopupViewModel) -> [String] {
        guard viewModel.supportsSelectionAdjustment else { return ["resizeSupported=false"] }
        do {
            let crop = viewModel.selectionCropRect
            let shrunk = CGRect(
                x: crop.minX,
                y: crop.minY,
                width: max(1, crop.width - 20),
                height: max(1, crop.height - 20)
            )
            viewModel.beginSelectionAdjustment()
            viewModel.setSelectionCropRect(shrunk)
            viewModel.endSelectionAdjustment()
            let resized = try AnnotationRenderer.render(viewModel.document)
            viewModel.undo()
            let restored = try AnnotationRenderer.render(viewModel.document)
            return [
                "resizeSupported=true",
                "resizedImageWidth=\(resized.width)",
                "resizedImageHeight=\(resized.height)",
                "undoRestoredWidth=\(restored.width)",
                "undoRestoredHeight=\(restored.height)",
            ]
        } catch {
            return ["resizeSupported=true", "resizeError=\(error.localizedDescription)"]
        }
    }

    private func e2eQuitIfRequested() {
        let env = ProcessInfo.processInfo.environment
        guard env["BELLOBOX_E2E_QUIT_AFTER_HOOKS"] == "1" || env["BELLOBOX_E2E_QUIT_AFTER_E2E"] == "1" else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            NSApp.terminate(nil)
        }
    }

    private static func writePNG(_ image: CGImage, to path: String) throws {
        let url = URL(fileURLWithPath: path)
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try ImageExportService.pngData(from: image).write(to: url, options: .atomic)
    }

    private static func writeE2EMarker(_ path: String?, lines: [String]) {
        guard let path, !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        let url = URL(fileURLWithPath: path)
        let payload = (lines + ["timestamp=\(Date().timeIntervalSince1970)"]).joined(separator: "\n")
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? payload.write(to: url, atomically: true, encoding: .utf8)
    }

    private static func fileSize(at path: String?) -> Int {
        guard let path, !path.isEmpty,
              let size = try? FileManager.default.attributesOfItem(atPath: path)[.size] as? NSNumber
        else { return 0 }
        return size.intValue
    }

    private static func selectionDisplayID(_ selection: CaptureSelection) -> CGDirectDisplayID? {
        switch selection {
        case let .area(area):
            return area.displayID
        case let .display(display):
            return display.displayID
        case .window:
            return nil
        }
    }

    private static func describe(_ source: ScreenshotSource) -> String {
        switch source {
        case let .area(rect, displayID):
            return "area(displayID=\(displayID.map { String($0) } ?? "nil"),rect=\(serialize(rect)))"
        case let .display(displayID):
            return "display(id=\(displayID.map { String($0) } ?? "nil"))"
        case let .window(title, ownerName, windowID):
            return "window(id=\(windowID.map { String($0) } ?? "nil"),owner=\(ownerName ?? ""),title=\(title ?? ""))"
        case let .scrolling(target, frameCount):
            return "scrolling(title=\(target.title ?? ""),frames=\(frameCount))"
        case .importedClipboard:
            return "importedClipboard"
        }
    }

    private static func colorTag(for image: CGImage) -> String {
        let pixel = samplePixel(image)
        if pixel.green > 130, pixel.red < 100, pixel.blue < 120 {
            return "secondary-left"
        }
        if pixel.red > 130, pixel.green < 100, pixel.blue < 100 {
            return "primary"
        }
        if pixel.blue > 130, pixel.red < 100 {
            return "upper"
        }
        return "\(pixel.red),\(pixel.green),\(pixel.blue),\(pixel.alpha)"
    }

    private static func samplePixel(_ image: CGImage) -> (red: UInt8, green: UInt8, blue: UInt8, alpha: UInt8) {
        var data = [UInt8](repeating: 0, count: 4)
        guard image.width > 0, image.height > 0,
              let context = CGContext(
                data: &data,
                width: 1,
                height: 1,
                bitsPerComponent: 8,
                bytesPerRow: 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
              )
        else { return (0, 0, 0, 0) }
        let x = image.width / 2
        let y = image.height / 2
        context.translateBy(x: CGFloat(-x), y: CGFloat(y - image.height + 1))
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return (data[0], data[1], data[2], data[3])
    }

#endif
}

private final class CaptureOverlayWindow: NSPanel {
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

private final class CaptureOverlayView: NSView {
    let screen: NSScreen
    let snapshot: DisplaySnapshot?
    let windows: [CaptureWindow]
    let policy: CaptureSelectionPolicy

    var onSelection: ((CaptureSelection) -> Void)?
    var onCancel: (() -> Void)?

    private let dimmingView: CaptureDimmingView
    private var startPoint: CGPoint?
    private var currentPoint: CGPoint?
    private var hoveredWindow: CaptureWindow?
    private var lockedSelection: CaptureSelection?
    private var lockedLocalRect: CGRect?
    /// Once any display locks a selection, every overlay stops accepting new drags so a
    /// second editor can never be opened from another screen.
    private var interactionLocked = false
    private var accessoryView: NSView?
    private var trackingArea: NSTrackingArea?
    fileprivate var hasLockedSelection: Bool { lockedSelection != nil }
    fileprivate var dimBandFrames: [CGRect] { dimmingView.dimBandFrames }
    fileprivate var visibleSelectionFrame: CGRect? {
        dimmingView.selectionFrame.map { CaptureDimmingView.flip($0, height: bounds.height) }
    }

    init(screen: NSScreen, snapshot: DisplaySnapshot?, windows: [CaptureWindow], policy: CaptureSelectionPolicy) {
        self.screen = screen
        self.snapshot = snapshot
        self.windows = windows
        self.policy = policy
        let frame = CGRect(origin: .zero, size: screen.frame.size)
        dimmingView = CaptureDimmingView(frame: frame, contentsScale: ScreenCoordinateSpace.backingScale(for: screen))
        super.init(frame: frame)
        wantsLayer = true
        dimmingView.snapshotImage = snapshot?.image
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
        guard !interactionLocked else { return }
        addCursorRect(bounds, cursor: .crosshair)
    }

    override func cursorUpdate(with event: NSEvent) {
        if interactionLocked {
            super.cursorUpdate(with: event)
        } else {
            NSCursor.crosshair.set()
        }
    }

    /// Locks every overlay. The selected display shows its selection; the others keep a
    /// full dim and stop reacting to the mouse.
    func lock(selection: CaptureSelection?) {
        lockedSelection = selection
        lockedLocalRect = selection.map { localRect(for: $0.cocoaRect) }
        interactionLocked = true
        startPoint = nil
        currentPoint = nil
        hoveredWindow = nil
        window?.invalidateCursorRects(for: self)
        refreshChrome()
    }

    /// Called by the inline editor whenever the user resizes or moves the selection.
    func updateLockedSelection(localRect: CGRect) {
        guard interactionLocked, lockedSelection != nil else { return }
        lockedLocalRect = localRect.intersection(bounds).standardized
        refreshChrome()
    }

    func showScreenshotEditor(viewModel: ScreenshotPopupViewModel, selection: CaptureSelection) {
        installHostingView(
            CaptureScreenshotOverlaySurface(
                viewModel: viewModel,
                screenFrame: screen.frame,
                selectionFrame: selection.cocoaRect,
                onSelectionFrameChanged: { [weak self] localRect in
                    self?.updateLockedSelection(localRect: localRect)
                },
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
        guard !interactionLocked else { return }
        updateHover(at: localPoint(for: event))
    }

    override func mouseExited(with event: NSEvent) {
        guard !interactionLocked else { return }
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
        guard !interactionLocked else { return }
        let point = localPoint(for: event)
        updateHover(at: point)
        startPoint = point
        currentPoint = point
        refreshChrome()
    }

    override func mouseDragged(with event: NSEvent) {
        guard !interactionLocked else { return }
        currentPoint = localPoint(for: event)
        refreshChrome()
    }

    override func mouseUp(with event: NSEvent) {
        guard !interactionLocked else { return }
        currentPoint = localPoint(for: event)
        guard let displayID = ScreenCoordinateSpace.displayID(for: screen),
              let selection = CaptureSelectionResolver.resolve(
                startLocal: startPoint,
                endLocal: currentPoint ?? localPoint(for: event),
                hoveredWindow: hoveredWindow,
                screenFrame: screen.frame,
                displayID: displayID,
                policy: policy
              )
        else {
            resetInteraction(at: currentPoint ?? localPoint(for: event))
            return
        }
        onSelection?(selection)
    }

    private var activeRect: CGRect? {
        if interactionLocked {
            return lockedLocalRect
        }
        switch policy {
        case .displayOnly:
            return nil
        case .windowOnly:
            guard let frame = hoveredWindow?.frame else { return nil }
            return RegionCaptureGeometry.globalCocoaRectToLocalFlipped(frame, screenFrame: screen.frame).intersection(bounds)
        case .areaOnly:
            return visibleDragRect
        case .areaOrWindow, .any:
            if let selection = visibleDragRect {
                return selection
            }
            guard let frame = hoveredWindow?.frame else { return selectionRect }
            return RegionCaptureGeometry.globalCocoaRectToLocalFlipped(frame, screenFrame: screen.frame).intersection(bounds)
        }
    }

    private var selectionRect: CGRect? {
        guard let startPoint, let currentPoint else { return nil }
        return RegionCaptureGeometry.clampedSelectionRect(from: startPoint, to: currentPoint, bounds: bounds)
    }

    private var visibleDragRect: CGRect? {
        guard let selection = selectionRect,
              selection.width >= RegionCaptureGeometry.dragThreshold || selection.height >= RegionCaptureGeometry.dragThreshold
        else { return nil }
        return selection
    }

    /// Pushes the current selection into the layer-backed chrome. Cheap enough to call on
    /// every mouse event; nothing is rasterised.
    private func refreshChrome() {
        let selection = activeRect.flatMap { $0.isNull || $0.isEmpty ? nil : $0 }
        var label: String?
        if let selection, !interactionLocked {
            let scale = snapshot?.scale ?? ScreenCoordinateSpace.backingScale(for: screen)
            label = CaptureDimmingView.sizeLabel(for: selection, scale: scale)
        }
        dimmingView.update(
            selection: selection.map { CaptureDimmingView.flip($0, height: bounds.height) },
            borderWidth: interactionLocked ? 2.5 : 2,
            label: label
        )
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
        guard policy == .any || policy == .windowOnly || policy == .areaOrWindow else {
            if hoveredWindow != nil {
                hoveredWindow = nil
                refreshChrome()
            }
            return
        }
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
        guard !interactionLocked else { return }
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

private struct SelectionAdjustmentState {
    var startRect: CGRect
    /// nil while the whole selection is being moved.
    var handle: SelectionHandle?
}

private struct CaptureScreenshotOverlaySurface: View {
    @ObservedObject var viewModel: ScreenshotPopupViewModel
    var screenFrame: CGRect
    var selectionFrame: CGRect
    /// Reports the selection frame (flipped local points) whenever it changes, so the
    /// dim cut-out behind the editor follows resizes, moves, crops and undo.
    var onSelectionFrameChanged: (CGRect) -> Void
    var onCancel: () -> Void
    @State private var adjustment: SelectionAdjustmentState?

    var body: some View {
        GeometryReader { geometry in
            let bounds = CGRect(origin: .zero, size: geometry.size)
            let selected = currentSelectionFrame(in: bounds)
            let toolbar = CaptureOverlayAccessoryLayout.frame(
                selection: selected,
                bounds: bounds,
                preferredSize: CGSize(width: 900, height: 54)
            )

            ZStack(alignment: .topLeading) {
                Color.clear

                AnnotationCanvasView(viewModel: viewModel, selectionMoveHandler: moveHandler(bounds: bounds))
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

                // Handles sit above the toolbar: when the toolbar has to be clamped inside a
                // tall selection it must not cover the bottom handles.
                if viewModel.supportsSelectionAdjustment {
                    ForEach(SelectionHandle.allCases, id: \.self) { handle in
                        SelectionHandleView(handle: handle, isActive: adjustment?.handle == handle)
                            .position(SelectionResizeGeometry.handlePosition(handle, in: selected))
                            .gesture(handleDragGesture(handle, bounds: bounds))
                    }
                }
            }
            .onExitCommand(perform: onCancel)
            .onAppear { onSelectionFrameChanged(selected) }
            .onChange(of: viewModel.document.cropRect) { _ in
                onSelectionFrameChanged(currentSelectionFrame(in: bounds))
            }
        }
    }

    /// Adjustable documents keep the whole display as their base image, so the on-screen
    /// frame is simply the crop rect converted from pixels to points. Window captures
    /// keep the fixed frame they were selected with.
    private func currentSelectionFrame(in bounds: CGRect) -> CGRect {
        if viewModel.supportsSelectionAdjustment {
            let scale = pixelsPerPoint(in: bounds)
            let crop = viewModel.selectionCropRect
            let local = CGRect(
                x: crop.minX / scale.width,
                y: crop.minY / scale.height,
                width: crop.width / scale.width,
                height: crop.height / scale.height
            )
            let clipped = local.intersection(bounds).standardized
            return clipped.isNull ? local.standardized : clipped
        }
        return RegionCaptureGeometry
            .globalCocoaRectToLocalFlipped(selectionFrame, screenFrame: screenFrame)
            .intersection(bounds)
            .standardized
    }

    private func moveHandler(bounds: CGRect) -> SelectionMoveGestureHandler? {
        guard viewModel.supportsSelectionAdjustment else { return nil }
        return SelectionMoveGestureHandler(
            onBegin: {
                adjustment = SelectionAdjustmentState(startRect: currentSelectionFrame(in: bounds), handle: nil)
                viewModel.beginSelectionAdjustment()
            },
            onChange: { translation in
                guard let adjustment, adjustment.handle == nil else { return }
                applySelection(
                    localRect: SelectionResizeGeometry.movedRect(
                        from: adjustment.startRect,
                        translation: translation,
                        bounds: bounds
                    )
                )
            },
            onEnd: {
                viewModel.endSelectionAdjustment()
                adjustment = nil
            }
        )
    }

    private func handleDragGesture(_ handle: SelectionHandle, bounds: CGRect) -> some Gesture {
        // Global coordinates keep `translation` stable while the handle itself moves.
        DragGesture(minimumDistance: 0, coordinateSpace: .global)
            .onChanged { value in
                if adjustment == nil {
                    adjustment = SelectionAdjustmentState(startRect: currentSelectionFrame(in: bounds), handle: handle)
                    viewModel.beginSelectionAdjustment()
                }
                guard let adjustment, adjustment.handle == handle else { return }
                let rect = SelectionResizeGeometry.resizedRect(
                    from: adjustment.startRect,
                    handle: handle,
                    translation: value.translation,
                    bounds: bounds,
                    minimumSize: RegionCaptureGeometry.minimumAreaSize
                )
                applySelection(localRect: rect)
            }
            .onEnded { _ in
                viewModel.endSelectionAdjustment()
                adjustment = nil
            }
    }

    private func applySelection(localRect: CGRect) {
        let bounds = CGRect(origin: .zero, size: CGSize(width: screenFrame.width, height: screenFrame.height))
        let scale = pixelsPerPoint(in: bounds)
        viewModel.setSelectionCropRect(
            CGRect(
                x: localRect.minX * scale.width,
                y: localRect.minY * scale.height,
                width: localRect.width * scale.width,
                height: localRect.height * scale.height
            )
        )
    }

    /// The base image of an adjustable document covers the whole display, so map each
    /// axis independently (the same way the crop was computed) rather than assuming one
    /// uniform scale.
    private func pixelsPerPoint(in bounds: CGRect) -> CGSize {
        let imageSize = viewModel.document.imageSize
        let fallback = max(viewModel.document.scale, 0.01)
        return CGSize(
            width: bounds.width > 0 && imageSize.width > 0 ? imageSize.width / bounds.width : fallback,
            height: bounds.height > 0 && imageSize.height > 0 ? imageSize.height / bounds.height : fallback
        )
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

private struct SelectionHandleView: View {
    var handle: SelectionHandle
    var isActive: Bool

    var body: some View {
        let diameter: CGFloat = isActive ? 14 : 11
        Circle()
            .fill(Color.white)
            .overlay(Circle().strokeBorder(BoxTheme.accent, lineWidth: isActive ? 2.5 : 1.5))
            .shadow(color: .black.opacity(0.35), radius: 2, y: 1)
            .frame(width: diameter, height: diameter)
            .frame(width: 22, height: 22)
            .contentShape(Rectangle())
            .onHover { inside in
                // set() rather than push()/pop(): the hosting view can be torn down while
                // the pointer rests on a handle, which would leave a pushed cursor behind.
                (inside ? Self.cursor(for: handle) : NSCursor.arrow).set()
            }
    }

    private static func cursor(for handle: SelectionHandle) -> NSCursor {
        if handle.isCorner { return .crosshair }
        return handle.movesLeftEdge || handle.movesRightEdge ? .resizeLeftRight : .resizeUpDown
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
