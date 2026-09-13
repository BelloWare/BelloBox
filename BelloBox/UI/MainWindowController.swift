import AppKit
import SwiftUI

/// Hosts the home window. Sizing is set before centering so the window lands in
/// the middle of the screen.
@MainActor
final class MainWindowController: NSObject, NSWindowDelegate {
    private(set) var window: NSWindow?
    private var navigation: WindowNavigation<HomeCategory>?

    func show(
        settings: AppSettings,
        canCheckForUpdates: Bool,
        category: HomeCategory? = nil,
        onOpenSettings: @escaping () -> Void,
        onOpenGuide: @escaping () -> Void,
        onOpenLauncher: @escaping () -> Void,
        onCapture: @escaping () -> Void,
        onScrollCapture: @escaping () -> Void,
        onRecord: @escaping () -> Void,
        onOpenQR: @escaping () -> Void,
        onOpenTextTools: @escaping () -> Void,
        onOpenWorldClock: @escaping () -> Void,
        onCheckForUpdates: @escaping () -> Void,
        onOpenTool: @escaping (LauncherCommand) -> Void = { _ in }
    ) {
        if let window {
            if let category { navigation?.requested = category }
            AppActivation.bringAppForward()
            AppWindowChrome.place(window, on: window.screen, centered: false)
            window.makeKeyAndOrderFront(nil)
            return
        }

        let navigation = WindowNavigation<HomeCategory>(category)
        self.navigation = navigation
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
            onCheckForUpdates: onCheckForUpdates,
            onOpenTool: onOpenTool,
            navigation: navigation
        )
        let hosting = NSHostingController(rootView: ToolViewport(minimumSize: NSSize(width: 900, height: 640)) { view }
            .workspaceBackground().windowSurfacePreferences(settings))
        let window = NSWindow(contentViewController: hosting)
        AppWindowChrome.apply(to: window, title: "Bello Box")
        window.delegate = self
        AppWindowChrome.size(window, content: NSSize(width: 1000, height: 760), minimum: NSSize(width: 900, height: 640))
        AppWindowChrome.place(window, centered: true)
        self.window = window

        AppActivation.bringAppForward()
        window.makeKeyAndOrderFront(nil)
    }

    func hide() { window?.orderOut(nil) }

    func windowWillClose(_ notification: Notification) {
        window = nil
        navigation = nil
    }
}
