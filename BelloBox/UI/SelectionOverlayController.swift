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
    private lazy var recordingCoordinator = RecordingCoordinator(settings: settings)

    private var toolbarPanel: FloatingButtonPanel?
    private var popupPanel: PopupPanel?
    private var popupFullContentView: NSView?
    private var popupFullSize: CGSize = .zero
    private var popupIsMinimized = false
    private var popupMinimizedIcon = ""
    private var popupMinimizedTitle = ""
    private var popupMinimizedSubtitle: (() -> String?)?
    private var popupOnDismiss: (() -> Void)?
    private var screenshotOverlayEditorController: ScreenshotOverlayEditorController?
    private var captureOverlayController: CaptureOverlayController?
    private var toolbarDismissMonitor: Any?

    private var pendingSelection: TextSelection?
    private var trustWatcher: Timer?
    private var lastTrusted = false
    private var regionCaptureController: RegionCaptureOverlayController?
    private var scrollingCaptureCoordinator: ScrollCaptureCoordinator?
#if DEBUG
    private var e2eRecordingPulseWindow: NSWindow?
#endif

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
            self?.screenshotOverlayEditorController?.close()
        }
        screenCaptureService.afterCapture = { [weak self] in
            self?.popupPanel?.orderFrontRegardless()
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
        runE2EHooksIfNeeded()
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

    func setShortcutRecordingActive(_ active: Bool) {
        monitor.hotkeysSuspended = active
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
        monitor.hotkeysSuspended = settings.activeShortcutRecorderID != nil
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
        guard !isCaptureSurfaceActive else { return }
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
        guard !isCaptureSurfaceActive else { NSSound.beep(); return }
        guard let selection = nonEmpty(currentSelection()) else { NSSound.beep(); return }
        hideToolbar()
        showAIPopup(for: selection)
    }

    /// Used by the global hotkey: read the selection now and show the tool board.
    func triggerBoardOnCurrentSelection() {
        guard !isCaptureSurfaceActive else { NSSound.beep(); return }
        guard popupPanel == nil else { return }
        guard let selection = nonEmpty(currentSelection()) else { NSSound.beep(); return }
        pendingSelection = selection
        showToolbar(for: selection)
    }

    /// Used by the menu: read the selection now and open the QR popup.
    func triggerQROnCurrentSelection() {
        guard !isCaptureSurfaceActive else { NSSound.beep(); return }
        guard let selection = nonEmpty(currentSelection()) else { NSSound.beep(); return }
        hideToolbar()
        showQRPopup(for: selection)
    }

    /// Used by the menu: read the selection now and open the text-tools popup.
    func triggerTextToolsOnCurrentSelection() {
        guard !isCaptureSurfaceActive else { NSSound.beep(); return }
        guard let selection = nonEmpty(currentSelection()) else { NSSound.beep(); return }
        hideToolbar()
        showTextToolsPopup(for: selection)
    }

    private var isCaptureSurfaceActive: Bool {
        captureOverlayController != nil
            || regionCaptureController != nil
            || screenshotOverlayEditorController != nil
            || scrollingCaptureCoordinator != nil
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
        beginUnifiedScreenshotCapture(anchorRect: anchor)
    }

    private func activateRecording() {
        let anchor = pendingSelection?.anchorRect
        hideToolbar()
        beginUnifiedRecordingCapture(anchorRect: anchor)
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
            onDismiss: { viewModel.cancel() },
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

    func triggerRecording() {
#if DEBUG
        if writeE2EHotkeyMarkerIfNeeded(kind: "recording") { return }
#endif
        guard !isCaptureSurfaceActive else { NSSound.beep(); return }
        hideToolbar()
        beginUnifiedRecordingCapture(anchorRect: nil)
    }

    func stopRecording() {
        recordingCoordinator.stop()
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
            options: options,
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

    private func recordingTarget(for selection: CaptureSelection) -> RecordingTarget? {
        switch selection {
        case let .area(area):
            return recordingTarget(for: area)
        case let .window(window):
            return recordingTarget(for: window)
        case let .display(display):
            return .display(displayID: display.displayID)
        }
    }

    private func beginUnifiedRecordingCapture(anchorRect: CGRect?, options: RecordingOptions? = nil) {
        let options = options ?? settings.recordingOptions
        let permissions = recordingCoordinator.permissionState(options: options)
        guard permissions.canRecordVideo else {
            showRecordingPermissions(permissions: permissions, options: options, anchorRect: anchorRect) { [weak self] sanitized in
                self?.beginUnifiedRecordingCapture(anchorRect: anchorRect, options: sanitized)
            }
            return
        }

        recordingCoordinator.showRecordingChooser(anchor: anchorRect)
        hidePopup()
        captureOverlayController?.cancel()
        let controller = CaptureOverlayController(
            screenCaptureService: screenCaptureService,
            settings: settings,
            macOCRService: macOCRService
        )
        captureOverlayController = controller
        controller.beginRecording(
            initialOptions: options,
            onRecord: { [weak self] selection, chosenOptions in
                guard let self else { return }
                self.captureOverlayController = nil
                guard let target = self.recordingTarget(for: selection) else {
                    self.showRecordingError("No display could be found for this recording target.", anchorRect: anchorRect)
                    return
                }
                self.prepareRecording(options: chosenOptions, anchorRect: anchorRect) { preparedOptions in
                    Task { await self.recordingCoordinator.start(target: target, options: preparedOptions) }
                }
            },
            onError: { [weak self] message in
                self?.captureOverlayController = nil
                self?.showRecordingError(message, anchorRect: anchorRect)
            },
            onCancel: { [weak self] in
                self?.captureOverlayController = nil
                self?.recordingCoordinator.cancel()
            }
        )
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
                size: recordingCountdownSize(),
                anchorRect: nil,
                minimizedIcon: "record.circle",
                minimizedTitle: "Recording",
                onDismiss: { [weak self] in self?.recordingCoordinator.cancel() },
                runExistingDismissAction: false
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
                size: recordingHUDSize(),
                anchorRect: nil,
                minimizedIcon: "record.circle",
                minimizedTitle: "Recording",
                runExistingDismissAction: false
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
                size: recordingHUDSize(),
                anchorRect: nil,
                minimizedIcon: "record.circle",
                minimizedTitle: "Recording",
                runExistingDismissAction: false
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
                minimizedTitle: "Recording",
                runExistingDismissAction: false
            )
        case .finishing:
            present(
                RecordingFinishingView(),
                size: CGSize(width: 320, height: 190),
                anchorRect: nil,
                minimizedIcon: "record.circle",
                minimizedTitle: "Recording",
                runExistingDismissAction: false
            )
        case let .failed(message):
            showRecordingError(message, anchorRect: nil)
        case .idle, .requestingPermissions, .choosingTarget:
            break
        }
    }

    private func recordingCountdownSize() -> CGSize {
        RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: AccessibilityService.isTrusted) == nil
            ? CGSize(width: 320, height: 240)
            : CGSize(width: 340, height: 280)
    }

    private func recordingHUDSize() -> CGSize {
        RecordingPrivacyNotice.secureFieldRedactionWarning(accessibilityTrusted: AccessibilityService.isTrusted) == nil
            ? CGSize(width: 520, height: 80)
            : CGSize(width: 680, height: 80)
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
            minimizedTitle: "Recording",
            onDismiss: { [weak self] in self?.recordingCoordinator.cancel() }
        )
    }

    private func screen(for displayID: CGDirectDisplayID) -> NSScreen? {
        NSScreen.screens.first { ScreenCoordinateSpace.displayID(for: $0) == displayID }
    }

    // MARK: - Screenshots

    func triggerScreenshotCapture() {
        guard !isCaptureSurfaceActive else { NSSound.beep(); return }
        hideToolbar()
        beginUnifiedScreenshotCapture(anchorRect: nil)
    }

    func triggerScrollingScreenshotCapture() {
        guard !isCaptureSurfaceActive else { NSSound.beep(); return }
        hideToolbar()
        showScreenshotChooser(anchorRect: nil, initialMode: .scrolling)
    }

    func triggerScreenshotShortcut() {
#if DEBUG
        if writeE2EHotkeyMarkerIfNeeded(kind: "screenshot") { return }
#endif
        guard !isCaptureSurfaceActive else { NSSound.beep(); return }
        hideToolbar()
        guard ScreenCapturePermission.isTrusted else {
            showScreenshotChooser(anchorRect: nil, initialMode: screenshotCaptureMode(from: settings.screenshotDefaultMode))
            return
        }
        beginUnifiedScreenshotCapture(anchorRect: nil)
    }

    private func beginUnifiedScreenshotCapture(anchorRect: CGRect?) {
#if DEBUG
        if let area = e2eRegionArea() {
            Task { await self.captureArea(area, anchorRect: anchorRect) }
            return
        }
#endif
        hidePopup()
        captureOverlayController?.cancel()
        let controller = CaptureOverlayController(
            screenCaptureService: screenCaptureService,
            settings: settings,
            macOCRService: macOCRService
        )
        captureOverlayController = controller
        controller.beginScreenshot(
            onError: { [weak self] message in
                self?.captureOverlayController = nil
                self?.showScreenshotError(message, anchorRect: anchorRect)
            },
            onCancel: { [weak self] in
                self?.captureOverlayController = nil
            }
        )
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
        beginUnifiedScreenshotCapture(anchorRect: anchorRect)
    }

    private func beginScrollingAreaCapture(anchorRect: CGRect?) {
#if DEBUG
        if let area = e2eRegionArea() {
            Task { await self.startScrollingCapture(target: .area(area), anchorRect: anchorRect) }
            return
        }
#endif
        hidePopup()
        regionCaptureController?.cancel()
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
            case .failure(.userCancelled):
                break
            case let .failure(error):
                self.showScreenshotError(error.localizedDescription, anchorRect: anchorRect)
            }
        }
    }

    private func captureArea(_ area: CaptureArea, anchorRect: CGRect?) async {
        do {
            let document = try await screenCaptureService.capture(
                .area(area),
                options: CaptureOptions(includeCursor: settings.screenshotIncludeCursor, hideBelloBoxWindows: true, delayAfterHidingOverlays: 0.05)
            )
            showScreenshotOverlayEditor(document: document, frame: area.cocoaRect)
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
            minimizedTitle: "Window Capture",
            onDismiss: { viewModel.cancelLoad() }
        )
    }

    private func captureWindow(_ window: CaptureWindow, anchorRect: CGRect?, preferInlineFrame: CGRect? = nil) async {
        do {
            let document = try await screenCaptureService.capture(
                .window(window),
                options: CaptureOptions(includeCursor: settings.screenshotIncludeCursor, hideBelloBoxWindows: true, delayAfterHidingOverlays: 0.05)
            )
            if let frame = preferInlineFrame {
                showScreenshotOverlayEditor(document: document, frame: frame)
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
            if scrollingCaptureCoordinator === coordinator {
                scrollingCaptureCoordinator = nil
            }
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
            minimizedTitle: "Scrolling Capture",
            onDismiss: { viewModel.cancel() }
        )
    }

    private func showScreenshotOverlayEditor(document: ScreenshotDocument, frame: CGRect) {
        hidePopup()
        let viewModel = ScreenshotPopupViewModel(
            document: document,
            settings: settings,
            macOCRService: macOCRService
        )
        let controller = ScreenshotOverlayEditorController()
        viewModel.onClose = { [weak self, weak controller] in
            controller?.closeFromViewModel()
            if self?.screenshotOverlayEditorController === controller {
                self?.screenshotOverlayEditorController = nil
            }
        }
        controller.show(viewModel: viewModel, captureFrame: frame)
        screenshotOverlayEditorController = controller
    }

    private func hideScreenshotOverlayEditor() {
        screenshotOverlayEditorController?.close()
        screenshotOverlayEditorController = nil
    }

    private func showScreenshotEditor(document: ScreenshotDocument, anchorRect: CGRect?) {
        let viewModel = ScreenshotPopupViewModel(
            document: document,
            settings: settings,
            macOCRService: macOCRService
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
            minimizedTitle: "Screenshot",
            onDismiss: { viewModel.close() }
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
    private func runE2EHooksIfNeeded() {
        if runRealScreenshotE2EHookIfNeeded() { return }
        if runRealRecordingE2EHookIfNeeded() { return }
        runScreenshotE2EHooksIfNeeded()
    }

    @discardableResult
    private func writeE2EHotkeyMarkerIfNeeded(kind: String) -> Bool {
        let env = ProcessInfo.processInfo.environment
        let key = "BELLOBOX_E2E_\(kind.uppercased())_HOTKEY_MARKER"
        guard let path = env[key], !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return false }
        let url = URL(fileURLWithPath: path)
        let payload = [
            "kind=\(kind)",
            "shownAt=\(Date().timeIntervalSince1970)"
        ].joined(separator: "\n")
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? payload.write(to: url, atomically: true, encoding: .utf8)
        return env["BELLOBOX_E2E_HOTKEY_MARKERS_ONLY"] == "1"
    }

    @discardableResult
    private func runRealScreenshotE2EHookIfNeeded() -> Bool {
        let env = ProcessInfo.processInfo.environment
        guard let outputPath = env["BELLOBOX_E2E_REAL_SCREENSHOT_OUTPUT"], !outputPath.isEmpty else { return false }
        let markerPath = env["BELLOBOX_E2E_REAL_SCREENSHOT_MARKER"]

        Task { @MainActor in
            do {
                guard ScreenCapturePermission.isTrusted else {
                    throw ScreenCaptureService.CaptureError.permissionDenied
                }
                guard let screen = NSScreen.main,
                      let displayID = ScreenCoordinateSpace.displayID(for: screen),
                      let rect = e2eCaptureRect(on: screen, defaultSize: CGSize(width: 320, height: 200))
                else {
                    throw ScreenCaptureService.CaptureError.noDisplayFound
                }

                let document = try await screenCaptureService.capture(
                    .area(CaptureArea(cocoaRect: rect, displayID: displayID)),
                    options: CaptureOptions(includeCursor: false, hideBelloBoxWindows: false, delayAfterHidingOverlays: 0)
                )
                let rendered = try AnnotationRenderer.render(document)
                try Self.writePNG(rendered, to: outputPath)
                Self.writeE2EMarker(
                    markerPath,
                    lines: [
                        "kind=real-screenshot",
                        "status=success",
                        "path=\(outputPath)",
                        "rect=\(Self.serialize(rect))",
                        "scale=\(document.scale)",
                        "imageWidth=\(rendered.width)",
                        "imageHeight=\(rendered.height)",
                        "fileSize=\(Self.fileSize(at: outputPath))",
                    ]
                )
            } catch {
                Self.writeE2EMarker(
                    markerPath,
                    lines: [
                        "kind=real-screenshot",
                        "status=failure",
                        "error=\(error.localizedDescription)",
                    ]
                )
            }
            e2eQuitIfRequested()
        }
        return true
    }

    @discardableResult
    private func runRealRecordingE2EHookIfNeeded() -> Bool {
        let env = ProcessInfo.processInfo.environment
        guard let outputPath = env["BELLOBOX_E2E_REAL_RECORDING_OUTPUT"], !outputPath.isEmpty else { return false }
        let markerPath = env["BELLOBOX_E2E_REAL_RECORDING_MARKER"]
        let duration = max(0.6, min(5, Double(env["BELLOBOX_E2E_RECORDING_DURATION"] ?? "") ?? 1.2))
        let showOwnPulse = env["BELLOBOX_E2E_RECORDING_OWN_PULSE"] == "1"

        Task { @MainActor in
            var engineForDiagnostics: RecordingEngine?
            do {
                guard ScreenCapturePermission.isTrusted else {
                    throw RecordingEngineError.permissionDenied
                }
                guard let screen = NSScreen.main,
                      let displayID = ScreenCoordinateSpace.displayID(for: screen),
                      let rect = e2eCaptureRect(on: screen, defaultSize: CGSize(width: 360, height: 220))
                else {
                    throw RecordingEngineError.noDisplayFound
                }

                let options = RecordingOptions(
                    audioSource: .none,
                    microphoneDeviceID: nil,
                    includeCursor: false,
                    clickOverlayMode: .off,
                    keystrokeMode: .off,
                    secureFieldRedactionMode: .strict,
                    quality: .compact,
                    countdownSeconds: 0,
                    excludeBelloBoxWindows: false,
                    excludesCurrentProcessAudio: true
                )
                let engine = RecordingEngine(
                    target: .area(displayID: displayID, rectInScreenPoints: rect),
                    options: options,
                    outputURL: URL(fileURLWithPath: outputPath)
                )
                engineForDiagnostics = engine

                if showOwnPulse {
                    showE2ERecordingPulseWindow(in: rect)
                }
                let runtime = try await engine.start()
                try await Task.sleep(nanoseconds: UInt64(duration * 1_000_000_000))
                let movieURL = try await engine.stop()
                if showOwnPulse {
                    hideE2ERecordingPulseWindow()
                }
                Self.writeE2EMarker(
                    markerPath,
                    lines: [
                        "kind=real-recording",
                        "status=success",
                        "path=\(movieURL.path)",
                        "rect=\(Self.serialize(rect))",
                        "duration=\(duration)",
                        "target=\(runtime.targetDescription)",
                        "fileSize=\(Self.fileSize(at: movieURL.path))",
                    ]
                )
            } catch {
                if showOwnPulse {
                    hideE2ERecordingPulseWindow()
                }
                let diagnostics = engineForDiagnostics?.diagnosticsSummary ?? "none"
                let lines = [
                    "kind=real-recording",
                    "status=failure",
                    "error=\(error.localizedDescription)",
                    "diagnostics=\(diagnostics)",
                ]
                Self.writeE2EMarker(
                    markerPath,
                    lines: lines
                )
            }
            e2eQuitIfRequested()
        }
        return true
    }

    private func showE2ERecordingPulseWindow(in rect: CGRect) {
        hideE2ERecordingPulseWindow()
        let width = min(max(120, rect.width * 0.45), rect.width)
        let height = min(max(90, rect.height * 0.45), rect.height)
        let windowRect = CGRect(
            x: rect.midX - width / 2,
            y: rect.midY - height / 2,
            width: width,
            height: height
        )
        let window = NSWindow(
            contentRect: windowRect,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.level = .floating
        window.isOpaque = true
        window.backgroundColor = .systemOrange
        window.hasShadow = false
        window.isReleasedWhenClosed = false
        window.contentView = E2ERecordingPulseView(frame: CGRect(origin: .zero, size: windowRect.size))
        window.orderFrontRegardless()
        e2eRecordingPulseWindow = window
    }

    private func hideE2ERecordingPulseWindow() {
        e2eRecordingPulseWindow?.orderOut(nil)
        e2eRecordingPulseWindow = nil
    }

    private func runScreenshotE2EHooksIfNeeded() {
        let env = ProcessInfo.processInfo.environment
        if let path = env["BELLOBOX_E2E_CAPTURE_OVERLAY_IMAGE"], !path.isEmpty {
            openE2ECaptureOverlay(path: path)
            return
        }
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
            macOCRService: macOCRService
        )
        viewModel.onClose = { [weak self] in self?.hidePopup() }
        let view = ScreenshotPopupView(viewModel: viewModel, onMinimize: { [weak self] in self?.minimizePopup() })
        present(
            view,
            size: ScreenshotPopupView.preferredSize,
            anchorRect: nil,
            minimizedIcon: "camera.viewfinder",
            minimizedTitle: "Screenshot",
            onDismiss: { viewModel.close() }
        )
        if runOCR { viewModel.runMacOCR() }
    }

    private func openE2ECaptureOverlay(path: String) {
        guard let image = Self.cgImage(at: path),
              let screen = NSScreen.main,
              let displayID = ScreenCoordinateSpace.displayID(for: screen)
        else { return }
        hidePopup()
        let scale = screen.frame.width > 0 ? CGFloat(image.width) / screen.frame.width : ScreenCoordinateSpace.backingScale(for: screen)
        let snapshot = DisplaySnapshot(
            displayID: displayID,
            screenFrame: screen.frame,
            scale: max(scale, 1),
            image: image
        )
        let initialSelection = Self.e2eOverlaySelection(
            raw: ProcessInfo.processInfo.environment["BELLOBOX_E2E_CAPTURE_OVERLAY_RECT"],
            displayID: displayID,
            screenFrame: screen.frame
        )
        let controller = CaptureOverlayController(
            screenCaptureService: screenCaptureService,
            settings: settings,
            macOCRService: macOCRService
        )
        captureOverlayController?.cancel()
        captureOverlayController = controller
        controller.beginScreenshotForTesting(
            snapshots: [snapshot],
            initialSelection: initialSelection,
            onError: { [weak self] message in
                self?.captureOverlayController = nil
                self?.showScreenshotError(message, anchorRect: nil)
            },
            onCancel: { [weak self] in
                self?.captureOverlayController = nil
            }
        )
    }

    private static func e2eOverlaySelection(raw: String?, displayID: CGDirectDisplayID, screenFrame: CGRect) -> CaptureSelection? {
        guard let raw, !raw.isEmpty else {
            guard ProcessInfo.processInfo.environment["BELLOBOX_E2E_CAPTURE_OVERLAY_AUTO_SELECT"] == "1" else { return nil }
            let width = min(max(180, screenFrame.width * 0.28), 420)
            let height = min(max(120, screenFrame.height * 0.22), 280)
            let rect = CGRect(
                x: screenFrame.midX - width / 2,
                y: screenFrame.midY - height / 2,
                width: width,
                height: height
            )
            return .area(CaptureArea(cocoaRect: rect, displayID: displayID))
        }
        let parts = raw.split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespacesAndNewlines)) }
        guard parts.count == 4 else { return nil }
        let rect = CGRect(x: CGFloat(parts[0]), y: CGFloat(parts[1]), width: CGFloat(parts[2]), height: CGFloat(parts[3]))
        return .area(CaptureArea(cocoaRect: rect, displayID: displayID))
    }

    private func openE2EScrollingFrames(path: String) async {
        let url = URL(fileURLWithPath: path)
        let files = (try? FileManager.default.contentsOfDirectory(at: url, includingPropertiesForKeys: nil)) ?? []
        let images = files
            .filter { ["png", "jpg", "jpeg"].contains($0.pathExtension.lowercased()) }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
            .compactMap { Self.cgImage(at: $0.path) }
        guard !images.isEmpty, let result = try? ImageStitcher.stitch(images) else { return }
        let document = ScrollCaptureCoordinator.makeDocument(
            from: result,
            target: ScrollCaptureTargetSummary(title: "E2E", ownerName: nil, frame: nil),
            frameCount: images.count
        )
        showScreenshotEditor(document: document, anchorRect: nil)
    }

    private static func cgImage(at path: String) -> CGImage? {
        guard let image = NSImage(contentsOfFile: path) else { return nil }
        var rect = CGRect(origin: .zero, size: image.size)
        return image.cgImage(forProposedRect: &rect, context: nil, hints: nil)
    }

    private func e2eCaptureRect(on screen: NSScreen, defaultSize: CGSize) -> CGRect? {
        if let raw = ProcessInfo.processInfo.environment["BELLOBOX_E2E_CAPTURE_RECT"], !raw.isEmpty {
            let parts = raw.split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespacesAndNewlines)) }
            guard parts.count == 4 else { return nil }
            return CGRect(x: CGFloat(parts[0]), y: CGFloat(parts[1]), width: CGFloat(parts[2]), height: CGFloat(parts[3]))
                .intersection(screen.frame)
                .standardized
        }

        let width = min(defaultSize.width, max(80, screen.frame.width * 0.4))
        let height = min(defaultSize.height, max(80, screen.frame.height * 0.3))
        return CGRect(
            x: screen.frame.midX - width / 2,
            y: screen.frame.midY - height / 2,
            width: width,
            height: height
        )
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

    private static func serialize(_ rect: CGRect) -> String {
        "\(rect.origin.x),\(rect.origin.y),\(rect.size.width),\(rect.size.height)"
    }
