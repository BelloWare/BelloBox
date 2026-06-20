import AppKit
import SwiftUI

/// Coordinates the whole selection → toolbar → popup flow: listens for
/// selections, shows the floating tool toolbar, and presents the AI or QR popup.
@MainActor
final class SelectionOverlayController: NSObject {
    private let settings: AppSettings
    private let accessibility = AccessibilityService()
    private let client = AIClient()
    private let monitor: SelectionMonitor
    private let screenCaptureService = ScreenCaptureService()
    private let macOCRService = MacVisionOCRService()
    private lazy var llmOCRService = LLMOCRService(settings: settings)
    private lazy var recordingCoordinator = RecordingCoordinator(settings: settings)

    private var toolbarPanel: FloatingButtonPanel?
    private var popupPanel: PopupPanel?
    private var popupFullContentView: NSView?
    private var popupFullSize: CGSize = .zero
    private var popupIsMinimized = false
    private var popupMinimizedIcon = ""
    private var popupMinimizedTitle = ""
    private var popupMinimizedSubtitle: (() -> String?)?
    private var inlineScreenshotPanel: InlineScreenshotEditorPanel?
    private var toolbarDismissMonitor: Any?

    private var pendingSelection: TextSelection?
    private var trustWatcher: Timer?
    private var lastTrusted = false
    private var regionCaptureController: RegionCaptureOverlayController?
    private var scrollingCaptureCoordinator: ScrollCaptureCoordinator?

    /// Set by the app to open the Settings window.
    var openSettings: () -> Void = {}

    init(settings: AppSettings) {
        self.settings = settings
        self.monitor = SelectionMonitor(accessibility: accessibility)
        super.init()
        monitor.onSelection = { [weak self] selection in
            self?.handleSelection(selection)
        }
        monitor.onHotkey = { [weak self] in
            self?.triggerBoardOnCurrentSelection()
        }
        monitor.onScreenshotHotkey = { [weak self] in
            self?.triggerScreenshotShortcut()
        }
        monitor.onRecordingHotkey = { [weak self] in
            self?.triggerRecording()
        }
        screenCaptureService.beforeCapture = { [weak self] in
            self?.toolbarPanel?.orderOut(nil)
            self?.popupPanel?.orderOut(nil)
            self?.inlineScreenshotPanel?.orderOut(nil)
        }
        screenCaptureService.afterCapture = { [weak self] in
            self?.popupPanel?.orderFrontRegardless()
            self?.inlineScreenshotPanel?.orderFrontRegardless()
        }
        recordingCoordinator.onStateChange = { [weak self] state in
            self?.handleRecordingState(state)
        }
    }

    func start() {
        lastTrusted = AccessibilityService.isTrusted
        applyMonitorSettings()
        monitor.start()
        // Keyboard monitoring only takes effect once the process is trusted, so
        // re-establish the monitors when Accessibility is granted while running.
        if !lastTrusted { startTrustWatcher() }
#if DEBUG
        runScreenshotE2EHooksIfNeeded()
#endif
    }

    /// Tears down and re-installs the event monitors. Needed after Accessibility
    /// is granted so the global keyboard monitor actually receives events.
    func restartMonitors() {
        monitor.stop()
        applyMonitorSettings()
        monitor.start()
    }

    func setFloatingButtonEnabled(_ enabled: Bool) {
        monitor.selectionMonitoringEnabled = enabled
        if !enabled { hideToolbar() }
    }

    func setGlobalHotkeyEnabled(_ enabled: Bool) {
        monitor.hotkeyEnabled = enabled
    }

    func setGlobalHotkey(_ hotkey: GlobalHotkey) {
        monitor.hotkey = hotkey
    }

    func setScreenshotHotkeyEnabled(_ enabled: Bool) {
        monitor.screenshotHotkeyEnabled = enabled
    }

    func setScreenshotHotkey(_ hotkey: GlobalHotkey) {
        monitor.screenshotHotkey = hotkey
    }

    func setRecordingHotkeyEnabled(_ enabled: Bool) {
        monitor.recordingHotkeyEnabled = enabled
    }

    func setRecordingHotkey(_ hotkey: GlobalHotkey) {
        monitor.recordingHotkey = hotkey
    }

    var isRecording: Bool { recordingCoordinator.isRecording }

    private func applyMonitorSettings() {
        monitor.selectionMonitoringEnabled = settings.floatingButtonEnabled
        monitor.hotkeyEnabled = settings.globalHotkeyEnabled
        monitor.hotkey = settings.globalHotkey
        monitor.screenshotHotkeyEnabled = settings.screenshotHotkeyEnabled
        monitor.screenshotHotkey = settings.screenshotHotkey
        monitor.recordingHotkeyEnabled = settings.recordingHotkeyEnabled
        monitor.recordingHotkey = settings.recordingHotkey
    }

    private func startTrustWatcher() {
        trustWatcher?.invalidate()
        trustWatcher = Timer.scheduledTimer(
            timeInterval: 2,
            target: self,
            selector: #selector(checkTrust),
            userInfo: nil,
            repeats: true
        )
    }

