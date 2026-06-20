import AppKit
import SwiftUI

/// The app's home window — something visible to open, check status, reach
/// Settings and the guide, and check for updates.
struct MainView: View {
    @ObservedObject var settings: AppSettings
    var canCheckForUpdates: Bool
    var onOpenSettings: () -> Void
    var onOpenGuide: () -> Void
    var onCheckForUpdates: () -> Void

    @State private var trusted = AccessibilityService.isTrusted
    private let timer = Timer.publish(every: 1.5, on: .main, in: .common).autoconnect()

    private var versionText: String {
        let short = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?"
        let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "?"
        return "Version \(short) (\(build))"
    }

    private var howToText: String {
        switch (settings.floatingButtonEnabled, settings.globalHotkeyEnabled) {
        case (true, true):
            return "Select text in any app — a floating toolbar appears with AI, Screenshot, QR, and Text Tools. Or press \(settings.globalHotkey.displayString) to summon the same board on the current selection."
        case (true, false):
            return "Select text in any app — a floating toolbar appears with AI, Screenshot, QR, and Text Tools."
        case (false, true):
            return "Select text in any app, then press \(settings.globalHotkey.displayString) to summon the Bello Box board with AI, Screenshot, QR, and Text Tools."
        case (false, false):
            return "Auto hint and the global shortcut are both off. Open Settings to choose how Bello Box appears."
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            header
            statusCard
            howToCard
            Spacer(minLength: 0)
            actions
        }
        .padding(24)
        .frame(width: 600, height: 640)
        .onReceive(timer) { _ in trusted = AccessibilityService.isTrusted }
    }

    private var header: some View {
        HStack(spacing: 14) {
            appIcon
            VStack(alignment: .leading, spacing: 2) {
                Text("Bello Box").font(.system(size: 30, weight: .bold))
                Text(versionText).font(.caption).foregroundStyle(.secondary)
                Text("A toolbox for the text you've selected.")
                    .font(.subheadline).foregroundStyle(.secondary)
            }
            Spacer()
        }
    }

    private var appIcon: some View {
        Group {
            if let icon = NSApp.applicationIconImage {
                Image(nsImage: icon).resizable().frame(width: 80, height: 80)
            } else {
                Image(systemName: "wand.and.stars")
                    .font(.system(size: 34)).foregroundStyle(BoxTheme.accent)
                    .frame(width: 80, height: 80)
                    .background(RoundedRectangle(cornerRadius: 14).fill(BoxTheme.accentSoft))
            }
        }
    }

    private var statusCard: some View {
        VStack(spacing: 0) {
            statusRow(
                ok: settings.isConfigured,
                title: "AI provider",
                okText: "Connected · \(settings.providerKind.shortName)",
                badText: "Not configured",
                action: settings.isConfigured ? nil : ("Set up", onOpenSettings)
            )
            Divider().padding(.leading, 44)
            statusRow(
                ok: trusted,
                title: "Accessibility access",
                okText: "Granted",
                badText: "Needed to read & replace your selection",
                action: trusted ? nil : ("Grant…", {
                    AccessibilityService.requestPermissionPrompt()
                    AccessibilityService.openAccessibilitySettings()
                })
            )
        }
        .padding(.vertical, 4)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(.primary.opacity(0.05)))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(.primary.opacity(0.06), lineWidth: 1))
    }

    private func statusRow(ok: Bool, title: String, okText: String, badText: String, action: (String, () -> Void)?) -> some View {
        HStack(spacing: 12) {
            Image(systemName: ok ? "checkmark.circle.fill" : "exclamationmark.circle.fill")
                .font(.system(size: 18))
                .foregroundStyle(ok ? .green : .orange)
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.callout.weight(.medium))
                Text(ok ? okText : badText).font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            if let action {
                Button(action.0) { action.1() }
                    .buttonStyle(SecondaryButtonStyle())
                    .controlSize(.small)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }

    private var howToCard: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: "cursorarrow.rays")
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(BoxTheme.accent)
                .frame(width: 30, height: 30)
                .background(Circle().fill(BoxTheme.accentSoft))
            VStack(alignment: .leading, spacing: 3) {
                Text("How to use it").font(.callout.weight(.semibold))
                Text(howToText)
                    .font(.caption).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(14)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(BoxTheme.accentSoft.opacity(0.5)))
    }

    private var actions: some View {
        HStack(spacing: 10) {
            Button { onOpenGuide() } label: { Label("Setup Guide", systemImage: "sparkles") }
                .buttonStyle(SecondaryButtonStyle())
            if canCheckForUpdates {
                Button { onCheckForUpdates() } label: { Label("Check for Updates", systemImage: "arrow.triangle.2.circlepath") }
                    .buttonStyle(SecondaryButtonStyle())
            }
            Spacer()
            Button { onOpenSettings() } label: { Label("Settings", systemImage: "gearshape") }
                .buttonStyle(PrimaryButtonStyle())
        }
    }
}
