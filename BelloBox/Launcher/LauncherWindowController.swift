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
    /// A native menu (a preview's zone or audio picker, the reference menu) is
    /// open: it owns the keyboard and must not dismiss the palette.
    private(set) var isTrackingMenu = false
    private(set) var model: LauncherModel?
    /// The one search field. It stays attached while a tool is open (parked
    /// by `LauncherView`), so returning to the palette never waits for a mount.
    private weak var searchField: LauncherSearchTextField?
    /// A focus request made before the field was attached (palette opening).
    private var pendingSearchFocus = false
    private weak var copilotField: LauncherSearchTextField?
    private var presentationID = UUID()
#if DEBUG
    private let snippets = SnippetStore(url: ProcessInfo.processInfo.environment["BELLOBOX_E2E_SNIPPETS_PATH"].map { URL(fileURLWithPath: $0) })
#else
    private let snippets = SnippetStore()
#endif
    private var pinned: String?
    var settings: AppSettings = .shared
    var isVisible: Bool { panel?.isVisible == true }
    var onCommand: (LauncherCommand, TextSelection, LauncherCommandContext) -> Void = { _, _, _ in }

    /// `focus` highlights a row (and expands its preview) without opening it;
    /// `initialCommand` opens a tool at once.
    func show(selection: TextSelection, initialCommand: LauncherCommand? = nil, focus: LauncherCommand? = nil) {
        close()
        var worldClockPreferences: WorldClockPreferencesStore?
#if DEBUG
        worldClockPreferences = WorldClockPreferencesStore.e2eFixture()
#endif
        let model = LauncherModel(selection: selection, snippets: snippets, settings: settings,
                                  worldClockPreferences: worldClockPreferences)
        model.hostWindow = { [weak self] in self?.panel }
        model.onClose = { [weak self] in self?.close() }
        model.onCommand = { [weak self] command, selection, context in
            self?.close(); self?.onCommand(command, selection, context)
        }
        model.pinnedText = { [weak self] in self?.pinned }
        model.pinText = { [weak self] in self?.pinned = $0 }
        model.onPresentationChange = { [weak self] in self?.updatePresentation() }
        model.onPreviewResize = { [weak self] in self?.updatePresentation(refocusSearch: false) }
        model.onFocusSearch = { [weak self] in self?.focusSearch(force: true) }
        let panel = LauncherPanel()
        self.panel = panel; self.model = model
        panel.delegate = self
        panel.contentViewController = NSHostingController(rootView: LauncherView(model: model, onSearchReady: { [weak self] field in
            guard let self else { return }
            self.searchField = field
            if self.pendingSearchFocus { self.focusSearch(force: true) }
        }, onCopilotFieldReady: { [weak self] field in
            self?.copilotField = field
        }).windowSurfacePreferences(settings))
        let screen = ScreenPlacement.screen(containing: NSEvent.mouseLocation)
        let contentSize = Self.fittedSize(model.paletteSize, visibleFrame: screen.visibleFrame)
        panel.setContentSize(contentSize)
        panel.contentMinSize = contentSize
        panel.contentMaxSize = contentSize
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
            if !self.isTrackingMenu, !Self.belongsToPanel(event.window, panel: panel), panel.attachedSheet == nil,
               !panel.frame.contains(NSEvent.mouseLocation) { self.close(reason: "outsideClick") }
            return event
        }
        menuObservers = [
            NotificationCenter.default.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.isTrackingMenu = true }
            },
            NotificationCenter.default.addObserver(forName: NSMenu.didEndTrackingNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.isTrackingMenu = false }
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
        if let focus { model.selectedID = focus.id; updatePresentation() }
        focusSearch()
#if DEBUG
        writeLifecycle("shown")