    @objc private func checkTrust() {
        guard !lastTrusted else { return }
        if AccessibilityService.isTrusted {
            lastTrusted = true
            restartMonitors()
            trustWatcher?.invalidate()
            trustWatcher = nil
        }
    }

    // MARK: - Selection handling

    private func handleSelection(_ selection: TextSelection) {
        guard settings.floatingButtonEnabled else { return }
        guard popupPanel == nil else { return } // don't interrupt an open popup
        pendingSelection = selection
        showToolbar(for: selection)
    }

    /// Reads the current selection (AX first, synthesized copy as a fallback).
    private func currentSelection() -> TextSelection? {
#if DEBUG
        if let injected = e2eInjectedSelection() { return injected }
#endif
        if let selection = accessibility.readSelection() { return selection }
        if let copied = accessibility.copySelectionViaPasteboard() {
            let front = NSWorkspace.shared.frontmostApplication
            return TextSelection(
                text: copied,
                anchorRect: nil,
                appName: front?.localizedName,
                bundleID: front?.bundleIdentifier,
                pid: front?.processIdentifier
            )
        }
        return nil
    }

#if DEBUG
    private func e2eInjectedSelection() -> TextSelection? {
        guard
            let text = ProcessInfo.processInfo.environment["BELLOBOX_E2E_SELECTION_TEXT"],
            !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return nil }

        return TextSelection(
            text: text,
            anchorRect: nil,
            appName: "E2E",
            bundleID: nil,
            pid: nil
        )
    }
#endif

    private func nonEmpty(_ selection: TextSelection?) -> TextSelection? {
        guard let selection, !selection.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
        return selection
    }

    /// Used by the hotkey / menu: read the selection now and open the AI popup.
    func triggerOnCurrentSelection() {
        guard let selection = nonEmpty(currentSelection()) else { NSSound.beep(); return }
        hideToolbar()
        showAIPopup(for: selection)
    }

    /// Used by the global hotkey: read the selection now and show the tool board.
    func triggerBoardOnCurrentSelection() {
        guard popupPanel == nil else { return }
        guard let selection = nonEmpty(currentSelection()) else { NSSound.beep(); return }
        pendingSelection = selection
        showToolbar(for: selection)
    }

    /// Used by the menu: read the selection now and open the QR popup.
    func triggerQROnCurrentSelection() {
        guard let selection = nonEmpty(currentSelection()) else { NSSound.beep(); return }
        hideToolbar()
        showQRPopup(for: selection)
    }

    /// Used by the menu: read the selection now and open the text-tools popup.
    func triggerTextToolsOnCurrentSelection() {
        guard let selection = nonEmpty(currentSelection()) else { NSSound.beep(); return }
        hideToolbar()
        showTextToolsPopup(for: selection)
    }

    // MARK: - Floating toolbar

    private func showToolbar(for selection: TextSelection) {
        hideToolbar()

        let view = FloatingToolbarView(
            onAI: { [weak self] in self?.activateAI() },
            onScreenshot: { [weak self] in self?.activateScreenshot() },
            onRecord: { [weak self] in self?.activateRecording() },
            onQR: { [weak self] in self?.activateQR() },
            onTools: { [weak self] in self?.activateTools() }
        )
        let hosting = NSHostingView(rootView: view)
        var size = hosting.fittingSize
        if size.width < 1 || size.height < 1 { size = FloatingToolbarView.preferredSize }

        let origin = ScreenPlacement.buttonOrigin(
            anchorRect: selection.anchorRect,
            mouse: NSEvent.mouseLocation,
            size: size
        )
        let panel = FloatingButtonPanel(contentRect: CGRect(origin: origin, size: size))
        panel.contentView = hosting
        panel.setFrameOrigin(origin)
        panel.orderFrontRegardless()
        toolbarPanel = panel
#if DEBUG
        writeE2EToolbarMarker(selection: selection)
#endif

        installToolbarDismissMonitor()
    }

#if DEBUG
    private func writeE2EToolbarMarker(selection: TextSelection) {
        guard
            let path = ProcessInfo.processInfo.environment["BELLOBOX_E2E_TOOLBAR_MARKER"],
            !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return }

        let url = URL(fileURLWithPath: path)
        let payload = [
            "shownAt=\(Date().timeIntervalSince1970)",
            "appName=\(selection.appName ?? "")",
            "text=\(selection.text)"
        ].joined(separator: "\n")
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? payload.write(to: url, atomically: true, encoding: .utf8)
    }
