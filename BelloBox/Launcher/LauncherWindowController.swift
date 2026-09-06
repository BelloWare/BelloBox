import AppKit
import SwiftUI

/// Takes keyboard focus without activating Bello Box over the source app.
final class LauncherPanel: NSPanel {
    init() {
        super.init(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel, .resizable], backing: .buffered, defer: false)
        title = "Bello Box"
        isFloatingPanel = true
        becomesKeyOnlyIfNeeded = false
        level = .floating
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        isMovableByWindowBackground = false
        animationBehavior = .none
    }
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

@MainActor
final class LauncherWindowController: NSObject, NSWindowDelegate {
    private(set) var panel: LauncherPanel?
    private var keyMonitor: Any?
    private var outsideMonitor: Any?
    private var localClickMonitor: Any?
    private var menuObservers: [NSObjectProtocol] = []
    private var trackingMenu = false
    private var model: LauncherModel?
    private weak var searchField: LauncherSearchTextField?
    private var presentationID = UUID()
#if DEBUG
    private let snippets = SnippetStore(url: ProcessInfo.processInfo.environment["BELLOBOX_E2E_SNIPPETS_PATH"].map { URL(fileURLWithPath: $0) })
#else
    private let snippets = SnippetStore()
#endif
    private var pinned: String?
    var isVisible: Bool { panel?.isVisible == true }
    var onCommand: (LauncherCommand, TextSelection) -> Void = { _, _ in }

    func show(selection: TextSelection, initialCommand: LauncherCommand? = nil) {
        close()
        let model = LauncherModel(selection: selection, snippets: snippets)
        model.onClose = { [weak self] in self?.close() }
        model.onCommand = { [weak self] command, selection in
            self?.close(); self?.onCommand(command, selection)
        }
        model.pinnedText = { [weak self] in self?.pinned }
        model.pinText = { [weak self] in self?.pinned = $0 }
        model.onPresentationChange = { [weak self] in self?.updatePresentation() }
        let panel = LauncherPanel()
        self.panel = panel; self.model = model
        panel.delegate = self
        panel.contentViewController = NSHostingController(rootView: LauncherView(model: model, onSearchReady: { [weak self] field in
            self?.searchField = field
        }))
        panel.setContentSize(model.paletteSize)
        panel.contentMinSize = model.paletteSize
        panel.contentMaxSize = model.paletteSize
        let screen = ScreenPlacement.screen(containing: NSEvent.mouseLocation)
        let size = panel.frame.size
        panel.setFrameOrigin(ScreenPlacement.clamp(origin: CGPoint(x: screen.visibleFrame.midX - size.width / 2,
            y: screen.visibleFrame.midY - size.height / 2 + 70), size: size, into: screen))
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            self?.handleKeyEvent(event) == true ? nil : event
        }
        outsideMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown]) { [weak self] _ in
            guard let self, self.panel?.attachedSheet == nil else { return }
            self.close(reason: "outsideClick")
        }
        localClickMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown]) { [weak self] event in
            guard let self, let panel = self.panel else { return event }
            if !self.trackingMenu, !Self.belongsToPanel(event.window, panel: panel), panel.attachedSheet == nil,
               !panel.frame.contains(NSEvent.mouseLocation) { self.close(reason: "outsideClick") }
            return event
        }
        menuObservers = [
            NotificationCenter.default.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.trackingMenu = true }
            },
            NotificationCenter.default.addObserver(forName: NSMenu.didEndTrackingNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.trackingMenu = false }
            }
        ]
        let animate = !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        panel.alphaValue = animate ? 0 : 1
        panel.makeKeyAndOrderFront(nil)
        if animate {
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.12
                panel.animator().alphaValue = 1
            }
        }
        focusSearch()
#if DEBUG
        writeLifecycle("shown")
