import AppKit
import SwiftUI

/// Owns full developer-tool windows independently of the transient launcher.
/// Each explicit open transfers or creates one model; windows never share drafts.
@MainActor
final class UtilityWorkbenchWindows {
    private(set) var windows: [UtilityWorkbenchWindowController] = []
    private var instanceCounts: [LauncherCommand: Int] = [:]
    private let onSearchTools: () -> Void

    init(onSearchTools: @escaping () -> Void) { self.onSearchTools = onSearchTools }

    @discardableResult
    func open(_ model: UtilityWorkbenchModel, settings: AppSettings, near frame: NSRect? = nil) -> UtilityWorkbenchWindowController {
        let instance = (instanceCounts[model.command] ?? 0) + 1
        instanceCounts[model.command] = instance
        let controller = UtilityWorkbenchWindowController(model: model, settings: settings, instance: instance)
        controller.onSearchTools = onSearchTools
        controller.onNewWindow = { [weak self, weak controller] in
            guard let self, let controller else { return }
            let previous = controller.model
            let fresh = UtilityWorkbenchModel(command: previous.command,
                selection: TextSelection(text: "", anchorRect: nil, appName: nil, bundleID: nil, pid: nil),
                snippets: previous.snippets)
            fresh.pinnedText = previous.pinnedText
            fresh.pinText = previous.pinText
            self.open(fresh, settings: settings, near: controller.window?.frame)
        }
        controller.onClose = { [weak self] closed in
            self?.windows.removeAll { $0 === closed }
        }
        let previousFrame = windows.last(where: { $0.window?.isVisible == true && $0.window?.isMiniaturized == false })?.window?.frame
        windows.append(controller)
        controller.show(near: frame, cascadingFrom: previousFrame)
        return controller
    }
}

/// A regular, independently closable macOS window: no outside-click or
/// resign-key dismissal, no auto-tabbing, and no always-on-top behavior.
@MainActor
final class UtilityWorkbenchWindowController: NSObject, NSWindowDelegate {
    let model: UtilityWorkbenchModel
    private let settings: AppSettings
    private let instance: Int
    private(set) var window: UtilityWorkbenchWindow?
    var onSearchTools: () -> Void = {}
    var onNewWindow: () -> Void = {}
    var onClose: (UtilityWorkbenchWindowController) -> Void = { _ in }

    init(model: UtilityWorkbenchModel, settings: AppSettings, instance: Int) {
        self.model = model
        self.settings = settings
        self.instance = instance
    }

    func show(near frame: NSRect?, cascadingFrom previousFrame: NSRect?) {
        guard window == nil else { window?.makeKeyAndOrderFront(nil); return }
        model.previewsOnly = false
        let window = UtilityWorkbenchWindow(contentRect: .zero, styleMask: AppWindowChrome.styleMask, backing: .buffered, defer: false)
        let title = model.command.title + (instance > 1 ? " (\(instance))" : "")
        AppWindowChrome.apply(to: window, title: title + " — Bello Box")
        window.identifier = NSUserInterfaceItemIdentifier("utility-" + UUID().uuidString)
        window.tabbingMode = .disallowed
        window.hidesOnDeactivate = false
        window.delegate = self
        window.onNewWindow = { [weak self] in self?.onNewWindow() }
        window.onSearchTools = { [weak self] in self?.onSearchTools() }
        window.onCopyResult = { [weak model] in model?.copyOutput() }
        window.onSend = { [weak model] in
            guard let model, model.command == .http, !model.busy, !model.request.url.isEmpty else { return false }
            model.sendRequest()
            return true
        }
        model.onReplace = { [weak model] text in
            guard let pid = model?.selection.pid else { return }
            AccessibilityService().replaceSelection(with: text, pid: pid)
        }
        window.contentViewController = NSHostingController(rootView:
            ToolViewport(minimumSize: CGSize(width: 740, height: 560)) {
                UtilityWorkbenchView(model: model, onSearchTools: { [weak self] in self?.onSearchTools() },
                                     onNewWindow: { [weak self] in self?.onNewWindow() })
            }.workspaceBackground().windowSurfacePreferences(settings))
        AppWindowChrome.size(window, content: NSSize(width: 820, height: 660), minimum: NSSize(width: 740, height: 560))
        let screen = frame.map { ScreenPlacement.screen(containing: NSPoint(x: $0.midX, y: $0.midY)) }
            ?? ScreenPlacement.screen(containing: NSEvent.mouseLocation)
        AppWindowChrome.place(window, on: screen, centered: true)
        if let previousFrame, screen.visibleFrame.intersects(previousFrame) {
            window.setFrameOrigin(NSPoint(x: previousFrame.minX + 28, y: previousFrame.maxY - window.frame.height - 28))
            AppWindowChrome.place(window, on: screen, centered: false)
        }
        self.window = window
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        if model.result == nil, model.error == nil, !model.busy, model.command != .http { model.schedule() }
    }

    func close() { window?.performClose(nil) }

    func windowWillClose(_ notification: Notification) {
        guard let closing = notification.object as? NSWindow, closing === window else { return }
        model.cancel()
        model.onReplace = { _ in }
        closing.delegate = nil
        window = nil
        onClose(self)
    }
}

/// Handle window commands in the key window rather than registering one
/// application-wide SwiftUI shortcut for every open tool instance.
final class UtilityWorkbenchWindow: NSWindow {
    var onNewWindow: () -> Void = {}
    var onSearchTools: () -> Void = {}
    var onCopyResult: () -> Void = {}
    var onSend: () -> Bool = { false }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard isKeyWindow, attachedSheet == nil,
              (firstResponder as? NSTextView)?.hasMarkedText() != true else {
            return super.performKeyEquivalent(with: event)
        }
        let modifiers = event.modifierFlags.intersection([.command, .control, .option, .shift])
        let key = event.charactersIgnoringModifiers?.lowercased()
        if modifiers == .command {
            switch key {
            case "w": performClose(nil); return true
            case "n": onNewWindow(); return true
            case "k": onSearchTools(); return true
            case "\r": if onSend() { return true }
            default: break
            }
        }
        if modifiers == [.command, .shift], key == "c" { onCopyResult(); return true }
        return super.performKeyEquivalent(with: event)
    }

    /// Escape may dismiss an editor's completion UI, but never the document.
    override func cancelOperation(_ sender: Any?) {}
}
