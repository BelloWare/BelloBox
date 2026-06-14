import AppKit
import SwiftUI

/// Coordinates the whole selection → button → popup flow: listens for
/// selections, shows the floating button, and presents the AI popup.
@MainActor
final class SelectionOverlayController {
    private let settings: AppSettings
    private let accessibility = AccessibilityService()
    private let client = AIClient()
    private let monitor: SelectionMonitor

    private var buttonPanel: FloatingButtonPanel?
    private var popupPanel: PopupPanel?
    private var buttonDismissMonitor: Any?
    private var popupDismissMonitor: Any?

    private var pendingSelection: TextSelection?
    private var trustWatcher: Timer?
    private var lastTrusted = false

    /// Set by the app to open the Settings window.
    var openSettings: () -> Void = {}

    init(settings: AppSettings) {
        self.settings = settings
        self.monitor = SelectionMonitor(accessibility: accessibility)
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
        if !enabled { hideButton() }
    }

    private func startTrustWatcher() {
        trustWatcher?.invalidate()
        trustWatcher = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
            guard let self else { return }
            let trusted = AccessibilityService.isTrusted
            if trusted, !self.lastTrusted {
                self.lastTrusted = true
                self.restartMonitors()
                self.trustWatcher?.invalidate()
                self.trustWatcher = nil
            }
        }
    }

    // MARK: - Selection handling

    private func handleSelection(_ selection: TextSelection) {
        guard settings.floatingButtonEnabled else { return }
        guard popupPanel == nil else { return } // don't interrupt an open popup
        pendingSelection = selection
        showButton(for: selection)
    }

    /// Used by the hotkey / menu: read the selection now and jump straight to the
    /// popup, falling back to a synthesized copy when AX text is unavailable.
    func triggerOnCurrentSelection() {
        var selection = accessibility.readSelection()
        if selection == nil, let copied = accessibility.copySelectionViaPasteboard() {
            let front = NSWorkspace.shared.frontmostApplication
            selection = TextSelection(
                text: copied,
                anchorRect: nil,
                appName: front?.localizedName,
                bundleID: front?.bundleIdentifier,
                pid: front?.processIdentifier
            )
        }
        guard let selection, !selection.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            NSSound.beep()
            return
        }
        hideButton()
        showPopup(for: selection)
    }

    // MARK: - Floating button

    private func showButton(for selection: TextSelection) {
        hideButton()
        let size = FloatingButtonView.preferredSize
        let panelSize = CGSize(width: size.width + 8, height: size.height + 8)
        let origin = ScreenPlacement.buttonOrigin(
            anchorRect: selection.anchorRect,
            mouse: NSEvent.mouseLocation,
            size: panelSize
        )

        let panel = FloatingButtonPanel(contentRect: CGRect(origin: origin, size: panelSize))
        let view = FloatingButtonView { [weak self] in
            self?.activateButton()
        }
        panel.contentView = NSHostingView(rootView: view)
        panel.setFrameOrigin(origin)
        panel.orderFrontRegardless()
        buttonPanel = panel

        installButtonDismissMonitor()
    }

    private func activateButton() {
        guard let selection = pendingSelection else { return }
        hideButton()
        showPopup(for: selection)
    }

    private func hideButton() {
        if let monitor = buttonDismissMonitor {
            NSEvent.removeMonitor(monitor)
            buttonDismissMonitor = nil
        }
        buttonPanel?.orderOut(nil)
        buttonPanel = nil
    }

    private func installButtonDismissMonitor() {
        buttonDismissMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            self?.hideButton()
        }
    }

    // MARK: - Popup

    private func showPopup(for selection: TextSelection) {
        hidePopup()

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

        let size = ActionPopupView.preferredSize
        let origin = ScreenPlacement.popupOrigin(
            anchorRect: selection.anchorRect,
            mouse: NSEvent.mouseLocation,
            size: size
        )
        let panel = PopupPanel(contentRect: CGRect(origin: origin, size: size))
        panel.contentView = NSHostingView(rootView: ActionPopupView(viewModel: viewModel))
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