#endif
        if let initialCommand { model.open(initialCommand) }
    }

    /// Test seam: what the menu tracking notifications set.
    func setMenuTracking(_ tracking: Bool) { isTrackingMenu = tracking }

    /// Whether keyboard input currently belongs to an editable text field other
    /// than the palette's search field: the copilot question or a preview's
    /// own input. A read-only output that was clicked does not count; the
    /// palette keeps its navigation keys and typing returns to search.
    func isSecondaryTextInputFocused(in panel: NSWindow) -> Bool {
        guard let editor = panel.firstResponder as? NSTextView, editor.isEditable else { return false }
        guard let searchField else { return true }
        return editor.delegate !== searchField
    }

    /// Whether a read-only preview output (a result text view) has keyboard focus.
    func isReadOnlyOutputFocused(in panel: NSWindow) -> Bool {
        guard let editor = panel.firstResponder as? NSTextView, !editor.isEditable else { return false }
        return editor.delegate !== searchField
    }

    /// Whether the search field is actively editing.
    var isSearchFieldFocused: Bool {
        guard let panel, let searchField, let editor = panel.firstResponder as? NSTextView else { return false }
        return editor.delegate === searchField
    }

    @discardableResult
    func handleKeyEvent(_ event: NSEvent) -> Bool {
        guard let panel, let model, panel.isVisible,
              event.window === panel || (event.window == nil && panel.isKeyWindow) else { return false }
        // Native menus and attached sheets own the keyboard entirely.
        if isTrackingMenu || panel.attachedSheet != nil { return false }
        // Let input methods finish composition.
        if let editor = panel.firstResponder as? NSTextView, editor.hasMarkedText() { return false }
        let modifiers = event.modifierFlags.intersection([.command, .control, .option, .shift])
        if modifiers == .command && event.charactersIgnoringModifiers?.lowercased() == "k" {
            model.back(); model.query = ""; focusSearch(force: true); return true
        }
        if model.workbench == nil, isSecondaryTextInputFocused(in: panel) {
            // A preview's input owns Enter and the arrows; Escape hands focus back.
            if modifiers.isEmpty, event.keyCode == 53 { focusSearch(force: true); return true }
            return false
        }
        if model.workbench == nil, isReadOnlyOutputFocused(in: panel) {
            // A clicked result keeps ⌘C and selection; Escape returns to search,
            // navigation keys still move rows, and typing goes to the search field.
            if modifiers.isEmpty, event.keyCode == 53 { focusSearch(force: true); return true }
            if modifiers.isSubset(of: [.shift, .option]), !Self.isNavigationKey(event.keyCode), let field = searchField, field.window === panel,
               event.characters?.isEmpty == false {
                if panel.makeFirstResponder(field) {
                    pendingSearchFocus = false
                    // Continue the query instead of replacing the selected text.
                    let length = (field.currentEditor()?.string as NSString?)?.length ?? 0
                    field.currentEditor()?.selectedRange = NSRange(location: length, length: 0)
                }
                return false
            }
        }
        if model.workbench == nil, model.featuresClock, model.query.isEmpty, [123, 124].contains(event.keyCode),
           modifiers.isSubset(of: [.option, .shift]) {
            let direction = event.keyCode == 124 ? 1 : -1
            let step: TimeInterval = modifiers.contains(.shift) ? 24 * 3_600 : modifiers.contains(.option) ? 3_600 : 15 * 60
            if modifiers.contains(.shift) { model.clockPreview?.moveDay(by: direction) } else { model.nudgeClock(by: direction, step: step) }
            return true
        }
        guard modifiers.isEmpty else { return false }
        if event.keyCode == 53 {
            if model.workbench != nil { model.back() } else { close() }
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

    /// Arrows, Enter, Escape, Tab, and paging keys: never redirected as typing.
    static func isNavigationKey(_ keyCode: UInt16) -> Bool {
        [36, 76, 48, 53, 115, 116, 119, 121, 123, 124, 125, 126].contains(keyCode)
    }

    /// Keeps the palette inside the screen it is on. Small displays get a
    /// shorter panel whose command list scrolls; the footer stays reachable.
    static let screenMargin: CGFloat = 12
    static func fittedSize(_ size: NSSize, visibleFrame: NSRect) -> NSSize {
        let maxHeight = max(320, visibleFrame.height - screenMargin * 2)
        return NSSize(width: size.width, height: min(size.height, maxHeight))
    }

    private func updatePresentation(refocusSearch: Bool = true) {
        guard let panel, let model else { return }
        let isWorkbench = model.workbench != nil
        let oldFrame = panel.frame
        let screen = panel.screen ?? ScreenPlacement.screen(containing: oldFrame.origin)
        let size = Self.fittedSize(isWorkbench ? NSSize(width: 820, height: 660) : model.paletteSize, visibleFrame: screen.visibleFrame)
        panel.contentMinSize = NSSize(width: 1, height: 1)
        panel.contentMaxSize = NSSize(width: 10_000, height: 10_000)
        let origin = ScreenPlacement.clamp(origin: CGPoint(x: oldFrame.midX - size.width / 2,
            y: oldFrame.maxY - size.height), size: size, into: screen)
        let frame = NSRect(origin: origin, size: size)
        presentationID = UUID()
        let id = presentationID
        let finish = { [weak self, weak panel] in
            guard let self, let panel, self.panel === panel, self.presentationID == id else { return }
            panel.contentMinSize = isWorkbench ? Self.fittedSize(NSSize(width: 740, height: 560), visibleFrame: screen.visibleFrame) : size
            panel.contentMaxSize = isWorkbench ? NSSize(width: 1_600, height: 1_200) : size
            // A quick Back → Open can reuse the fading editor before it ever
            // detaches from the window, so attachment alone cannot restore focus.
            if isWorkbench, let root = panel.contentView {
                let current = panel.firstResponder as? NSTextView
                let ownsInput = current?.window === panel && !(current?.delegate is LauncherSearchTextField)
                if !ownsInput, let editor = LiteralTextView.initialEditor(in: root) {
                    panel.makeFirstResponder(editor)
                }
            }
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
        if !isWorkbench, refocusSearch { focusSearch() }
    }

    /// Returns focus to the search field. Unless forced, it leaves another
    /// text input (the copilot question, a preview field) alone so typing is
    /// never interrupted. A forced request (Escape, Back, ⌘K) takes effect
    /// at once: the field is always attached, so the next key event already
    /// belongs to it. Before the field is attached (the palette opening) the
    /// request waits for `onSearchReady`.
    private func focusSearch(force: Bool = false) {
        if force {
            pendingSearchFocus = true
            if focusSearchNow() { return }
        }
        DispatchQueue.main.async { [weak self] in self?.focusSearchNow() }
    }

    /// Makes the search field first responder if it is attached and nothing
    /// else should keep typing. Returns whether it is editing afterwards.
    @discardableResult
    private func focusSearchNow() -> Bool {
        guard model?.workbench == nil, let panel, panel.isVisible, let field = searchField, field.window === panel else { return false }
        if !pendingSearchFocus, isSecondaryTextInputFocused(in: panel) { return false }
        // The parked state may not have been cleared by SwiftUI yet.
        field.isEnabled = true
        guard panel.firstResponder === field.currentEditor() || panel.makeFirstResponder(field) else { return false }
        pendingSearchFocus = false
        return true
    }
    func windowDidBecomeKey(_ notification: Notification) { focusSearch() }
    func windowDidResignKey(_ notification: Notification) {
        guard let previous = notification.object as? NSWindow, previous === panel else { return }
        DispatchQueue.main.async { [weak self, weak previous] in
            guard let self, let panel = self.panel, panel === previous, !panel.isKeyWindow, !self.isTrackingMenu, panel.attachedSheet == nil,
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
        menuObservers = []; isTrackingMenu = false
        panel?.delegate = nil
        panel?.close()
        searchField = nil; copilotField = nil; pendingSearchFocus = false; panel = nil; model = nil
    }
    func windowWillClose(_ notification: Notification) { close() }
#if DEBUG
    private func writeLifecycle(_ state: String) {
        guard let path = ProcessInfo.processInfo.environment["BELLOBOX_E2E_LAUNCHER_LIFECYCLE"] else { return }
        try? state.write(to: URL(fileURLWithPath: path), atomically: true, encoding: .utf8)
    }
#endif
}
