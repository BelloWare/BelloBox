import AppKit
import SwiftUI

@MainActor
final class CodexTokenUsageWindowController: NSObject, NSWindowDelegate {
    private var window: NSWindow?

    func show() {
        if let window {
            AppActivation.bringAppForward()
            window.makeKeyAndOrderFront(nil)
            return
        }

        let hosting = NSHostingController(rootView: CodexTokenUsageDashboardView())
        let window = NSWindow(contentViewController: hosting)
        window.title = "Codex Token Usage"
        window.styleMask = [.titled, .closable, .miniaturizable, .resizable]
        window.isReleasedWhenClosed = false
        window.delegate = self
        window.setContentSize(NSSize(width: 980, height: 760))
        window.center()
        self.window = window

        AppActivation.bringAppForward()
        window.makeKeyAndOrderFront(nil)
    }

    func windowWillClose(_ notification: Notification) {
        window = nil
    }
}