#endif

    private func activateAI() {
        guard let selection = pendingSelection else { return }
        hideToolbar()
        showAIPopup(for: selection)
    }

    private func activateScreenshot() {
        let anchor = pendingSelection?.anchorRect
        hideToolbar()
        showScreenshotChooser(anchorRect: anchor)
    }

    private func activateRecording() {
        let anchor = pendingSelection?.anchorRect
        hideToolbar()
        showRecordingChooser(anchorRect: anchor)
    }

    private func activateQR() {
        guard let selection = pendingSelection else { return }
        hideToolbar()
        showQRPopup(for: selection)
    }

    private func activateTools() {
        guard let selection = pendingSelection else { return }
        hideToolbar()
        showTextToolsPopup(for: selection)
    }

    private func hideToolbar() {
        if let monitor = toolbarDismissMonitor {
            NSEvent.removeMonitor(monitor)
            toolbarDismissMonitor = nil
        }
        toolbarPanel?.orderOut(nil)
        toolbarPanel = nil
    }

    private func installToolbarDismissMonitor() {
        toolbarDismissMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            self?.hideToolbar()
        }
    }

    // MARK: - Popups

    private func showAIPopup(for selection: TextSelection) {
        let viewModel = ActionPopupViewModel(
            selection: selection,
            settings: settings,
            client: client,
            accessibility: accessibility
        )
        viewModel.onClose = { [weak self] in self?.hidePopup() }
        viewModel.onOpenSettings = { [weak self] in
            self?.hidePopup()
            self?.openSettings()
        }
        let view = ActionPopupView(
            viewModel: viewModel,
            settings: settings,
            onMinimize: { [weak self] in self?.minimizePopup() }
        )
        present(
            view,
            size: ActionPopupView.preferredSize,
            anchorRect: selection.anchorRect,
            minimizedIcon: "wand.and.stars",
            minimizedTitle: "Bello Box",
            minimizedSubtitle: { viewModel.providerSummary }
        )
    }

    private func showQRPopup(for selection: TextSelection) {
        let viewModel = QRCodePopupViewModel(text: selection.text)
        viewModel.onClose = { [weak self] in self?.hidePopup() }
        let view = QRCodePopupView(
            viewModel: viewModel,
            onMinimize: { [weak self] in self?.minimizePopup() }
        )
        present(
            view,
            size: QRCodePopupView.preferredSize,
            anchorRect: selection.anchorRect,
            minimizedIcon: "qrcode",
            minimizedTitle: "QR Code"
        )
    }

    private func showTextToolsPopup(for selection: TextSelection) {
        let viewModel = TextToolsPopupViewModel(selection: selection, settings: settings, accessibility: accessibility)
        viewModel.onClose = { [weak self] in self?.hidePopup() }
        let view = TextToolsPopupView(
            viewModel: viewModel,
            settings: settings,
            onMinimize: { [weak self] in self?.minimizePopup() }
        )
        present(
            view,
            size: TextToolsPopupView.preferredSize,
            anchorRect: selection.anchorRect,
            minimizedIcon: "wrench.and.screwdriver",
            minimizedTitle: "Text Tools"
        )
    }

    // MARK: - Recording

    func triggerRecording(mode: RecordingCaptureMode? = nil) {
        hideToolbar()
        showRecordingChooser(anchorRect: nil, initialMode: mode)
    }

    func stopRecording() {
        recordingCoordinator.stop()
        hidePopup()
    }

    private func showRecordingChooser(anchorRect: CGRect?, initialMode: RecordingCaptureMode? = nil) {
        recordingCoordinator.showRecordingChooser(anchor: anchorRect)
        let view = RecordingCaptureChooserView(
            settings: settings,
            initialMode: initialMode,
            onArea: { [weak self] options in self?.prepareRecording(options: options, anchorRect: anchorRect) { self?.beginRecordingArea(options: $0, anchorRect: anchorRect) } },
            onWindow: { [weak self] options in self?.prepareRecording(options: options, anchorRect: anchorRect) { self?.showRecordingWindowPicker(options: $0, anchorRect: anchorRect) } },
            onDisplay: { [weak self] options in self?.prepareRecording(options: options, anchorRect: anchorRect) { self?.startDisplayRecording(options: $0, anchorRect: anchorRect) } },
            onCancel: { [weak self] in
                self?.recordingCoordinator.cancel()
                self?.hidePopup()
            }
        )
        present(
            view,
            size: RecordingCaptureChooserView.preferredSize,
            anchorRect: anchorRect,
            minimizedIcon: "record.circle",
            minimizedTitle: "Record"
        )
    }

    private func prepareRecording(options: RecordingOptions, anchorRect: CGRect?, start: @escaping (RecordingOptions) -> Void) {
        let permissions = recordingCoordinator.permissionState(options: options)
        if shouldShowRecordingPermissions(permissions, options: options) {
            showRecordingPermissions(permissions: permissions, options: options, anchorRect: anchorRect, start: start)
            return
        }
        start(options)
    }

    private func shouldShowRecordingPermissions(_ permissions: RecordingPermissionState, options: RecordingOptions) -> Bool {
        if !permissions.canRecordVideo { return true }
        if options.audioSource.includesMicrophone, permissions.microphone != .granted { return true }
        if (options.clickOverlayMode.isEnabled || options.keystrokeMode != .off), permissions.inputMonitoring != .granted { return true }
        if options.keystrokeMode == .allKeys, permissions.accessibility != .granted { return true }
        return false
    }

    private func showRecordingPermissions(
        permissions: RecordingPermissionState,
        options: RecordingOptions,
        anchorRect: CGRect?,
        start: @escaping (RecordingOptions) -> Void
    ) {
        let view = RecordingPermissionView(
            permissions: permissions,
            onRequestScreenRecording: {
                _ = ScreenCapturePermission.requestPrompt()
                ScreenCapturePermission.openSettings()
            },
            onRequestMicrophone: {
                Task { _ = await MicrophonePermission.request() }
            },
            onRequestInputMonitoring: {
                _ = InputMonitoringPermission.request()
            },
            onOpenAccessibility: {
                AccessibilityService.requestPermissionPrompt()
                AccessibilityService.openAccessibilitySettings()
            },
            onContinueWithoutOptional: { [weak self] in
                guard let self else { return }
                let sanitized = self.recordingOptionsByRemovingUnavailableOptionalFeatures(options, permissions: RecordingPermissionState.current(options: options))
                self.hidePopup()
                self.prepareRecording(options: sanitized, anchorRect: anchorRect, start: start)
            },
            onCancel: { [weak self] in
                self?.recordingCoordinator.cancel()
                self?.hidePopup()
            }
        )
        present(
            view,
            size: CGSize(width: 520, height: 420),
            anchorRect: anchorRect,
            minimizedIcon: "record.circle",
            minimizedTitle: "Recording Permissions"
        )
    }

    private func recordingOptionsByRemovingUnavailableOptionalFeatures(
        _ options: RecordingOptions,
        permissions: RecordingPermissionState
    ) -> RecordingOptions {
        var sanitized = options
        if permissions.microphone != .granted {
            switch sanitized.audioSource {
            case .microphone:
                sanitized.audioSource = .none
            case .microphoneAndSystemAudio:
                sanitized.audioSource = .systemAudio
            case .none, .systemAudio:
                break
            }
        }
        if permissions.inputMonitoring != .granted {
            sanitized.clickOverlayMode = .off
            sanitized.keystrokeMode = .off
        }
        if permissions.accessibility != .granted, sanitized.keystrokeMode == .allKeys {
            sanitized.keystrokeMode = .shortcutsOnly
        }
        return sanitized
    }

    private func beginRecordingArea(options: RecordingOptions, anchorRect: CGRect?) {
#if DEBUG
        if let area = e2eRegionArea(), let target = recordingTarget(for: area) {
            Task { await recordingCoordinator.start(target: target, options: options) }
            return
        }
#endif
        hidePopup()
        let controller = RegionCaptureOverlayController()
        regionCaptureController = controller
        controller.begin { [weak self] result in
            guard let self else { return }
            self.regionCaptureController = nil
            switch result {
            case let .success(capture):
                switch capture {
                case let .area(area):
                    guard let target = self.recordingTarget(for: area) else {
                        self.showRecordingError("No display could be found for this recording area.", anchorRect: anchorRect)
                        return
                    }
                    Task { await self.recordingCoordinator.start(target: target, options: options) }
                case let .window(window):
                    Task { await self.recordingCoordinator.start(target: self.recordingTarget(for: window), options: options) }
                }
            case let .failure(error):
                self.showRecordingError(error.localizedDescription, anchorRect: anchorRect)
            }
        }
    }

    private func recordingTarget(for area: CaptureArea) -> RecordingTarget? {
        guard let screen = area.displayID.flatMap(screen(for:)) ?? ScreenCoordinateSpace.displayForCocoaRect(area.cocoaRect),
              let displayID = ScreenCoordinateSpace.displayID(for: screen)
        else { return nil }
        return .area(displayID: displayID, rectInScreenPoints: area.cocoaRect)
    }

    private func recordingTarget(for window: CaptureWindow) -> RecordingTarget {
        .window(
            windowID: CGWindowID(window.windowID),
            displayID: window.frame.flatMap { ScreenCoordinateSpace.displayForCocoaRect($0).flatMap(ScreenCoordinateSpace.displayID(for:)) },
            frameInScreenPoints: window.frame
        )
    }

    private func showRecordingWindowPicker(options: RecordingOptions, anchorRect: CGRect?) {
        let viewModel = WindowCapturePickerViewModel(service: screenCaptureService)
        viewModel.onCancel = { [weak self] in self?.hidePopup() }
        viewModel.onSelect = { [weak self] window in
            self?.hidePopup()
            guard let self else { return }
            Task { await self.recordingCoordinator.start(target: self.recordingTarget(for: window), options: options) }
        }
        let view = WindowCapturePickerView(viewModel: viewModel)
        present(
            view,
            size: WindowCapturePickerView.preferredSize,
            anchorRect: anchorRect,
            minimizedIcon: "record.circle",
            minimizedTitle: "Record Window"
        )
    }

    private func startDisplayRecording(options: RecordingOptions, anchorRect: CGRect?) {
        hidePopup()
        let screen = ScreenCoordinateSpace.screenContainingMouse()
        guard let displayID = ScreenCoordinateSpace.displayID(for: screen) else {
            showRecordingError("No display could be found for this recording.", anchorRect: anchorRect)
            return
        }
        Task { await recordingCoordinator.start(target: .display(displayID: displayID), options: options) }
    }

    private func handleRecordingState(_ state: RecordingState) {
        switch state {
        case let .countingDown(seconds):
            let view = RecordingCountdownView(secondsRemaining: seconds) { [weak self] in
                self?.recordingCoordinator.cancel()
                self?.hidePopup()
            }
            present(
                view,
                size: CGSize(width: 320, height: 240),
                anchorRect: nil,
                minimizedIcon: "record.circle",
                minimizedTitle: "Recording"
            )
        case let .recording(runtime):
            let view = RecordingHUDView(
                runtime: runtime,
                isPaused: false,
                onPauseResume: { [weak self] in self?.recordingCoordinator.pause() },
                onStop: { [weak self] in self?.recordingCoordinator.stop() }
            )
            present(
                view,
                size: CGSize(width: 520, height: 80),
                anchorRect: nil,
                minimizedIcon: "record.circle",
                minimizedTitle: "Recording"
            )
        case let .paused(runtime):
            let view = RecordingHUDView(
                runtime: runtime,
                isPaused: true,
                onPauseResume: { [weak self] in self?.recordingCoordinator.resume() },
                onStop: { [weak self] in self?.recordingCoordinator.stop() }
            )
            present(
                view,
                size: CGSize(width: 520, height: 80),
                anchorRect: nil,
                minimizedIcon: "record.circle",
                minimizedTitle: "Recording"
            )
        case let .reviewing(url):
            let viewModel = RecordingReviewViewModel(fileURL: url)
            viewModel.onClose = { [weak self] in self?.hidePopup() }
            let view = RecordingReviewView(viewModel: viewModel)
            present(
                view,
                size: CGSize(width: 760, height: 430),
                anchorRect: nil,
                minimizedIcon: "play.rectangle",
                minimizedTitle: "Recording"
            )
        case let .failed(message):
            showRecordingError(message, anchorRect: nil)
        case .idle, .requestingPermissions, .choosingTarget, .finishing:
            break
        }
    }

    private func showRecordingError(_ message: String, anchorRect: CGRect?) {
        let view = RecordingErrorView(message: message) { [weak self] in
            self?.recordingCoordinator.cancel()
            self?.hidePopup()
        }
        present(
            view,
            size: CGSize(width: 420, height: 220),
            anchorRect: anchorRect,
            minimizedIcon: "record.circle",
            minimizedTitle: "Recording"
        )
    }

    private func screen(for displayID: CGDirectDisplayID) -> NSScreen? {
        NSScreen.screens.first { ScreenCoordinateSpace.displayID(for: $0) == displayID }
    }

    // MARK: - Screenshots

    func triggerScreenshotCapture() {
        hideToolbar()
        showScreenshotChooser(anchorRect: nil)
    }

    func triggerScrollingScreenshotCapture() {
        hideToolbar()
        showScreenshotChooser(anchorRect: nil, initialMode: .scrolling)
    }

    func triggerScreenshotShortcut() {
        hideToolbar()
        guard ScreenCapturePermission.isTrusted else {
            showScreenshotChooser(anchorRect: nil, initialMode: screenshotCaptureMode(from: settings.screenshotDefaultMode))
            return
        }
        switch settings.screenshotDefaultMode {
        case .area:
            beginAreaCapture(anchorRect: nil)
        case .window:
            showWindowPicker(anchorRect: nil)
        case .screen:
            captureScreen(anchorRect: nil)
        case .scrolling:
            beginScrollingAreaCapture(anchorRect: nil)
        }
    }

    private func showScreenshotChooser(anchorRect: CGRect?, initialMode: ScreenshotCaptureMode? = nil) {
        let viewModel = ScreenshotCaptureChooserViewModel()
        viewModel.onClose = { [weak self] in self?.hidePopup() }
        viewModel.onCaptureArea = { [weak self] in self?.beginAreaCapture(anchorRect: anchorRect) }
        viewModel.onCaptureWindow = { [weak self] in self?.showWindowPicker(anchorRect: anchorRect) }
        viewModel.onCaptureScreen = { [weak self] in self?.captureScreen(anchorRect: anchorRect) }
        viewModel.onCaptureScrolling = { [weak self] in self?.beginScrollingAreaCapture(anchorRect: anchorRect) }
        let view = ScreenshotCaptureChooserView(viewModel: viewModel, initialMode: initialMode)
        present(
            view,
            size: ScreenshotCaptureChooserView.preferredSize,
            anchorRect: anchorRect,
            minimizedIcon: "camera.viewfinder",
            minimizedTitle: "Screenshot"
        )
    }

    private func screenshotCaptureMode(from mode: ScreenshotDefaultMode) -> ScreenshotCaptureMode {
        switch mode {
        case .area: return .area
        case .window: return .window
        case .screen: return .screen
        case .scrolling: return .scrolling
        }
    }

    private func beginAreaCapture(anchorRect: CGRect?) {
#if DEBUG
        if let area = e2eRegionArea() {
            Task { await self.captureArea(area, anchorRect: anchorRect) }
            return
        }
#endif
        hidePopup()
        let controller = RegionCaptureOverlayController()
        regionCaptureController = controller
        controller.begin { [weak self] result in
            guard let self else { return }
            self.regionCaptureController = nil
            switch result {
            case let .success(capture):
                switch capture {
                case let .area(area):
                    Task { await self.captureArea(area, anchorRect: anchorRect) }
                case let .window(window):
                    Task { await self.captureWindow(window, anchorRect: anchorRect, preferInlineFrame: window.frame) }
                }
            case let .failure(error):
                self.showScreenshotError(error.localizedDescription, anchorRect: anchorRect)
            }
        }
    }

    private func beginScrollingAreaCapture(anchorRect: CGRect?) {
#if DEBUG
        if let area = e2eRegionArea() {
            Task { await self.startScrollingCapture(target: .area(area), anchorRect: anchorRect) }
            return
        }
#endif
        hidePopup()
        let controller = RegionCaptureOverlayController()
        regionCaptureController = controller
        controller.begin { [weak self] result in
            guard let self else { return }
            self.regionCaptureController = nil
            switch result {
            case let .success(capture):
                switch capture {
                case let .area(area):
                    Task { await self.startScrollingCapture(target: .area(area), anchorRect: anchorRect) }
                case let .window(window):
                    Task { await self.startScrollingCapture(target: .window(window), anchorRect: anchorRect) }
                }
            case let .failure(error):
                self.showScreenshotError(error.localizedDescription, anchorRect: anchorRect)
            }
        }
    }

    private func captureArea(_ area: CaptureArea, anchorRect: CGRect?) async {
        do {
            let document = try await screenCaptureService.capture(
                .area(area),
                options: CaptureOptions(includeCursor: settings.screenshotIncludeCursor, hideBelloBoxWindows: true, delayAfterHidingOverlays: 0.15)
            )
            showInlineScreenshotEditor(document: document, frame: area.cocoaRect)
        } catch {
            showScreenshotError(error.localizedDescription, anchorRect: anchorRect)
        }
    }

    private func captureScreen(anchorRect: CGRect?) {
        hidePopup()
        Task {
            do {
                let document = try await screenCaptureService.captureScreenUnderMouse(
                    options: CaptureOptions(includeCursor: settings.screenshotIncludeCursor, hideBelloBoxWindows: true, delayAfterHidingOverlays: 0.15)
                )
                showScreenshotEditor(document: document, anchorRect: anchorRect)
            } catch {
                showScreenshotError(error.localizedDescription, anchorRect: anchorRect)
            }
        }
    }

    private func showWindowPicker(anchorRect: CGRect?) {
        let viewModel = WindowCapturePickerViewModel(service: screenCaptureService)
        viewModel.onCancel = { [weak self] in self?.hidePopup() }
        viewModel.onSelect = { [weak self] window in
            self?.hidePopup()
            Task { await self?.captureWindow(window, anchorRect: anchorRect, preferInlineFrame: window.frame) }
        }
        let view = WindowCapturePickerView(viewModel: viewModel)
        present(
            view,
            size: WindowCapturePickerView.preferredSize,
            anchorRect: anchorRect,
            minimizedIcon: "macwindow",
            minimizedTitle: "Window Capture"
        )
    }

    private func captureWindow(_ window: CaptureWindow, anchorRect: CGRect?, preferInlineFrame: CGRect? = nil) async {
        do {
            let document = try await screenCaptureService.capture(
                .window(window),
                options: CaptureOptions(includeCursor: settings.screenshotIncludeCursor, hideBelloBoxWindows: true, delayAfterHidingOverlays: 0.15)
            )
            if let frame = preferInlineFrame {
                showInlineScreenshotEditor(document: document, frame: frame)
            } else {
                showScreenshotEditor(document: document, anchorRect: anchorRect)
            }
        } catch {
            showScreenshotError(error.localizedDescription, anchorRect: anchorRect)
        }
    }

    private func startScrollingCapture(target: ScrollCaptureTarget, anchorRect: CGRect?) async {
        let coordinator = ScrollCaptureCoordinator(target: target, service: screenCaptureService, settings: settings)
        scrollingCaptureCoordinator = coordinator
        do {
            try await coordinator.captureInitialFrame()
        } catch {
            showScreenshotError(error.localizedDescription, anchorRect: anchorRect)
            return
        }
        let viewModel = ScrollingCaptureHUDViewModel(coordinator: coordinator)
        viewModel.onCancel = { [weak self] in
            self?.scrollingCaptureCoordinator = nil
            self?.hidePopup()
        }
        viewModel.onFinished = { [weak self] document in
            self?.scrollingCaptureCoordinator = nil
            self?.showScreenshotEditor(document: document, anchorRect: anchorRect)
        }
        let view = ScrollingCaptureHUDView(viewModel: viewModel)
        present(
            view,
            size: ScrollingCaptureHUDView.preferredSize,
            anchorRect: anchorRect,
            minimizedIcon: "arrow.down.doc",
            minimizedTitle: "Scrolling Capture"
        )
    }

    private func showInlineScreenshotEditor(document: ScreenshotDocument, frame: CGRect) {
        hidePopup()
        let viewModel = ScreenshotPopupViewModel(
            document: document,
            settings: settings,
            macOCRService: macOCRService,
            llmOCRService: llmOCRService
        )
        viewModel.onClose = { [weak self] in self?.hideInlineScreenshotEditor() }

        let screen = ScreenCoordinateSpace.displayForCocoaRect(frame) ?? ScreenPlacement.screen(containing: CGPoint(x: frame.midX, y: frame.midY))
        let canvasSize = CGSize(
            width: max(120, min(frame.width, screen.visibleFrame.width - 24)),
            height: max(80, min(frame.height, screen.visibleFrame.height - InlineScreenshotEditorView.toolbarHeight - 44))
        )
        let panelSize = CGSize(
            width: max(canvasSize.width + 20, InlineScreenshotEditorView.minimumWidth),
            height: InlineScreenshotEditorView.toolbarHeight + max(canvasSize.height, InlineScreenshotEditorView.minimumCanvasHeight) + 42
        )
        let desiredOrigin = CGPoint(x: frame.minX, y: frame.minY)
        let origin = ScreenPlacement.clamp(origin: desiredOrigin, size: panelSize, into: screen)

        let view = InlineScreenshotEditorView(
            viewModel: viewModel,
            canvasSize: canvasSize,
            onMinimize: nil
        )
        let panel = InlineScreenshotEditorPanel(contentRect: CGRect(origin: origin, size: panelSize))
        panel.contentView = NSHostingView(rootView: view)
        panel.setFrame(NSRect(origin: origin, size: panelSize), display: true)
        panel.makeKeyAndOrderFront(nil)
        inlineScreenshotPanel = panel
    }

    private func hideInlineScreenshotEditor() {
        inlineScreenshotPanel?.orderOut(nil)
        inlineScreenshotPanel = nil
    }

    private func showScreenshotEditor(document: ScreenshotDocument, anchorRect: CGRect?) {
        let viewModel = ScreenshotPopupViewModel(
            document: document,
            settings: settings,
            macOCRService: macOCRService,
            llmOCRService: llmOCRService
        )
        viewModel.onClose = { [weak self] in self?.hidePopup() }
        let view = ScreenshotPopupView(
            viewModel: viewModel,
            onMinimize: { [weak self] in self?.minimizePopup() }
        )
        present(
            view,
            size: ScreenshotPopupView.preferredSize,
            anchorRect: anchorRect,
            minimizedIcon: "camera.viewfinder",
            minimizedTitle: "Screenshot"
        )
    }

    private func showScreenshotError(_ message: String, anchorRect: CGRect?) {
        let viewModel = ScreenshotCaptureChooserViewModel()
        viewModel.errorMessage = message
        viewModel.onClose = { [weak self] in self?.hidePopup() }
        viewModel.onCaptureArea = { [weak self] in self?.beginAreaCapture(anchorRect: anchorRect) }
        viewModel.onCaptureWindow = { [weak self] in self?.showWindowPicker(anchorRect: anchorRect) }
        viewModel.onCaptureScreen = { [weak self] in self?.captureScreen(anchorRect: anchorRect) }
        viewModel.onCaptureScrolling = { [weak self] in self?.beginScrollingAreaCapture(anchorRect: anchorRect) }
        let view = ScreenshotCaptureChooserView(viewModel: viewModel)
        present(
            view,
            size: ScreenshotCaptureChooserView.preferredSize,
            anchorRect: anchorRect,
            minimizedIcon: "camera.viewfinder",
            minimizedTitle: "Screenshot"
        )
    }

