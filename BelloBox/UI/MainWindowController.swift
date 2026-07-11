import AppKit
import SwiftUI

/// Hosts the home window. Sizing is set before centering so the window lands in
/// the middle of the screen.
@MainActor
final class MainWindowController: NSObject, NSWindowDelegate {
    private var window: NSWindow?

    func show(
        settings: AppSettings,
        canCheckForUpdates: Bool,
        onOpenSettings: @escaping () -> Void,
        onOpenGuide: @escaping () -> Void,
        onOpenTokenUsage: @escaping () -> Void,
        onCheckForUpdates: @escaping () -> Void
    ) {
        if let window {
            AppActivation.bringAppForward()
            window.makeKeyAndOrderFront(nil)
            return
        }

        let view = MainView(
            settings: settings,
            canCheckForUpdates: canCheckForUpdates,
            onOpenSettings: onOpenSettings,
            onOpenGuide: onOpenGuide,
            onOpenTokenUsage: onOpenTokenUsage,
            onCheckForUpdates: onCheckForUpdates
        )
        let hosting = NSHostingController(rootView: view)
        let window = NSWindow(contentViewController: hosting)
        window.title = "Bello Box"
        window.styleMask = [.titled, .closable, .miniaturizable]
        window.isReleasedWhenClosed = false
        window.delegate = self
        window.setContentSize(NSSize(width: 660, height: 720))
        window.center()
        self.window = window

        AppActivation.bringAppForward()
        window.makeKeyAndOrderFront(nil)
    }

    func windowWillClose(_ notification: Notification) {
        window = nil
    }
}
