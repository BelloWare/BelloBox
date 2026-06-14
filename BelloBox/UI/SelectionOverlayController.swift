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

    private var toolbarPanel: FloatingButtonPanel?
    private var popupPanel: PopupPanel?
    private var toolbarDismissMonitor: Any?
    private var popupDismissMonitor: Any?

    private var pendingSelection: TextSelection?
    private var trustWatcher: Timer?
    private var lastTrusted = false

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
            self?.triggerOnCurrentSelection()
        }
    }

    func start() {
        lastTrusted = AccessibilityService.isTrusted
        monitor.isEnabled = settings.floatingButtonEnabled
        monitor.start()
        // Keyboard monitoring only takes effect once the process is trusted, so
        // re-establish the monitors when Accessibility is granted while running.
        if !lastTrusted { startTrustWatcher() }
    }

    /// Tears down and re-installs the event monitors. Needed after Accessibility
    /// is granted so the global keyboard monitor actually receives events.
    func restartMonitors() {
        monitor.stop()
        monitor.isEnabled = settings.floatingButtonEnabled
        monitor.start()
    }

    func setFloatingButtonEnabled(_ enabled: Bool) {
        monitor.isEnabled = enabled
        if !enabled { hideToolbar() }
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

    /// Used by the menu: read the selection now and open the QR popup.
    func triggerQROnCurrentSelection() {
        guard let selection = nonEmpty(currentSelection()) else { NSSound.beep(); return }
        hideToolbar()
        showQRPopup(for: selection)
    }

    // MARK: - Floating toolbar

    private func showToolbar(for selection: TextSelection) {
        hideToolbar()

        let view = FloatingToolbarView(
            onAI: { [weak self] in self?.activateAI() },
            onQR: { [weak self] in self?.activateQR() }
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

        installToolbarDismissMonitor()
    }

    private func activateAI() {
        guard let selection = pendingSelection else { return }
        hideToolbar()
        showAIPopup(for: selection)
    }

    private func activateQR() {
        guard let selection = pendingSelection else { return }
        hideToolbar()
        showQRPopup(for: selection)
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
        present(ActionPopupView(viewModel: viewModel), size: ActionPopupView.preferredSize, anchorRect: selection.anchorRect)
    }

    private func showQRPopup(for selection: TextSelection) {
        let viewModel = QRCodePopupViewModel(text: selection.text)
        viewModel.onClose = { [weak self] in self?.hidePopup() }
        present(QRCodePopupView(viewModel: viewModel), size: QRCodePopupView.preferredSize, anchorRect: selection.anchorRect)
    }

    private func present<V: View>(_ view: V, size: CGSize, anchorRect: CGRect?) {
        hidePopup()
        let origin = ScreenPlacement.popupOrigin(
            anchorRect: anchorRect,
            mouse: NSEvent.mouseLocation,
            size: size
        )
        let panel = PopupPanel(contentRect: CGRect(origin: origin, size: size))
        panel.contentView = NSHostingView(rootView: view)
        panel.setFrameOrigin(origin)
        panel.makeKeyAndOrderFront(nil)
        popupPanel = panel

        installPopupDismissMonitor()
    }

    private func hidePopup() {
        if let monitor = popupDismissMonitor {
            NSEvent.removeMonitor(monitor)
            popupDismissMonitor = nil
        }
        popupPanel?.orderOut(nil)
        popupPanel = nil
        pendingSelection = nil
    }

    private func installPopupDismissMonitor() {
        // Clicks outside our app dismiss the popup (clicks inside are delivered
        // locally and never reach this global monitor).
        popupDismissMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            self?.hidePopup()
        }
    }
}