#if DEBUG
    private func runScreenshotE2EHooksIfNeeded() {
        let env = ProcessInfo.processInfo.environment
        if let path = env["BELLOBOX_E2E_SCROLL_FRAMES_DIR"], !path.isEmpty {
            Task { await openE2EScrollingFrames(path: path) }
            return
        }
        if let path = env["BELLOBOX_E2E_OCR_IMAGE"], !path.isEmpty {
            Task { await openE2EScreenshot(path: path, runOCR: true) }
            return
        }
        if let path = env["BELLOBOX_E2E_SCREENSHOT_IMAGE"], !path.isEmpty {
            Task { await openE2EScreenshot(path: path, runOCR: false) }
        }
    }

    private func e2eRegionArea() -> CaptureArea? {
        guard let raw = ProcessInfo.processInfo.environment["BELLOBOX_E2E_REGION_RECT"] else { return nil }
        let parts = raw.split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespacesAndNewlines)) }
        guard parts.count == 4 else { return nil }
        let rect = CGRect(x: CGFloat(parts[0]), y: CGFloat(parts[1]), width: CGFloat(parts[2]), height: CGFloat(parts[3]))
        return CaptureArea(cocoaRect: rect, displayID: ScreenCoordinateSpace.displayForCocoaRect(rect).flatMap(ScreenCoordinateSpace.displayID(for:)))
    }

    private func openE2EScreenshot(path: String, runOCR: Bool) async {
        guard let image = Self.cgImage(at: path) else { return }
        let document = ScreenshotDocument(baseImage: image, scale: 1, source: .importedClipboard)
        let viewModel = ScreenshotPopupViewModel(
            document: document,
            settings: settings,
            macOCRService: macOCRService,
            llmOCRService: llmOCRService
        )
        viewModel.onClose = { [weak self] in self?.hidePopup() }
        let view = ScreenshotPopupView(viewModel: viewModel, onMinimize: { [weak self] in self?.minimizePopup() })
        present(view, size: ScreenshotPopupView.preferredSize, anchorRect: nil, minimizedIcon: "camera.viewfinder", minimizedTitle: "Screenshot")
        if runOCR { viewModel.runMacOCR() }
    }

    private func openE2EScrollingFrames(path: String) async {
        let url = URL(fileURLWithPath: path)
        let files = (try? FileManager.default.contentsOfDirectory(at: url, includingPropertiesForKeys: nil)) ?? []
        let images = files
            .filter { ["png", "jpg", "jpeg"].contains($0.pathExtension.lowercased()) }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
            .compactMap { Self.cgImage(at: $0.path) }
        guard !images.isEmpty, let result = try? ImageStitcher.stitch(images) else { return }
        let document = ScreenshotDocument(
            baseImage: result.image,
            scale: 1,
            source: .scrolling(target: ScrollCaptureTargetSummary(title: "E2E", ownerName: nil, frame: nil), frameCount: images.count)
        )
        showScreenshotEditor(document: document, anchorRect: nil)
    }

    private static func cgImage(at path: String) -> CGImage? {
        guard let image = NSImage(contentsOfFile: path) else { return nil }
        var rect = CGRect(origin: .zero, size: image.size)
        return image.cgImage(forProposedRect: &rect, context: nil, hints: nil)
    }
