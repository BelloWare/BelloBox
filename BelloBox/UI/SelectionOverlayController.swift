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
    private var toolbarTooltipPanel: FloatingTooltipPanel?
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
#if DEBUG
    private var e2eScreenshotPulseWindow: NSWindow?
    private var e2eRecordingPulseWindow: NSWindow?
#endif

    /// Set by the app to open the Settings window.
    var openSettings: () -> Void = {}
    /// Set by the app to open the persistent world clock at a selected instant.
    var openWorldClock: (Date) -> Void = { _ in }

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
            self?.hideToolbar(animated: false)
            // Skip the utility-window fade: a panel still fading out would be frozen
            // into the display snapshot taken a few milliseconds later.
            self?.popupPanel?.animationBehavior = .none
            self?.popupPanel?.orderOut(nil)
            self?.screenshotOverlayEditorController?.close()
        }
        screenCaptureService.afterCapture = { [weak self] in
            self?.popupPanel?.orderFrontRegardless()
            self?.popupPanel?.animationBehavior = .utilityWindow
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
        showToolbar(
            for: selection,
            timestampSummary: TimestampSummary.make(from: selection.text)
        )
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
            || screenshotOverlayEditorController != nil
    }

    // MARK: - Floating toolbar

    private func showToolbar(for selection: TextSelection, timestampSummary: TimestampSummary? = nil) {
        hideToolbar()

        let view = FloatingToolbarView(
            onAI: { [weak self] in self?.activateAI() },
            onScreenshot: { [weak self] in self?.activateScreenshot() },
            onRecord: { [weak self] in self?.activateRecording() },
            onQR: { [weak self] in self?.activateQR() },
            onTools: { [weak self] in self?.activateTools() },
            onOpenWorldClock: { [weak self] date in self?.activateWorldClock(at: date) },
            onHoverHelp: { [weak self] text in self?.updateToolbarTooltip(text) },
            timestampSummary: timestampSummary
        )
        let hosting = NSHostingView(rootView: view)
        var size = hosting.fittingSize
        if size.width < 1 || size.height < 1 {
            size = timestampSummary == nil
                ? FloatingToolbarView.preferredSize
                : FloatingToolbarView.timestampPreferredSize
        }

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
        writeE2EToolbarMarker(selection: selection, timestampSummary: timestampSummary)
#endif

        installToolbarDismissMonitor()
    }

#if DEBUG
    private func writeE2EToolbarMarker(selection: TextSelection, timestampSummary: TimestampSummary?) {
        guard
            let path = ProcessInfo.processInfo.environment["BELLOBOX_E2E_TOOLBAR_MARKER"],
            !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return }

        let url = URL(fileURLWithPath: path)
        var payload = [
            "shownAt=\(Date().timeIntervalSince1970)",
            "appName=\(selection.appName ?? "")",
            "text=\(selection.text)",
            "timestampDetected=\(timestampSummary != nil)",
        ]
        if let timestampSummary {
            payload.append("timestamp.relative=\(timestampSummary.relativeTime)")
            payload.append("timestamp.local=\(timestampSummary.localDateTime)")
        }
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? payload.joined(separator: "\n").write(to: url, atomically: true, encoding: .utf8)
    }
