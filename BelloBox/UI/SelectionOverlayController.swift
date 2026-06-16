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
    private var popupFullContentView: NSView?
    private var popupFullSize: CGSize = .zero
    private var popupIsMinimized = false
    private var popupMinimizedIcon = ""
    private var popupMinimizedTitle = ""
    private var popupMinimizedSubtitle: (() -> String?)?
    private var toolbarDismissMonitor: Any?

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
            self?.triggerBoardOnCurrentSelection()
        }
    }

    func start() {
        lastTrusted = AccessibilityService.isTrusted
        applyMonitorSettings()
        monitor.start()
        // Keyboard monitoring only takes effect once the process is trusted, so
        // re-establish the monitors when Accessibility is granted while running.
        if !lastTrusted { startTrustWatcher() }
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

    private func applyMonitorSettings() {
        monitor.selectionMonitoringEnabled = settings.floatingButtonEnabled
        monitor.hotkeyEnabled = settings.globalHotkeyEnabled
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
        popupFullContentView = nil
        popupFullSize = .zero
        popupIsMinimized = false
        popupMinimizedIcon = ""
        popupMinimizedTitle = ""
        popupMinimizedSubtitle = nil
        pendingSelection = nil
    }
}
