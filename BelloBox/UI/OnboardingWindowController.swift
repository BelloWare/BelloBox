import AppKit
import SwiftUI

/// Hosts the onboarding flow in a standard window. The app is a menu-bar
/// accessory, so onboarding is shown by explicitly creating and focusing a
/// window rather than relying on a SwiftUI `WindowGroup`.
@MainActor
final class OnboardingWindowController: NSObject, NSWindowDelegate {
    private var window: NSWindow?
    private weak var settings: AppSettings?

    func show(settings: AppSettings, onPermissionGranted: @escaping () -> Void) {
        self.settings = settings

        if let window {
            NSApp.activate(ignoringOtherApps: true)
            window.makeKeyAndOrderFront(nil)
            return
        }

        let view = OnboardingView(
            settings: settings,
            onPermissionGranted: onPermissionGranted,
            onFinish: { [weak self] in self?.finish() }
        )
        let hosting = NSHostingController(rootView: view)
        let window = NSWindow(contentViewController: hosting)
        window.title = "Welcome to BelloBox"
        window.styleMask = [.titled, .closable]
        window.isReleasedWhenClosed = false
        window.delegate = self
        window.center()
        self.window = window

        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    private func finish() {
        settings?.hasCompletedSetup = true
        window?.close()
    }

    func windowWillClose(_ notification: Notification) {
        // Closing the guide counts as having seen setup; it stays reachable from
        // the menu bar.
        settings?.hasCompletedSetup = true
        window = nil
    }
}
