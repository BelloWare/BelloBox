import AppKit
import SwiftUI

/// Hosts the onboarding flow in a standard window so the menu-bar extra and
/// Dock-launched app paths use the same setup surface.
@MainActor
final class OnboardingWindowController: NSObject, NSWindowDelegate {
    private var window: NSWindow?
    private weak var settings: AppSettings?
    private var onClosed: (() -> Void)?

    func show(settings: AppSettings, onPermissionGranted: @escaping () -> Void, onClosed: @escaping () -> Void) {
        self.settings = settings
        self.onClosed = onClosed

        if let window {
            AppActivation.bringAppForward()
            AppWindowChrome.place(window, on: window.screen, centered: false)
            window.makeKeyAndOrderFront(nil)
            return
        }

        let view = OnboardingView(
            settings: settings,
            onPermissionGranted: onPermissionGranted,
            onFinish: { [weak self] in self?.finish() }
        )
        let hosting = NSHostingController(rootView: ToolViewport(minimumSize: NSSize(width: 680, height: 500)) { view }
            .workspaceBackground().windowSurfacePreferences(settings))
        let window = NSWindow(contentViewController: hosting)
        AppWindowChrome.apply(to: window, title: "Welcome to Bello Box")
        window.delegate = self
        // Size before centering so the window lands in the middle of the screen.
        AppWindowChrome.size(window, content: NSSize(width: 680, height: 720), minimum: NSSize(width: 680, height: 500))
        AppWindowChrome.place(window, centered: true)
        self.window = window

        AppActivation.bringAppForward()
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
        let closed = onClosed
        onClosed = nil
        closed?()
    }
}
