import AppKit
import Combine
import SwiftUI
import Sparkle

@main
struct BelloBoxApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var settings = AppSettings.shared

    var body: some Scene {
        MenuBarExtra("Bello Box", systemImage: "wand.and.stars") {
            menuContent
        }
    }

    @ViewBuilder
    private var menuContent: some View {
        Button("Open Bello Box") { appDelegate.showMainWindow() }

        Divider()

        Button("Ask Bello Box About Selection") {
            appDelegate.overlay?.triggerOnCurrentSelection()
        }
        Button("Generate QR Code from Selection") {
            appDelegate.overlay?.triggerQROnCurrentSelection()
        }
        Button("Text Tools on Selection") {
            appDelegate.overlay?.triggerTextToolsOnCurrentSelection()
        }
        Text(settings.isConfigured ? "Provider: \(settings.providerKind.displayName)" : "Not configured — open Settings")
            .font(.caption)

        Divider()

        Button("Set Up Bello Box…") { appDelegate.showOnboarding() }

        Button("Settings…") { appDelegate.showSettings() }
            .keyboardShortcut(",", modifiers: .command)

        if appDelegate.updaterConfigured {
            Button("Check for Updates…") { appDelegate.checkForUpdates() }
        }

        Divider()

        Button("Quit Bello Box") { NSApp.terminate(nil) }
            .keyboardShortcut("q", modifiers: .command)
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private(set) var overlay: SelectionOverlayController?
    private(set) var updaterConfigured = false

    private let settings = AppSettings.shared
    private let onboarding = OnboardingWindowController()
    private let mainWindow = MainWindowController()
    private let settingsWindow = SettingsWindowController()
    private var updaterController: SPUStandardUpdaterController?
    private var cancellables = Set<AnyCancellable>()

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        applyApplicationIcon()
        configureUpdater()

        applyAppearance(settings.appearance)
        settings.$appearance
            .receive(on: RunLoop.main)
            .sink { [weak self] preference in self?.applyAppearance(preference) }
            .store(in: &cancellables)

        let overlay = SelectionOverlayController(settings: settings)
        overlay.openSettings = { [weak self] in self?.showSettings() }
        overlay.start()
        self.overlay = overlay

        settings.$floatingButtonEnabled
            .receive(on: RunLoop.main)
            .sink { [weak overlay] enabled in overlay?.setFloatingButtonEnabled(enabled) }
            .store(in: &cancellables)

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in
            guard let self else { return }
            if self.settings.hasCompletedSetup {
                if !AccessibilityService.isTrusted { AccessibilityService.requestPermissionPrompt() }
                self.showMainWindow()
            } else {
                self.showOnboarding()
            }
        }
    }

    /// Re-opening the app (e.g. double-clicking it in Finder) brings a window up,
    /// since an accessory app has nothing in the Dock.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if settings.hasCompletedSetup {
            showMainWindow()
        } else {
            showOnboarding()
        }
        return true
    }

    // MARK: - Windows

    func showMainWindow() {
        mainWindow.show(
            settings: settings,
            canCheckForUpdates: updaterConfigured,
            onOpenSettings: { [weak self] in self?.showSettings() },
            onOpenGuide: { [weak self] in self?.showOnboarding() },
            onCheckForUpdates: { [weak self] in self?.checkForUpdates() }
        )
    }

    func showOnboarding() {
        onboarding.show(
            settings: settings,
            onPermissionGranted: { [weak self] in self?.overlay?.restartMonitors() },
            onClosed: { [weak self] in self?.showMainWindow() }
        )
    }

    func showSettings() {
        settingsWindow.show(settings: settings)
    }

    // MARK: - Updates

    private func configureUpdater() {
        updaterConfigured = Self.hasValidSparkleConfiguration()
        updaterController = SPUStandardUpdaterController(
            startingUpdater: updaterConfigured,
            updaterDelegate: nil,
            userDriverDelegate: nil
        )
    }

    func checkForUpdates() {
        guard updaterConfigured else { return }
        NSApp.activate(ignoringOtherApps: true)
        updaterController?.checkForUpdates(nil)
    }

    // MARK: - Appearance

    private func applyAppearance(_ preference: AppearancePreference) {
        switch preference {
        case .system: NSApp.appearance = nil
        case .light: NSApp.appearance = NSAppearance(named: .aqua)
        case .dark: NSApp.appearance = NSAppearance(named: .darkAqua)
        }
    }

    private func applyApplicationIcon() {
        let rawName = (Bundle.main.object(forInfoDictionaryKey: "CFBundleIconFile") as? String) ?? "AppIcon"
        let iconName = rawName.replacingOccurrences(of: ".icns", with: "")
        guard
            let url = Bundle.main.url(forResource: iconName, withExtension: "icns"),
            let image = NSImage(contentsOf: url)
        else { return }
        NSApp.applicationIconImage = image
    }

    // MARK: - Sparkle configuration validation

    static func hasValidSparkleConfiguration(bundle: Bundle = .main) -> Bool {
        guard
            let feed = bundle.object(forInfoDictionaryKey: "SUFeedURL") as? String,
            let publicKey = bundle.object(forInfoDictionaryKey: "SUPublicEDKey") as? String
        else { return false }

        let feedValue = feed.trimmingCharacters(in: .whitespacesAndNewlines)
        let keyValue = publicKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !feedValue.isEmpty, !keyValue.isEmpty else { return false }
        guard !feedValue.contains("$("), !keyValue.contains("$(") else { return false }
        guard let keyData = Data(base64Encoded: keyValue), keyData.count == 32 else { return false }
        guard let url = URL(string: feedValue), let scheme = url.scheme?.lowercased() else { return false }
        return scheme == "https" || scheme == "http"
    }
}