#endif

    private func activateAI() {
        guard let selection = pendingSelection else { return }
        hideToolbar()
        showAIPopup(for: selection)
    }

    private func activateScreenshot() {
        let anchor = pendingSelection?.anchorRect
        hideToolbar(animated: false)
        beginUnifiedScreenshotCapture(anchorRect: anchor)
    }

    private func activateRecording() {
        let anchor = pendingSelection?.anchorRect
        hideToolbar(animated: false)
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

    private func activateWorldClock(at date: Date) {
        hideToolbar()
        openWorldClock(date)
    }

    private func updateToolbarTooltip(_ text: String?) {
        guard let text, !text.isEmpty, let toolbarPanel else {
            toolbarTooltipPanel?.orderOut(nil)
            return
        }

        let tooltip = toolbarTooltipPanel ?? FloatingTooltipPanel()
        toolbarTooltipPanel = tooltip
        tooltip.update(text: text)

        let mouse = NSEvent.mouseLocation
        let screen = ScreenPlacement.screen(containing: mouse)
        let size = tooltip.frame.size
        var origin = CGPoint(
            x: mouse.x - size.width / 2,
            y: toolbarPanel.frame.maxY + 7
        )
        if origin.y + size.height > screen.visibleFrame.maxY - 6 {
            origin.y = toolbarPanel.frame.minY - size.height - 7
        }
        origin = ScreenPlacement.clamp(origin: origin, size: size, into: screen)
        tooltip.setFrameOrigin(origin)
        tooltip.orderFrontRegardless()
    }

    /// `animated: false` drops the utility-window fade so a capture that starts right
    /// after this call never freezes a half-transparent toolbar ghost into the snapshot.
    private func hideToolbar(animated: Bool = true) {
        if let monitor = toolbarDismissMonitor {
            NSEvent.removeMonitor(monitor)
            toolbarDismissMonitor = nil
        }
        if !animated {
            toolbarTooltipPanel?.animationBehavior = .none
            toolbarPanel?.animationBehavior = .none
        }
        toolbarTooltipPanel?.orderOut(nil)
        toolbarPanel?.orderOut(nil)
        toolbarPanel = nil
        if !animated {
            // The tooltip panel is reused; give it its fade back for normal dismissals.
            toolbarTooltipPanel?.animationBehavior = .utilityWindow
        }
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
        hidePopup(animated: false)
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
                onStop: { [weak self] in self?.recordingCoordinator.stop() },
                onInputOverlaysChange: { [weak self] clicks, keys in
                    self?.recordingCoordinator.updateInputOverlays(clicks: clicks, keys: keys)
                }
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
                onStop: { [weak self] in self?.recordingCoordinator.stop() },
                onInputOverlaysChange: { [weak self] clicks, keys in
                    self?.recordingCoordinator.updateInputOverlays(clicks: clicks, keys: keys)
                }
            )
            present(
                view,
                size: recordingHUDSize(),
                anchorRect: nil,
                minimizedIcon: "record.circle",
                minimizedTitle: "Recording",
                runExistingDismissAction: false
            )
        case let .reviewing(url, warning):
            let viewModel = RecordingReviewViewModel(fileURL: url, recoveryWarning: warning)
            viewModel.onClose = { [weak self] in self?.hidePopup() }
            let view = RecordingReviewView(viewModel: viewModel)
            present(
                view,
                size: RecordingReviewView.preferredSize,
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
        CGSize(width: 520, height: 128)
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
        hideToolbar(animated: false)
        // Invoked from the menu bar: give the menu's own dismissal fade time to finish
        // before the displays are frozen.
        beginUnifiedScreenshotCapture(anchorRect: nil, snapshotDelay: 0.2)
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
        hideToolbar(animated: false)
        guard ScreenCapturePermission.isTrusted else {
            showScreenshotChooser(anchorRect: nil, initialMode: screenshotCaptureMode(from: settings.screenshotDefaultMode))
            return
        }
        beginUnifiedScreenshotCapture(anchorRect: nil)
    }

    private func beginUnifiedScreenshotCapture(
        anchorRect: CGRect?,
        policy: CaptureSelectionPolicy = .any,
        snapshotDelay: TimeInterval = CaptureOverlayController.defaultSnapshotDelay,
        scrollCaptureOnSelection: Bool = false
    ) {
#if DEBUG
        if let area = e2eRegionArea() {
            Task { await self.captureArea(area, anchorRect: anchorRect) }
            return
        }
#endif
        hidePopup(animated: false)
        captureOverlayController?.cancel()
        let controller = CaptureOverlayController(
            screenCaptureService: screenCaptureService,
            settings: settings,
            macOCRService: macOCRService
        )
        captureOverlayController = controller
        controller.onScrollCaptureFinished = { [weak self] document in
            self?.captureOverlayController = nil
            self?.showScreenshotEditor(document: document, anchorRect: anchorRect)
        }
        controller.beginScreenshot(
            policy: policy,
            snapshotDelay: snapshotDelay,
            scrollCaptureOnSelection: scrollCaptureOnSelection,
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
        viewModel.onCaptureWindow = { [weak self] in self?.beginWindowCapture(anchorRect: anchorRect) }
        viewModel.onCaptureScreen = { [weak self] in self?.beginScreenCapture(anchorRect: anchorRect) }
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
        beginUnifiedScreenshotCapture(anchorRect: anchorRect, policy: .areaOnly)
    }

    private func beginWindowCapture(anchorRect: CGRect?) {
        beginUnifiedScreenshotCapture(anchorRect: anchorRect, policy: .windowOnly)
    }

    private func beginScreenCapture(anchorRect: CGRect?) {
        beginUnifiedScreenshotCapture(anchorRect: anchorRect, policy: .displayOnly)
    }

    private func beginScrollingAreaCapture(anchorRect: CGRect?) {
        beginUnifiedScreenshotCapture(anchorRect: anchorRect, policy: .areaOrWindow, scrollCaptureOnSelection: true)
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
        viewModel.onCaptureWindow = { [weak self] in self?.beginWindowCapture(anchorRect: anchorRect) }
        viewModel.onCaptureScreen = { [weak self] in self?.beginScreenCapture(anchorRect: anchorRect) }
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
        let env = ProcessInfo.processInfo.environment
        if let path = env["BELLOBOX_E2E_RECORDING_REVIEW_FILE"] {
            handleRecordingState(.reviewing(URL(fileURLWithPath: path), warning: env["BELLOBOX_E2E_RECORDING_REVIEW_WARNING"]))
            return
        }
        if let text = env["BELLOBOX_E2E_QR_TEXT"] {
            showQRPopup(for: TextSelection(text: text, anchorRect: nil, appName: nil, bundleID: nil, pid: nil))
            return
        }
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
            var markerLines: [String] = ["kind=real-screenshot"]
            do {
                guard ScreenCapturePermission.isTrusted else {
                    throw ScreenCaptureService.CaptureError.permissionDenied
                }
                let screens = NSScreen.screens
                guard !screens.isEmpty else {
                    throw ScreenCaptureService.CaptureError.noDisplayFound
                }

                markerLines += [
                    "status=success",
                    "displayCount=\(screens.count)",
                ]
                if let virtualDisplayStatus = Self.e2eVirtualDisplayStatusIfRequested() {
                    markerLines.append("virtualDisplay=\(virtualDisplayStatus)")
                }
                var primaryOutputPath = outputPath

                for (index, screen) in screens.enumerated() {
                    guard let displayID = ScreenCoordinateSpace.displayID(for: screen),
                          let rect = e2eCaptureRect(on: screen, defaultSize: CGSize(width: 320, height: 200))
                    else {
                        throw ScreenCaptureService.CaptureError.noDisplayFound
                    }

                    let expectedColor = Self.e2eScreenshotColor(for: index)
                    showE2EScreenshotPulseWindow(in: rect, color: expectedColor)
                    defer { hideE2EScreenshotPulseWindow() }
                    try await Task.sleep(nanoseconds: 200_000_000)

                    let document = try await screenCaptureService.capture(
                        .area(CaptureArea(cocoaRect: rect, displayID: displayID)),
                        options: CaptureOptions(includeCursor: false, hideBelloBoxWindows: false, delayAfterHidingOverlays: 0)
                    )
                    let rendered = try AnnotationRenderer.render(document)
                    let displayOutputPath = Self.e2eDisplayOutputPath(basePath: outputPath, displayIndex: index, displayID: displayID)
                    if index == 0 {
                        primaryOutputPath = displayOutputPath
                    }
                    try Self.writePNG(rendered, to: displayOutputPath)

                    let expected = Self.e2eExpectedImageRect(for: rect, on: screen, displayID: displayID)
                    let dimensionsMatch = rendered.width == Int(expected.width) && rendered.height == Int(expected.height)
                    let expectedSample = RGBColorSample(expectedColor)
                    let averageSample = ImageColorAnalyzer.averageColor(rendered)
                    let colorDistance = averageSample?.distance(to: expectedSample) ?? .infinity
                    let contentMatches = colorDistance <= 0.25

                    markerLines += [
                        "display[\(index)].status=success",
                        "display[\(index)].displayID=\(displayID)",
                        "display[\(index)].screenFrame=\(Self.serialize(screen.frame))",
                        "display[\(index)].rect=\(Self.serialize(rect))",
                        "display[\(index)].scale=\(document.scale)",
                        "display[\(index)].expectedWidth=\(Int(expected.width))",
                        "display[\(index)].expectedHeight=\(Int(expected.height))",
                        "display[\(index)].imageWidth=\(rendered.width)",
                        "display[\(index)].imageHeight=\(rendered.height)",
                        "display[\(index)].dimensionMatches=\(dimensionsMatch)",
                        "display[\(index)].expectedColor=\(expectedSample.diagnosticString)",
                        "display[\(index)].averageColor=\(averageSample?.diagnosticString ?? "nil")",
                        "display[\(index)].colorDistance=\(Self.serialize(colorDistance))",
                        "display[\(index)].contentMatches=\(contentMatches)",
                        "display[\(index)].path=\(displayOutputPath)",
                        "display[\(index)].fileSize=\(Self.fileSize(at: displayOutputPath))",
                    ]

                    guard dimensionsMatch else {
                        throw ScreenCaptureService.CaptureError.captureFailed(
                            "Display \(displayID) produced \(rendered.width)x\(rendered.height), expected \(Int(expected.width))x\(Int(expected.height))."
                        )
                    }
                    guard contentMatches else {
                        throw ScreenCaptureService.CaptureError.captureFailed(
                            "Display \(displayID) content color did not match the E2E marker window."
                        )
                    }
                }

                Self.writeE2EMarker(
                    markerPath,
                    lines: markerLines + [
                        "path=\(primaryOutputPath)",
                        "fileSize=\(Self.fileSize(at: primaryOutputPath))",
                    ]
                )
            } catch {
                hideE2EScreenshotPulseWindow()
                let linesWithoutStatus = markerLines.filter { !$0.hasPrefix("status=") }
                Self.writeE2EMarker(
                    markerPath,
                    lines: linesWithoutStatus + [
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
        let duration = max(0.6, min(20, Double(env["BELLOBOX_E2E_RECORDING_DURATION"] ?? "") ?? 1.2))
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
                    clickOverlayMode: env["BELLOBOX_E2E_RECORDING_CLICKS"].flatMap(ClickOverlayMode.init(rawValue:)) ?? .off,
                    keystrokeMode: env["BELLOBOX_E2E_RECORDING_KEYS"].flatMap(KeystrokeCaptureMode.init(rawValue:)) ?? .off,
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
                if env["BELLOBOX_E2E_RECORDING_DISABLE_INPUT_HALFWAY"] == "1" {
                    try await Task.sleep(nanoseconds: UInt64(duration * 500_000_000))
                    _ = engine.updateInputOverlays(clicks: .off, keys: .off)
                    try await Task.sleep(nanoseconds: UInt64(duration * 500_000_000))
                } else {
                    try await Task.sleep(nanoseconds: UInt64(duration * 1_000_000_000))
                }
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
                        "inputOverlaysStarted=\(runtime.isInputOverlayEnabled)",
                        "diagnostics=\(engine.diagnosticsSummary)",
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

    private func showE2EScreenshotPulseWindow(in rect: CGRect, color: NSColor) {
        hideE2EScreenshotPulseWindow()
        let window = NSWindow(
            contentRect: rect,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.level = .screenSaver
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        window.ignoresMouseEvents = true
        window.isOpaque = true
        window.backgroundColor = color
        window.hasShadow = false
        window.isReleasedWhenClosed = false
        window.contentView = E2ESolidColorView(color: color, frame: CGRect(origin: .zero, size: rect.size))
        window.orderFrontRegardless()
        window.displayIfNeeded()
        e2eScreenshotPulseWindow = window
    }

    private func hideE2EScreenshotPulseWindow() {
        e2eScreenshotPulseWindow?.orderOut(nil)
        e2eScreenshotPulseWindow = nil
    }

    private static func e2eScreenshotColor(for index: Int) -> NSColor {
        let colors = [
            NSColor(calibratedRed: 0.92, green: 0.12, blue: 0.18, alpha: 1),
            NSColor(calibratedRed: 0.10, green: 0.70, blue: 0.28, alpha: 1),
            NSColor(calibratedRed: 0.08, green: 0.32, blue: 0.92, alpha: 1),
            NSColor(calibratedRed: 0.90, green: 0.62, blue: 0.08, alpha: 1),
        ]
        return colors[index % colors.count]
    }

    private static func e2eVirtualDisplayStatusIfRequested() -> String? {
        guard ProcessInfo.processInfo.environment["BELLOBOX_E2E_VIRTUAL_DISPLAY"] == "1" else { return nil }
        let classNames = ["CGVirtualDisplay", "CGVirtualDisplayDescriptor", "CGVirtualDisplaySettings", "CGVirtualDisplayMode"]
        let missing = classNames.filter { NSClassFromString($0) == nil }
        if missing.isEmpty {
            let reason = "skipped(classesPresentPrivateAPIUnstable)"
            NSLog("Bello Box E2E virtual display \(reason)")
            return reason
        }
        let reason = "skipped(missing:\(missing.joined(separator: ",")))"
        NSLog("Bello Box E2E virtual display \(reason)")
        return reason
    }

    private func runScreenshotE2EHooksIfNeeded() {
        let env = ProcessInfo.processInfo.environment
        if let path = env["BELLOBOX_E2E_FROZEN_OVERLAY_MARKER"], !path.isEmpty {
            openE2EFrozenScreenOverlay(markerPath: path)
            return
        }
        if let path = env["BELLOBOX_E2E_SCROLL_CAPTURE_MARKER"], !path.isEmpty {
            openE2EScrollCapture(markerPath: path)
            return
        }
        if env["BELLOBOX_E2E_CAPTURE_OVERLAY_SIMULATED_DISPLAYS"] == "1" {
            openE2ESimulatedMultiDisplayCaptureOverlay()
            return
        }
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

    /// Drives the real snapshot-first screenshot overlay: a solid-color window per display
    /// must end up inside the frozen snapshot, and no overlay window may exist before the
    /// snapshots are taken (that ordering is what preserves hover states).
    private func openE2EFrozenScreenOverlay(markerPath: String) {
        Task { @MainActor in
            var markerLines: [String] = ["kind=frozen-overlay"]
            var pulseWindows: [NSWindow] = []
            do {
                guard ScreenCapturePermission.isTrusted else {
                    throw ScreenCaptureService.CaptureError.permissionDenied
                }
                let screens = NSScreen.screens
                guard !screens.isEmpty else {
                    throw ScreenCaptureService.CaptureError.noDisplayFound
                }

                var expectations: [(screen: NSScreen, displayID: CGDirectDisplayID, rect: CGRect, color: NSColor)] = []
                for (index, screen) in screens.enumerated() {
                    guard let displayID = ScreenCoordinateSpace.displayID(for: screen),
                          let rect = e2eCaptureRect(on: screen, defaultSize: CGSize(width: 320, height: 200))
                    else {
                        throw ScreenCaptureService.CaptureError.noDisplayFound
                    }
                    let color = Self.e2eScreenshotColor(for: index)
                    pulseWindows.append(Self.makeE2EPulseWindow(in: rect, color: color))
                    expectations.append((screen, displayID, rect, color))
                }
                try await Task.sleep(nanoseconds: 200_000_000)

                hidePopup()
                captureOverlayController?.cancel()
                let controller = CaptureOverlayController(
                    screenCaptureService: screenCaptureService,
                    settings: settings,
                    macOCRService: macOCRService
                )
                captureOverlayController = controller
                let overlayError = E2EErrorBox()
                let phaseBox = E2ESnapshotPhaseBox()
                controller.debugSnapshotPhaseObserver = { windowCount, snapshotCount in
                    phaseBox.windowCountWhenSnapshotsCompleted = windowCount
                    phaseBox.snapshotCountWhenCompleted = snapshotCount
                }
                let startedAt = Date()
                controller.beginScreenshot(
                    policy: .areaOnly,
                    onError: { overlayError.message = $0 },
                    onCancel: {}
                )
                let windowCountBeforeSnapshots = controller.debugOverlayWindowCount

                let deadline = Date().addingTimeInterval(15)
                while controller.debugOverlayWindowCount == 0, overlayError.message == nil, Date() < deadline {
                    try await Task.sleep(nanoseconds: 50_000_000)
                }
                if let message = overlayError.message {
                    throw ScreenCaptureService.CaptureError.captureFailed(message)
                }
                guard controller.debugOverlayWindowCount > 0 else {
                    throw ScreenCaptureService.CaptureError.captureFailed("The frozen-screen overlay never appeared.")
                }
                let readyAfterMs = Int(Date().timeIntervalSince(startedAt) * 1000)
                let snapshots = controller.debugSnapshots

                markerLines += [
                    "status=success",
                    "displayCount=\(screens.count)",
                    "windowCountBeforeSnapshots=\(windowCountBeforeSnapshots)",
                    "windowCountWhenSnapshotsCompleted=\(phaseBox.windowCountWhenSnapshotsCompleted.map(String.init) ?? "unobserved")",
                    "snapshotCountWhenCompleted=\(phaseBox.snapshotCountWhenCompleted.map(String.init) ?? "unobserved")",
                    "windowCount=\(controller.debugOverlayWindowCount)",
                    "snapshotCount=\(snapshots.count)",
                    "overlayViewsWithSnapshot=\(controller.debugOverlayViewsWithSnapshotCount)",
                    "readyAfterMs=\(readyAfterMs)",
                ]

                for (index, expectation) in expectations.enumerated() {
                    guard let snapshot = snapshots.first(where: { $0.displayID == expectation.displayID }) else {
                        markerLines += [
                            "snapshot[\(index)].status=missing",
                            "snapshot[\(index)].displayID=\(expectation.displayID)",
                        ]
                        continue
                    }
                    let document = try screenCaptureService.displayDocument(
                        fromSnapshot: snapshot,
                        selectionCocoaRect: expectation.rect,
                        source: .area(rect: expectation.rect, displayID: expectation.displayID)
                    )
                    let rendered = try AnnotationRenderer.render(document)
                    let expected = Self.e2eExpectedImageRect(for: expectation.rect, on: expectation.screen, displayID: expectation.displayID)
                    let dimensionMatches = rendered.width == Int(expected.width) && rendered.height == Int(expected.height)
                    let expectedSample = RGBColorSample(expectation.color)
                    let averageSample = ImageColorAnalyzer.averageColor(rendered)
                    let colorDistance = averageSample?.distance(to: expectedSample) ?? .infinity
                    let contentMatches = colorDistance <= 0.25
                    markerLines += [
                        "snapshot[\(index)].status=success",
                        "snapshot[\(index)].displayID=\(expectation.displayID)",
                        "snapshot[\(index)].imageWidth=\(snapshot.image.width)",
                        "snapshot[\(index)].imageHeight=\(snapshot.image.height)",
                        "snapshot[\(index)].scale=\(snapshot.scale)",
                        "snapshot[\(index)].cropWidth=\(rendered.width)",
                        "snapshot[\(index)].cropHeight=\(rendered.height)",
                        "snapshot[\(index)].expectedWidth=\(Int(expected.width))",
                        "snapshot[\(index)].expectedHeight=\(Int(expected.height))",
                        "snapshot[\(index)].dimensionMatches=\(dimensionMatches)",
                        "snapshot[\(index)].expectedColor=\(expectedSample.diagnosticString)",
                        "snapshot[\(index)].averageColor=\(averageSample?.diagnosticString ?? "nil")",
                        "snapshot[\(index)].colorDistance=\(Self.serialize(colorDistance))",
                        "snapshot[\(index)].contentMatches=\(contentMatches)",
                    ]
                }
                controller.cancel()
                captureOverlayController = nil
            } catch {
                captureOverlayController?.cancel()
                captureOverlayController = nil
                let linesWithoutStatus = markerLines.filter { !$0.hasPrefix("status=") }
                markerLines = linesWithoutStatus + [
                    "status=failure",
                    "error=\(error.localizedDescription)",
                ]
            }
            for window in pulseWindows {
                window.orderOut(nil)
            }
            Self.writeE2EMarker(markerPath, lines: markerLines)
            e2eQuitIfRequested()
        }
    }

    /// Drives scroll-to-capture three ways against a tall, uniquely textured document in
    /// Bello Box's own scroll view: programmatic scrolling through the engine, the engine's
    /// auto-scroll (synthetic wheel events), and the full overlay path (frozen snapshot →
    /// editor → scroll mode). Each result must match the scrolled range and keep the
    /// coloured bands in order.
    private func openE2EScrollCapture(markerPath: String) {
        Task { @MainActor in
            var markerLines: [String] = ["kind=scroll-capture"]
            var fixture: E2EScrollFixtureWindow?
            do {
                guard ScreenCapturePermission.isTrusted else {
                    throw ScreenCaptureService.CaptureError.permissionDenied
                }
                guard let screen = NSScreen.main,
                      let displayID = ScreenCoordinateSpace.displayID(for: screen)
                else {
                    throw ScreenCaptureService.CaptureError.noDisplayFound
                }
                let bandHeight: CGFloat = 100
                let documentHeight: CGFloat = 1500
                let viewport = CGSize(width: 360, height: 300)
                let rect = CGRect(
                    x: (screen.frame.minX + 160).rounded(),
                    y: (screen.frame.minY + 160).rounded(),
                    width: viewport.width,
                    height: viewport.height
                )
                let window = E2EScrollFixtureWindow(frame: rect, documentHeight: documentHeight, bandHeight: bandHeight)
                fixture = window
                try await Task.sleep(nanoseconds: 400_000_000)

                let scale = ScreenCoordinateSpace.backingScale(for: screen)
                let pixelSize = CGSize(width: (rect.width * scale).rounded(), height: (rect.height * scale).rounded())
                let area = CaptureArea(cocoaRect: rect, displayID: displayID)
                let summary = ScrollCaptureTargetSummary(title: "E2E", ownerName: nil, frame: CGRectCodable(rect))
                let bandPixels = bandHeight * scale
                hidePopup()

                // 1. Programmatic scrolling, as if the user scrolled in four steps.
                let manualInitial = try await screenCaptureService.captureRegionImage(area, pixelSize: pixelSize)
                let manual = ScrollCaptureEngine(
                    area: area, summary: summary, initialFrame: manualInitial, pixelSize: pixelSize,
                    service: screenCaptureService, settings: settings
                )
                manual.start()
                // 150 pt steps keep the visible content overlap above the stitcher's
                // minimum even on a 1x display with the footer bar excluded.
                for step in 1...4 {
                    window.scroll(toTopOffset: CGFloat(step) * 150)
                    await Self.e2eWaitForFrames(manual, count: step + 1)
                }
                let manualDocument = try await manual.finish()
                let manualExpected = Int(((viewport.height + 600) * scale).rounded())
                markerLines += Self.e2eScrollResultLines(
                    prefix: "manual", document: manualDocument, frames: manual.frames.count,
                    expectedHeight: manualExpected, bandPixels: bandPixels, bandColors: window.bandColors
                )

                // 2. Auto-scroll from the top until the end of the document.
                window.scroll(toTopOffset: 0)
                try await Task.sleep(nanoseconds: 500_000_000)
                let autoInitial = try await screenCaptureService.captureRegionImage(area, pixelSize: pixelSize)
                let auto = ScrollCaptureEngine(
                    area: area, summary: summary, initialFrame: autoInitial, pixelSize: pixelSize,
                    service: screenCaptureService, settings: settings
                )
                auto.start()
                auto.toggleAutoScroll()
                let autoStarted = Date()
                let autoDeadline = autoStarted.addingTimeInterval(90)
                while auto.isAutoScrolling, Date() < autoDeadline {
                    try await Task.sleep(nanoseconds: 100_000_000)
                }
                markerLines.append("auto.durationMs=\(Int(Date().timeIntervalSince(autoStarted) * 1000))")
                let autoDocument = try await auto.finish()
                markerLines += Self.e2eScrollResultLines(
                    prefix: "auto", document: autoDocument, frames: auto.frames.count,
                    expectedHeight: Int((documentHeight * scale).rounded()), bandPixels: bandPixels, bandColors: window.bandColors
                )
                markerLines.append("auto.reachedEnd=\(auto.reachedEnd)")
                markerLines.append("auto.finalOffset=\(Int(window.topOffset))")
                markerLines.append("auto.events=\(auto.debugEvents.joined(separator: "|"))")
                markerLines.append("auto.placements=\(auto.debugLastPlacements.map { "y\($0.y)/o\($0.overlapWithPrevious)/t\($0.croppedTop)/b\($0.croppedBottom)" }.joined(separator: "|"))")
                markerLines.append("manual.placements=\(manual.debugLastPlacements.map { "y\($0.y)/o\($0.overlapWithPrevious)/t\($0.croppedTop)/b\($0.croppedBottom)" }.joined(separator: "|"))")

                // 3. The real overlay: frozen snapshot → editor → scroll mode → programmatic scroll → Done.
                window.scroll(toTopOffset: 0)
                try await Task.sleep(nanoseconds: 500_000_000)
                let snapshots = try await screenCaptureService.captureDisplaySnapshots(
                    options: CaptureOptions(includeCursor: false, hideBelloBoxWindows: false, delayAfterHidingOverlays: 0)
                )
                let controller = CaptureOverlayController(
                    screenCaptureService: screenCaptureService, settings: settings, macOCRService: macOCRService
                )
                captureOverlayController?.cancel()
                captureOverlayController = controller
                let finishedBox = E2EScrollFinishedBox()
                controller.onScrollCaptureFinished = { document in finishedBox.document = document }
                controller.beginScreenshotForTesting(
                    snapshots: snapshots,
                    initialSelection: .area(area),
                    policy: .areaOnly,
                    scrollCaptureOnSelection: false,
                    onError: { finishedBox.error = $0 }
                )
                let previewDirectory = ProcessInfo.processInfo.environment["BELLOBOX_E2E_UI_PREVIEW_DIR"]
                if let previewDirectory, !previewDirectory.isEmpty {
                    try await Task.sleep(nanoseconds: 800_000_000)
                    OverlayTooltipPresenter.shared.showImmediately(
                        "Scroll to capture more: keep scrolling (or auto-scroll) and stitch the frames into one tall screenshot",
                        at: CGPoint(x: rect.midX + 120, y: rect.maxY + 40)
                    )
                    try await Task.sleep(nanoseconds: 200_000_000)
                    try await writeE2EUIPreview(named: "editor-toolbar", around: rect, screen: screen, directory: previewDirectory)
                    OverlayTooltipPresenter.shared.hide()
                    markerLines.append("ui.editorToolbarPreview=\(previewDirectory)/editor-toolbar.png")
                }
                controller.beginScrollCapture()
                guard let overlayEngine = controller.debugScrollCaptureEngine else {
                    throw ScreenCaptureService.CaptureError.captureFailed("The overlay did not enter scroll-to-capture mode.")
                }
                markerLines.append("overlay.hudVisible=\(controller.debugScrollCaptureHUDVisible)")
                markerLines.append("overlay.windowsIgnoreMouse=\(controller.debugOverlayWindows.allSatisfy(\.ignoresMouseEvents))")
                for step in 1...3 {
                    window.scroll(toTopOffset: CGFloat(step) * 150)
                    await Self.e2eWaitForFrames(overlayEngine, count: step + 1)
                    if step == 2, let previewDirectory, !previewDirectory.isEmpty {
                        try await Task.sleep(nanoseconds: 300_000_000)
                        // The compact card is shown above the selection (where the hidden
                        // toolbar was) so one screenshot shows both HUD layouts.
                        let compactDemo = Self.e2eCompactHUDDemoPanel(engine: overlayEngine, above: rect)
                        OverlayTooltipPresenter.shared.showImmediately(
                            "Scroll the content automatically until it ends, capturing as it goes",
                            at: CGPoint(x: rect.maxX + 330, y: rect.minY - 120)
                        )
                        try await Task.sleep(nanoseconds: 300_000_000)
                        try await writeE2EUIPreview(named: "scroll-hud", around: rect, screen: screen, directory: previewDirectory)
                        OverlayTooltipPresenter.shared.hide()
                        compactDemo.orderOut(nil)
                        markerLines.append("ui.scrollHUDPreview=\(previewDirectory)/scroll-hud.png")
                    }
                }
                markerLines.append("overlay.liveContentSampled=\(overlayEngine.frames.count > 1)")
                controller.finishScrollCapture()
                let overlayDeadline = Date().addingTimeInterval(20)
                while finishedBox.document == nil, finishedBox.error == nil, Date() < overlayDeadline {
                    try await Task.sleep(nanoseconds: 100_000_000)
                }
                if let error = finishedBox.error {
                    throw ScreenCaptureService.CaptureError.captureFailed(error)
                }
                guard let overlayDocument = finishedBox.document else {
                    throw ScreenCaptureService.CaptureError.captureFailed("The overlay scroll capture never finished.")
                }
                hidePopup()
                captureOverlayController = nil
                markerLines += Self.e2eScrollResultLines(
                    prefix: "overlay", document: overlayDocument, frames: overlayDocument.source.scrollingFrameCount,
                    expectedHeight: Int(((viewport.height + 450) * scale).rounded()), bandPixels: bandPixels, bandColors: window.bandColors
                )
                markerLines.append("status=success")
            } catch {
                captureOverlayController?.cancel()
                captureOverlayController = nil
                markerLines += ["status=failure", "error=\(error.localizedDescription)"]
            }
            fixture?.orderOut(nil)
            Self.writeE2EMarker(markerPath, lines: markerLines)
            e2eQuitIfRequested()
        }
    }

    /// Captures the display around `rect` (selection plus toolbar/HUD space) and writes it
    /// as a PNG so the overlay UI can be inspected.
    /// Shows the compact HUD card (as used when the full card has no room) above `rect`,
    /// bound to the live engine, for the UI preview screenshot.
    private static func e2eCompactHUDDemoPanel(engine: ScrollCaptureEngine, above rect: CGRect) -> NSPanel {
        let panel = ScrollCaptureHUDPanel()
        let padding = ScrollCaptureHUDView.outerPadding
        let size = ScrollCaptureHUDView.compactSize.applying(padding: padding)
        panel.contentView = NSHostingView(
            rootView: ScrollCaptureHUDView(engine: engine, layout: .compact, onDone: {}, onCancel: {})
        )
        panel.setFrame(CGRect(x: rect.minX - padding, y: rect.maxY + 16 - padding, width: size.width, height: size.height), display: true)
        panel.orderFrontRegardless()
        return panel
    }

    private func writeE2EUIPreview(named name: String, around rect: CGRect, screen: NSScreen, directory: String) async throws {
        let region = rect.insetBy(dx: -700, dy: -220).intersection(screen.frame)
        guard let displayID = ScreenCoordinateSpace.displayID(for: screen) else { return }
        let scale = ScreenCoordinateSpace.backingScale(for: screen)
        let image = try await screenCaptureService.captureRegionImage(
            CaptureArea(cocoaRect: region, displayID: displayID),
            pixelSize: CGSize(width: (region.width * scale).rounded(), height: (region.height * scale).rounded())
        )
        try Self.writePNG(image, to: "\(directory)/\(name).png")
    }

    /// Waits until the engine has appended `count` frames (or gives up after a while).
    private static func e2eWaitForFrames(_ engine: ScrollCaptureEngine, count: Int, timeout: TimeInterval = 12) async {
        let deadline = Date().addingTimeInterval(timeout)
        while engine.frames.count < count, Date() < deadline {
            try? await Task.sleep(nanoseconds: 100_000_000)
        }
    }

    private static func e2eScrollResultLines(
        prefix: String,
        document: ScreenshotDocument,
        frames: Int,
        expectedHeight: Int,
        bandPixels: CGFloat,
        bandColors: [NSColor]
    ) -> [String] {
        let image = document.baseImage
        let heightMatches = abs(image.height - expectedHeight) <= max(8, Int(Double(expectedHeight) * 0.06))
        let bandsInOrder = e2eBandsInOrder(image, bandPixels: bandPixels, colors: bandColors)
        return [
            "\(prefix).frames=\(frames)",
            "\(prefix).width=\(image.width)",
            "\(prefix).height=\(image.height)",
            "\(prefix).expectedHeight=\(expectedHeight)",
            "\(prefix).heightMatches=\(heightMatches)",
            "\(prefix).bandsInOrder=\(bandsInOrder)",
        ]
    }

    /// Every band centre inside the stitched image must show that band's colour.
    private static func e2eBandsInOrder(_ image: CGImage, bandPixels: CGFloat, colors: [NSColor]) -> Bool {
        guard bandPixels > 0, image.height > 0 else { return false }
        let bandCount = Int((CGFloat(image.height) / bandPixels).rounded(.down))
        guard bandCount >= 2 else { return false }
        let x = max(1, image.width / 5)
        for band in 0..<min(bandCount, colors.count) {
            let y = Int(CGFloat(band) * bandPixels + bandPixels / 2)
            guard y < image.height else { break }
            let sample = e2ePixel(image, x: x, y: y)
            let expected = RGBColorSample(colors[band])
            if sample.distance(to: expected) > 0.25 { return false }
        }
        return true
    }

    private static func e2ePixel(_ image: CGImage, x: Int, y: Int) -> RGBColorSample {
        var data = [UInt8](repeating: 0, count: 4)
        guard let context = CGContext(
            data: &data, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return RGBColorSample(red: 0, green: 0, blue: 0) }
        context.translateBy(x: CGFloat(-x), y: CGFloat(y - image.height + 1))
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return RGBColorSample(red: Double(data[0]) / 255, green: Double(data[1]) / 255, blue: Double(data[2]) / 255)
    }

    private static func makeE2EPulseWindow(in rect: CGRect, color: NSColor) -> NSWindow {
        let window = NSWindow(
            contentRect: rect,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.level = .screenSaver
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        window.ignoresMouseEvents = true
        window.isOpaque = true
        window.backgroundColor = color
        window.hasShadow = false
        window.isReleasedWhenClosed = false
        window.contentView = E2ESolidColorView(color: color, frame: CGRect(origin: .zero, size: rect.size))
        window.orderFrontRegardless()
        window.displayIfNeeded()
        return window
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

    private func openE2ESimulatedMultiDisplayCaptureOverlay() {
        hidePopup()
        let primary = DisplaySnapshot(
            displayID: 9_001,
            screenFrame: CGRect(x: 0, y: 0, width: 360, height: 240),
            scale: 1,
            image: Self.e2eSolidImage(width: 360, height: 240, color: NSColor(calibratedRed: 0.78, green: 0.12, blue: 0.10, alpha: 1))
        )
        let secondaryLeft = DisplaySnapshot(
            displayID: 9_002,
            screenFrame: CGRect(x: -320, y: 0, width: 320, height: 220),
            scale: 2,
            image: Self.e2eSolidImage(width: 640, height: 440, color: NSColor(calibratedRed: 0.10, green: 0.70, blue: 0.28, alpha: 1))
        )
        let upper = DisplaySnapshot(
            displayID: 9_003,
            screenFrame: CGRect(x: 0, y: 240, width: 260, height: 180),
            scale: 1.5,
            image: Self.e2eSolidImage(width: 390, height: 270, color: NSColor(calibratedRed: 0.10, green: 0.24, blue: 0.78, alpha: 1))
        )
        let selectionRect = CGRect(x: -260, y: 70, width: 120, height: 70)
        let initialSelection = CaptureSelection.area(CaptureArea(cocoaRect: selectionRect, displayID: secondaryLeft.displayID))
        let controller = CaptureOverlayController(
            screenCaptureService: screenCaptureService,
            settings: settings,
            macOCRService: macOCRService
        )
        captureOverlayController?.cancel()
        captureOverlayController = controller
        controller.beginScreenshotForTesting(
            snapshots: [primary, secondaryLeft, upper],
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

    private static func e2eSolidImage(width: Int, height: Int, color: NSColor) -> CGImage {
        let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(color.cgColor)
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        return context.makeImage()!
    }

    private func openE2EScrollingFrames(path: String) async {
        let url = URL(fileURLWithPath: path)
        let files = (try? FileManager.default.contentsOfDirectory(at: url, includingPropertiesForKeys: nil)) ?? []
        let images = files
            .filter { ["png", "jpg", "jpeg"].contains($0.pathExtension.lowercased()) }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
            .compactMap { Self.cgImage(at: $0.path) }
        guard !images.isEmpty, let result = try? ImageStitcher.stitch(images) else { return }
        let document = ScrollCaptureEngine.makeDocument(
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

    private static func e2eDisplayOutputPath(
        basePath: String,
        displayIndex: Int,
        displayID: CGDirectDisplayID
    ) -> String {
        guard displayIndex > 0 else { return basePath }
        let url = URL(fileURLWithPath: basePath)
        let baseName = url.deletingPathExtension().lastPathComponent
        let ext = url.pathExtension.isEmpty ? "png" : url.pathExtension
        return url
            .deletingLastPathComponent()
            .appendingPathComponent("\(baseName)-display-\(displayIndex)-\(displayID)")
            .appendingPathExtension(ext)
            .path
    }

    private static func e2eExpectedImageRect(
        for rect: CGRect,
        on screen: NSScreen,
        displayID: CGDirectDisplayID
    ) -> CGRect {
        let pixelSize = ScreenCoordinateSpace.displayPixelSize(for: displayID, fallbackScreen: screen)
        return ScreenCoordinateSpace.cocoaRectToImagePixelRect(
            rect,
            screenFrame: screen.frame,
            imageSize: pixelSize
        ).integral
    }

    private func e2eCaptureRect(on screen: NSScreen, defaultSize: CGSize) -> CGRect? {
        if let raw = ProcessInfo.processInfo.environment["BELLOBOX_E2E_CAPTURE_RECT"], !raw.isEmpty {
            let parts = raw.split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespacesAndNewlines)) }
            guard parts.count == 4 else { return nil }
            let rect = CGRect(x: CGFloat(parts[0]), y: CGFloat(parts[1]), width: CGFloat(parts[2]), height: CGFloat(parts[3]))
                .intersection(screen.frame)
                .standardized
            guard rect.width >= 1, rect.height >= 1 else { return nil }
            return rect
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

    private static func serialize(_ value: Double) -> String {
        String(format: "%.4f", value)
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
        OverlayTooltipPresenter.shared.hide()
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

    private func hidePopup(runDismissAction: Bool = true, animated: Bool = true) {
        let onDismiss = runDismissAction ? popupOnDismiss : nil
        popupOnDismiss = nil
        OverlayTooltipPresenter.shared.hide()
        if !animated {
            // Vanish immediately so a capture started right after cannot see the fade.
            popupPanel?.animationBehavior = .none
        }
        popupPanel?.orderOut(nil)
        popupPanel = nil
        hideScreenshotOverlayEditor()
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
private final class E2EErrorBox {
    var message: String?
}

private final class E2EScrollFinishedBox {
    var document: ScreenshotDocument?
    var error: String?
}

/// A borderless window with a scroll view over a tall document made of coloured bands,
/// each carrying its number and a ruler of ticks whose lengths vary row by row, so the
/// stitcher can only match frames at their true offset.
private final class E2EScrollFixtureWindow: NSWindow {
    let bandColors: [NSColor]
    private let scrollView = NSScrollView()

    init(frame: CGRect, documentHeight: CGFloat, bandHeight: CGFloat) {
        let bandCount = Int((documentHeight / bandHeight).rounded(.up))
        bandColors = (0..<bandCount).map { index in
            NSColor(calibratedHue: CGFloat((index * 5) % 12) / 12, saturation: 0.75, brightness: 0.85, alpha: 1)
        }
        super.init(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
        level = .floating
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        isOpaque = true
        backgroundColor = .white
        hasShadow = false
        isReleasedWhenClosed = false

        let document = E2EScrollFixtureDocumentView(
            frame: CGRect(x: 0, y: 0, width: frame.width, height: documentHeight),
            bandHeight: bandHeight,
            colors: bandColors
        )
        scrollView.frame = CGRect(origin: .zero, size: frame.size)
        scrollView.documentView = document
        scrollView.hasVerticalScroller = false
        scrollView.hasHorizontalScroller = false
        scrollView.verticalScrollElasticity = .none
        scrollView.horizontalScrollElasticity = .none
        scrollView.drawsBackground = true
        let container = NSView(frame: CGRect(origin: .zero, size: frame.size))
        container.addSubview(scrollView)
        // A fixed bar over the bottom, like a status or input bar, which the stitcher
        // must keep only once at the end and see through for the content under it.
        let footer = E2ESolidColorView(
            color: NSColor(calibratedRed: 0.10, green: 0.10, blue: 0.12, alpha: 1),
            frame: CGRect(x: 0, y: 0, width: frame.width, height: Self.footerHeight)
        )
        container.addSubview(footer)
        contentView = container
        scroll(toTopOffset: 0)
        orderFrontRegardless()
        displayIfNeeded()
    }

    static let footerHeight: CGFloat = 36

    func scroll(toTopOffset offset: CGFloat) {
        scrollView.contentView.scroll(to: NSPoint(x: 0, y: offset))
        scrollView.reflectScrolledClipView(scrollView.contentView)
        scrollView.displayIfNeeded()
        displayIfNeeded()
    }

    var topOffset: CGFloat { scrollView.contentView.bounds.origin.y }
}

private final class E2EScrollFixtureDocumentView: NSView {
    private let bandHeight: CGFloat
    private let colors: [NSColor]

    init(frame: CGRect, bandHeight: CGFloat, colors: [NSColor]) {
        self.bandHeight = bandHeight
        self.colors = colors
        super.init(frame: frame)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var isFlipped: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        for (index, color) in colors.enumerated() {
            let band = CGRect(x: 0, y: CGFloat(index) * bandHeight, width: bounds.width, height: bandHeight)
            color.setFill()
            band.fill()
            let label = "\(index + 1)" as NSString
            let attributes: [NSAttributedString.Key: Any] = [
                .font: NSFont.systemFont(ofSize: 44, weight: .bold),
                .foregroundColor: NSColor.black.withAlphaComponent(0.55),
            ]
            let size = label.size(withAttributes: attributes)
            label.draw(at: CGPoint(x: band.midX - size.width / 2, y: band.midY - size.height / 2), withAttributes: attributes)
        }
        NSColor.black.withAlphaComponent(0.35).setFill()
        var y: CGFloat = 0
        var row = 0
        while y < bounds.height {
            let width = 14 + CGFloat((row * 7) % 11) * 6
            CGRect(x: 6, y: y, width: width, height: 1).fill()
            y += 8
            row += 1
        }
    }
}

private final class E2ESnapshotPhaseBox {
    var windowCountWhenSnapshotsCompleted: Int?
    var snapshotCountWhenCompleted: Int?
}

private final class E2ESolidColorView: NSView {
    private let color: NSColor

    init(color: NSColor, frame frameRect: NSRect) {
        self.color = color
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = color.cgColor
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func draw(_ dirtyRect: NSRect) {
        color.setFill()
        bounds.fill()
    }
}

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