#endif
        if let initialCommand { model.open(initialCommand) }
    }

    @discardableResult
    func handleKeyEvent(_ event: NSEvent) -> Bool {
        guard let panel, let model, panel.isVisible,
              event.window === panel || (event.window == nil && panel.isKeyWindow) else { return false }
        // Let input methods finish composition and native sheets/menus handle their keys.
        if let editor = panel.firstResponder as? NSTextView, editor.hasMarkedText() { return false }
        let modifiers = event.modifierFlags.intersection([.command, .control, .option, .shift])
        if modifiers == .command && event.charactersIgnoringModifiers?.lowercased() == "k" {
            model.back(); model.query = ""; focusSearch(); return true
        }
        guard modifiers.isEmpty else { return false }
        if event.keyCode == 53 {
            if model.workbench != nil { model.back(); focusSearch() } else { close() }
            return true
        }
        guard model.workbench == nil else { return false }
        switch event.keyCode {
        case 125: model.move(1)
        case 126: model.move(-1)
        case 121: model.move(7)
        case 116: model.move(-7)
        case 36, 76: model.openSelected()
        default: return false
        }
        return true
    }

    private func updatePresentation() {
        guard let panel, let model else { return }
        let isWorkbench = model.workbench != nil
        let size = isWorkbench ? NSSize(width: 820, height: 660) : model.paletteSize
        panel.contentMinSize = NSSize(width: 1, height: 1)
        panel.contentMaxSize = NSSize(width: 10_000, height: 10_000)
        let oldFrame = panel.frame
        let screen = panel.screen ?? ScreenPlacement.screen(containing: oldFrame.origin)
        let origin = ScreenPlacement.clamp(origin: CGPoint(x: oldFrame.midX - size.width / 2,
            y: oldFrame.maxY - size.height), size: size, into: screen)
        let frame = NSRect(origin: origin, size: size)
        presentationID = UUID()
        let id = presentationID
        let finish = { [weak self, weak panel] in
            guard let self, let panel, self.panel === panel, self.presentationID == id else { return }
            panel.contentMinSize = isWorkbench ? NSSize(width: 740, height: 560) : size
            panel.contentMaxSize = isWorkbench ? NSSize(width: 1_600, height: 1_200) : size
        }
        if frame == oldFrame || NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
            panel.setFrame(frame, display: true)
            finish()
        } else {
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.16
                panel.animator().setFrame(frame, display: true)
            }, completionHandler: finish)
        }
        if !isWorkbench { focusSearch() }
    }

    private func focusSearch() {
        DispatchQueue.main.async { [weak self] in
            guard let self, self.model?.workbench == nil, let panel = self.panel, panel.isVisible,
                  let field = self.searchField, field.window === panel else { return }
            if panel.firstResponder !== field.currentEditor() { panel.makeFirstResponder(field) }
        }
    }
    func windowDidBecomeKey(_ notification: Notification) { focusSearch() }
    func windowDidResignKey(_ notification: Notification) {
        guard let previous = notification.object as? NSWindow, previous === panel else { return }
        DispatchQueue.main.async { [weak self, weak previous] in
            guard let self, let panel = self.panel, panel === previous, !panel.isKeyWindow, !self.trackingMenu, panel.attachedSheet == nil,
                  !Self.belongsToPanel(NSApp.keyWindow, panel: panel) else { return }
            self.close(reason: "focusLost")
        }
    }
    static func belongsToPanel(_ window: NSWindow?, panel: NSWindow) -> Bool {
        guard let window else { return false }
        return window === panel || window.sheetParent === panel || window.parent === panel
    }
    func close(reason: String = "dismissed") {
#if DEBUG
        if panel != nil { writeLifecycle(reason) }
#endif
        model?.cancelAll()
        presentationID = UUID()
        for monitor in [keyMonitor, outsideMonitor, localClickMonitor].compactMap({ $0 }) { NSEvent.removeMonitor(monitor) }
        keyMonitor = nil; outsideMonitor = nil; localClickMonitor = nil
        menuObservers.forEach { NotificationCenter.default.removeObserver($0) }
        menuObservers = []; trackingMenu = false
        panel?.delegate = nil
        panel?.close()
        searchField = nil; panel = nil; model = nil
    }
    func windowWillClose(_ notification: Notification) { close() }
#if DEBUG
    private func writeLifecycle(_ state: String) {
        guard let path = ProcessInfo.processInfo.environment["BELLOBOX_E2E_LAUNCHER_LIFECYCLE"] else { return }
        try? state.write(to: URL(fileURLWithPath: path), atomically: true, encoding: .utf8)
    }
#endif
}
