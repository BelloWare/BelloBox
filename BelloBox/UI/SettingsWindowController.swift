import AppKit
import SwiftUI

/// Hosts the Settings window directly. A managed window keeps the menu-bar extra
/// and Dock-launched app paths using the same settings surface.
@MainActor
final class SettingsWindowController: NSObject, NSWindowDelegate {
    private var window: NSWindow?
    private var navigation: WindowNavigation<SettingsCategory>?

    /// `category` opens (or switches) the window to that page.
    func show(settings: AppSettings, category: SettingsCategory? = nil) {
        if let window {
            if let category { navigation?.requested = category }
            AppActivation.bringAppForward()
            AppWindowChrome.place(window, on: window.screen, centered: false)
            window.makeKeyAndOrderFront(nil)
            return
        }

        let navigation = WindowNavigation<SettingsCategory>(category)
        self.navigation = navigation
        let hosting = NSHostingController(rootView: ToolViewport(minimumSize: NSSize(width: 900, height: 680)) {
            SettingsView(settings: settings, navigation: navigation)
        }.workspaceBackground().windowSurfacePreferences(settings))
        let window = NSWindow(contentViewController: hosting)
        AppWindowChrome.apply(to: window, title: "Bello Box Settings")
        window.delegate = self
        AppWindowChrome.size(window, content: NSSize(width: 900, height: 720), minimum: NSSize(width: 900, height: 680))
        AppWindowChrome.place(window, centered: true)
        self.window = window

        AppActivation.bringAppForward()
        window.makeKeyAndOrderFront(nil)
    }

    func windowWillClose(_ notification: Notification) {
        window = nil
        navigation = nil
    }
}
