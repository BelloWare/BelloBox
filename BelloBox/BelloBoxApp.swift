import AppKit
import Combine
import SwiftUI
import Sparkle

@main
struct BelloBoxApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var settings = AppSettings.shared

    private let updaterController: SPUStandardUpdaterController
    private let updaterConfigured: Bool

    init() {
        let configured = Self.hasValidSparkleConfiguration()
        updaterConfigured = configured
        updaterController = SPUStandardUpdaterController(
            startingUpdater: configured,
            updaterDelegate: nil,
            userDriverDelegate: nil
        )
    }

    var body: some Scene {
        MenuBarExtra("BelloBox", systemImage: "wand.and.stars") {
            menuContent
        }

        Settings {
            SettingsView(settings: settings)
        }
    }

    @ViewBuilder
    private var menuContent: some View {
        Button("Ask BelloBox About Selection") {
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

        Button("Set Up BelloBox…") { appDelegate.showOnboarding() }

        Button("Settings…") { AppDelegate.openSettingsWindow() }
            .keyboardShortcut(",", modifiers: .command)

        if updaterConfigured {
            Button("Check for Updates…") { updaterController.checkForUpdates(nil) }
        }

        Divider()

        Button("Quit BelloBox") { NSApp.terminate(nil) }
            .keyboardShortcut("q", modifiers: .command)
    }

    // MARK: - Sparkle configuration validation

    private static func hasValidSparkleConfiguration(bundle: Bundle = .main) -> Bool {
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

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private(set) var overlay: SelectionOverlayController?
    private let settings = AppSettings.shared
    private let onboarding = OnboardingWindowController()
    private var cancellables = Set<AnyCancellable>()

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)

        let overlay = SelectionOverlayController(settings: settings)
        overlay.openSettings = { AppDelegate.openSettingsWindow() }
        overlay.start()
        self.overlay = overlay

        settings.$floatingButtonEnabled
            .receive(on: RunLoop.main)
            .sink { [weak overlay] enabled in overlay?.setFloatingButtonEnabled(enabled) }
            .store(in: &cancellables)

        if !settings.hasCompletedSetup {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in
                self?.showOnboarding()
            }
        } else if !AccessibilityService.isTrusted {
            AccessibilityService.requestPermissionPrompt()
        }
    }

    func showOnboarding() {
        onboarding.show(settings: settings) { [weak self] in
            // Re-establish monitors as soon as Accessibility is granted.
            self?.overlay?.restartMonitors()
        }
    }

    /// Opens the SwiftUI Settings scene. Uses the AppKit selector so it works on
    /// macOS 13 (where `SettingsLink` / `openSettings` are unavailable).
    static func openSettingsWindow() {
        NSApp.activate(ignoringOtherApps: true)
        if NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil) { return }
        NSApp.sendAction(Selector(("showPreferencesWindow:")), to: nil, from: nil)
    }
}
