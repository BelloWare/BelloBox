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
        .commands {
            CommandGroup(after: .appInfo) {
                Button("Check for Updates…") { appDelegate.checkForUpdates() }
            }
        }
    }

    @ViewBuilder
    private var menuContent: some View {
        Button("Open Bello Box") { appDelegate.showMainWindow() }

        Divider()

        Button("Ask Bello Box About Selection") {
            appDelegate.overlay?.triggerOnCurrentSelection()
        }
        Button("Capture Screenshot…") {
            appDelegate.overlay?.triggerScreenshotCapture()
        }
        Button("Capture Scrolling Screenshot…") {
            appDelegate.overlay?.triggerScrollingScreenshotCapture()
        }
        Button("Record Screen…") {
            appDelegate.overlay?.triggerRecording()
        }
        Button("Stop Recording") {
            appDelegate.overlay?.stopRecording()
        }
        .disabled(!(appDelegate.overlay?.isRecording ?? false))
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
        NSApp.setActivationPolicy(.regular)
        applyApplicationIcon()
        configureUpdater()

        applyAppearance(settings.appearance)
        settings.$appearance
            .receive(on: RunLoop.main)
            .sink { [weak self] preference in self?.applyAppearance(preference) }
            .store(in: &cancellables)

        syncLaunchAtLoginFromSettings()
        settings.$launchAtLoginEnabled
            .removeDuplicates()
            .dropFirst()
            .receive(on: RunLoop.main)
            .sink { [weak self] enabled in self?.applyLaunchAtLogin(enabled) }
            .store(in: &cancellables)

        let overlay = SelectionOverlayController(settings: settings)
        overlay.openSettings = { [weak self] in self?.showSettings() }
        overlay.start()
        self.overlay = overlay

        settings.$floatingButtonEnabled
            .receive(on: RunLoop.main)
            .sink { [weak overlay] enabled in overlay?.setFloatingButtonEnabled(enabled) }
            .store(in: &cancellables)

        settings.$globalHotkeyEnabled
            .receive(on: RunLoop.main)
            .sink { [weak overlay] enabled in overlay?.setGlobalHotkeyEnabled(enabled) }
            .store(in: &cancellables)

        Publishers.CombineLatest(settings.$globalHotkeyKeyCode, settings.$globalHotkeyModifiersRawValue)
            .receive(on: RunLoop.main)
            .sink { [weak overlay, weak settings] _, _ in
                guard let settings else { return }
                overlay?.setGlobalHotkey(settings.globalHotkey)
            }
            .store(in: &cancellables)

        settings.$screenshotHotkeyEnabled
            .receive(on: RunLoop.main)
            .sink { [weak overlay] enabled in overlay?.setScreenshotHotkeyEnabled(enabled) }
            .store(in: &cancellables)

        Publishers.CombineLatest(settings.$screenshotHotkeyKeyCode, settings.$screenshotHotkeyModifiersRawValue)
            .receive(on: RunLoop.main)
            .sink { [weak overlay, weak settings] _, _ in
                guard let settings else { return }
                overlay?.setScreenshotHotkey(settings.screenshotHotkey)
            }
            .store(in: &cancellables)

        settings.$recordingHotkeyEnabled
            .receive(on: RunLoop.main)
            .sink { [weak overlay] enabled in overlay?.setRecordingHotkeyEnabled(enabled) }
            .store(in: &cancellables)

        Publishers.CombineLatest(settings.$recordingHotkeyKeyCode, settings.$recordingHotkeyModifiersRawValue)
            .receive(on: RunLoop.main)
            .sink { [weak overlay, weak settings] _, _ in
                guard let settings else { return }
                overlay?.setRecordingHotkey(settings.recordingHotkey)
            }
            .store(in: &cancellables)

        settings.$activeShortcutRecorderID
            .map { $0 != nil }
            .removeDuplicates()
            .receive(on: RunLoop.main)
            .sink { [weak overlay] active in overlay?.setShortcutRecordingActive(active) }
            .store(in: &cancellables)

#if DEBUG
        runE2EPermissionBootstrapIfNeeded()
#endif

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

    /// Re-opening the app (e.g. double-clicking it in Finder) brings a window up.
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
        AppActivation.bringAppForward()
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

    private func syncLaunchAtLoginFromSettings() {
        let actual = LaunchAtLoginController.isEnabled
        if actual {
            settings.launchAtLoginEnabled = true
        } else if settings.launchAtLoginEnabled {
            applyLaunchAtLogin(true)
        }
    }

    private func applyLaunchAtLogin(_ enabled: Bool) {
        do {
            try LaunchAtLoginController.setEnabled(enabled)
        } catch {
            NSLog("Bello Box could not update launch-at-login: \(error.localizedDescription)")
            let actual = LaunchAtLoginController.isEnabled
            if settings.launchAtLoginEnabled != actual {
                settings.launchAtLoginEnabled = actual
            }
        }
    }

#if DEBUG
    private func runE2EPermissionBootstrapIfNeeded() {
        let env = ProcessInfo.processInfo.environment
        guard env["BELLOBOX_E2E_REQUEST_PERMISSIONS"] == "1" ||
              env["BELLOBOX_E2E_PERMISSION_MARKER"] != nil
        else { return }

        Task { @MainActor in
            let before = e2ePermissionStatusLines(prefix: "before")
            writeE2EPermissionMarker(phase: "before-requests", lines: before)

            if env["BELLOBOX_E2E_REQUEST_PERMISSIONS"] == "1" {
                if !AccessibilityService.isTrusted {
                    AccessibilityService.requestPermissionPrompt()
                }
                if !ScreenCapturePermission.isTrusted {
                    _ = ScreenCapturePermission.requestPrompt()
                }
                if InputMonitoringPermission.status() != .granted {
                    _ = InputMonitoringPermission.request()
                }
                if MicrophonePermission.status() == .notDetermined {
                    _ = await MicrophonePermission.request()
                }
            }

            let after = e2ePermissionStatusLines(prefix: "after")
            writeE2EPermissionMarker(phase: "after-requests", lines: before + after)
        }
    }

    private func e2ePermissionStatusLines(prefix: String) -> [String] {
        [
            "\(prefix).accessibility=\(AccessibilityService.isTrusted ? "granted" : "missing")",
            "\(prefix).screenRecording=\(ScreenCapturePermission.isTrusted ? "granted" : "missing")",
            "\(prefix).inputMonitoring=\(String(describing: InputMonitoringPermission.status()))",
            "\(prefix).microphone=\(String(describing: MicrophonePermission.status()))",
        ]
    }

    private func writeE2EPermissionMarker(phase: String, lines: [String]) {
        let env = ProcessInfo.processInfo.environment
        guard let path = env["BELLOBOX_E2E_PERMISSION_MARKER"],
              !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return }

        let url = URL(fileURLWithPath: path)
        let payload = ([
            "kind=permission-bootstrap",
            "phase=\(phase)",
            "bundleID=\(Bundle.main.bundleIdentifier ?? "unknown")",
            "timestamp=\(Date().timeIntervalSince1970)",
        ] + lines).joined(separator: "\n")

        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try payload.write(to: url, atomically: true, encoding: .utf8)
        } catch {
            NSLog("BelloBox E2E permission marker failed: \(error.localizedDescription)")
        }
    }
#endif

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