#endif

    private func present<V: View>(
        _ view: V,
        size: CGSize,
        anchorRect: CGRect?,
        minimizedIcon: String,
        minimizedTitle: String,
        minimizedSubtitle: @escaping () -> String? = { nil }
    ) {
        hidePopup()
        let origin = ScreenPlacement.popupOrigin(
            anchorRect: anchorRect,
            mouse: NSEvent.mouseLocation,
            size: size
        )
        let panel = PopupPanel(contentRect: CGRect(origin: origin, size: size))
        let hosting = NSHostingView(rootView: view)
        panel.contentView = hosting
        panel.setFrameOrigin(origin)
        panel.makeKeyAndOrderFront(nil)
        popupPanel = panel
        popupFullContentView = hosting
        popupFullSize = size
        popupIsMinimized = false
        popupMinimizedIcon = minimizedIcon
        popupMinimizedTitle = minimizedTitle
        popupMinimizedSubtitle = minimizedSubtitle
        // Note: the popup intentionally does NOT dismiss on an outside click, so
        // it stays put while you work (copy/paste, switch apps). Close it with
        // the × button or Esc.
    }

    private func minimizePopup() {
        guard let panel = popupPanel, !popupIsMinimized else { return }
        popupFullContentView = panel.contentView
        popupIsMinimized = true

        let size = minimizedPopupSize()
        let oldFrame = panel.frame
        let origin = ScreenPlacement.clamp(
            origin: CGPoint(x: oldFrame.minX, y: oldFrame.maxY - size.height),
            size: size,
            into: ScreenPlacement.screen(containing: CGPoint(x: oldFrame.midX, y: oldFrame.midY))
        )

        let bar = MinimizedPopupBar(
            icon: popupMinimizedIcon,
            title: popupMinimizedTitle,
            subtitle: popupMinimizedSubtitle?(),
            onRestore: { [weak self] in self?.restorePopup() },
            onClose: { [weak self] in self?.hidePopup() }
        )
        panel.contentView = NSHostingView(rootView: bar.frame(width: size.width, height: size.height))
        panel.setFrame(NSRect(origin: origin, size: size), display: true, animate: true)
        panel.orderFrontRegardless()
    }

    private func restorePopup() {
        guard let panel = popupPanel, popupIsMinimized, let contentView = popupFullContentView else { return }
        let size = popupFullSize
        let oldFrame = panel.frame
        let origin = ScreenPlacement.clamp(
            origin: CGPoint(x: oldFrame.minX, y: oldFrame.maxY - size.height),
            size: size,
            into: ScreenPlacement.screen(containing: CGPoint(x: oldFrame.midX, y: oldFrame.midY))
        )

        panel.setFrame(NSRect(origin: origin, size: size), display: true, animate: true)
        panel.contentView = contentView
        panel.makeKeyAndOrderFront(nil)
        popupIsMinimized = false
    }

    private func minimizedPopupSize() -> CGSize {
        let width = min(max(popupFullSize.width * 0.52, 340), 430)
        return CGSize(width: width, height: 66)
    }

    private func hidePopup() {
        popupPanel?.orderOut(nil)
        popupPanel = nil
        hideInlineScreenshotEditor()
        popupFullContentView = nil
        popupFullSize = .zero
        popupIsMinimized = false
        popupMinimizedIcon = ""
        popupMinimizedTitle = ""
        popupMinimizedSubtitle = nil
        pendingSelection = nil
    }
}
