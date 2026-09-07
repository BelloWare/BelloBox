import AppKit
import SwiftUI

/// Hosts the Settings window directly. A managed window keeps the menu-bar extra
/// and Dock-launched app paths using the same settings surface.
@MainActor
final class SettingsWindowController: NSObject, NSWindowDelegate {
    private var window: NSWindow?

    func show(settings: AppSettings) {
        if let window {
            AppActivation.bringAppForward()
            AppWindowChrome.place(window, on: window.screen, centered: false)
            window.makeKeyAndOrderFront(nil)
            return
        }

        let hosting = NSHostingController(rootView: ToolViewport(minimumSize: NSSize(width: 900, height: 680)) { SettingsView(settings: settings) })
        let window = NSWindow(contentViewController: hosting)
        AppWindowChrome.apply(to: window, title: "Bello Box Settings")
        window.contentMinSize = NSSize(width: 900, height: 680)
        window.delegate = self
        window.setContentSize(NSSize(width: 900, height: 720))
        AppWindowChrome.place(window, centered: true)
        self.window = window

        AppActivation.bringAppForward()
        window.makeKeyAndOrderFront(nil)
    }

    func windowWillClose(_ notification: Notification) {
        window = nil
    }
}
