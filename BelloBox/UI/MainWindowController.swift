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
        onOpenLauncher: @escaping () -> Void,
        onCapture: @escaping () -> Void,
        onScrollCapture: @escaping () -> Void,
        onRecord: @escaping () -> Void,
        onOpenQR: @escaping () -> Void,
        onOpenTextTools: @escaping () -> Void,
        onOpenWorldClock: @escaping () -> Void,
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
            onOpenLauncher: onOpenLauncher,
            onCapture: onCapture,
            onScrollCapture: onScrollCapture,
            onRecord: onRecord,
            onOpenQR: onOpenQR,
            onOpenTextTools: onOpenTextTools,
            onOpenWorldClock: onOpenWorldClock,
            onCheckForUpdates: onCheckForUpdates
        )
        let hosting = NSHostingController(rootView: view)
        let window = NSWindow(contentViewController: hosting)
        window.title = "Bello Box"
        window.styleMask = [.titled, .closable, .miniaturizable, .resizable]
        window.isReleasedWhenClosed = false
        window.delegate = self
        window.setContentSize(NSSize(width: 660, height: 780))
        window.contentMinSize = NSSize(width: 660, height: 600)
        window.center()
        self.window = window

        AppActivation.bringAppForward()
        window.makeKeyAndOrderFront(nil)
    }

    func hide() { window?.orderOut(nil) }

    func windowWillClose(_ notification: Notification) {
        window = nil
    }
}