#endif

    private func present<V: View>(
        _ view: V,
        size: CGSize,
        anchorRect: CGRect?,
        minimizedIcon: String,
        minimizedTitle: String,
        onDismiss: (() -> Void)? = nil,
        runExistingDismissAction: Bool = true,
        minimizedSubtitle: @escaping () -> String? = { nil }
    ) {
        hidePopup(runDismissAction: runExistingDismissAction)
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
        popupOnDismiss = onDismiss
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

    private func hidePopup(runDismissAction: Bool = true) {
        let onDismiss = runDismissAction ? popupOnDismiss : nil
        popupOnDismiss = nil
        popupPanel?.orderOut(nil)
        popupPanel = nil
        hideScreenshotOverlayEditor()
        // Scrolling capture owns scrollingCaptureCoordinator through its HUD
        // callbacks and initial-frame error path; do not clear it here, because
        // present() uses hidePopup() before installing the scrolling HUD.
        popupFullContentView = nil
        popupFullSize = .zero
        popupIsMinimized = false
        popupMinimizedIcon = ""
        popupMinimizedTitle = ""
        popupMinimizedSubtitle = nil
        pendingSelection = nil
        onDismiss?()
    }
}

#if DEBUG
private final class E2ERecordingPulseView: NSView {
    private var timer: Timer?
    private var tick = 0

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        timer = Timer.scheduledTimer(withTimeInterval: 0.12, repeats: true) { [weak self] _ in
            guard let self else { return }
            tick += 1
            needsDisplay = true
        }
        RunLoop.main.add(timer!, forMode: .common)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    deinit {
        timer?.invalidate()
    }

    override func draw(_ dirtyRect: NSRect) {
        let palette: [NSColor] = [.systemOrange, .systemBlue, .systemGreen, .systemPink]
        palette[tick % palette.count].setFill()
        bounds.fill()

        NSColor.white.withAlphaComponent(0.9).setStroke()
        let inset = CGFloat(8 + (tick % 6) * 3)
        let ring = NSBezierPath(ovalIn: bounds.insetBy(dx: inset, dy: inset))
        ring.lineWidth = 6
        ring.stroke()
    }
}
#endif
