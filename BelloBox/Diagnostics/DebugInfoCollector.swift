import AppKit
import CoreGraphics
import Foundation

enum DebugInfoCollector {
    static func report(settings: AppSettings, diagnosticsLogTail: String? = CaptureDiagnostics.readLogTail()) -> String {
        var sections: [String] = []
        sections.append(appSection())
        sections.append(permissionSection())
        sections.append(settingsSection(settings))
        sections.append(screenSection())
        sections.append(diagnosticsSection(diagnosticsLogTail))
        return sections.joined(separator: "\n\n") + "\n"
    }

    private static func appSection() -> String {
        let bundle = Bundle.main
        let shortVersion = bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?"
        let build = bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "?"
        let bundleID = bundle.bundleIdentifier ?? "?"
        return lines(
            title: "App",
            [
                "generatedAt=\(ISO8601DateFormatter().string(from: Date()))",
                "name=Bello Box",
                "bundleID=\(bundleID)",
                "version=\(shortVersion)",
                "build=\(build)",
                "macOS=\(ProcessInfo.processInfo.operatingSystemVersionString)",
                "processID=\(ProcessInfo.processInfo.processIdentifier)",
            ]
        )
    }

    private static func permissionSection() -> String {
        lines(
            title: "Permissions",
            [
                "accessibility=\(AccessibilityService.isTrusted ? "granted" : "notGranted")",
                "screenRecording=\(ScreenCapturePermission.isTrusted ? "granted" : "notGranted")",
                "inputMonitoring=\(permissionStatus(InputMonitoringPermission.status()))",
            ]
        )
    }

    private static func settingsSection(_ settings: AppSettings) -> String {
        lines(
            title: "Settings",
            [
                "provider=\(settings.providerKind.rawValue)",
                "providerConfigured=\(settings.isConfigured)",
                "openAIAPIKind=\(settings.openAIAPIKind.rawValue)",
                "temperatureMode=\(settings.temperatureMode.rawValue)",
                "floatingButtonEnabled=\(settings.floatingButtonEnabled)",
                "globalHotkeyEnabled=\(settings.globalHotkeyEnabled)",
                "globalHotkey=\(settings.globalHotkey.displayString)",
                "screenshotHotkeyEnabled=\(settings.screenshotHotkeyEnabled)",
                "screenshotHotkey=\(settings.screenshotHotkey.displayString)",
                "recordingHotkeyEnabled=\(settings.recordingHotkeyEnabled)",
                "recordingHotkey=\(settings.recordingHotkey.displayString)",
                "screenshotIncludeCursor=\(settings.screenshotIncludeCursor)",
                "screenshotAutoCopy=\(settings.screenshotAutoCopy)",
                "screenshotDefaultMode=\(settings.screenshotDefaultMode.rawValue)",
                "captureDiagnosticsEnabled=\(settings.captureDiagnosticsEnabled)",
                "scrollingScreenshotMaxFrames=\(settings.scrollingScreenshotMaxFrames)",
                "scrollingScreenshotAutoCompact=\(settings.scrollingScreenshotAutoCompact)",
                "recordingIncludeCursor=\(settings.recordingIncludeCursor)",
                "recordingAudioSource=\(settings.recordingAudioSource.rawValue)",
                "recordingQuality=\(settings.recordingQualityPreset.rawValue)",
                "launchAtLoginEnabled=\(settings.launchAtLoginEnabled)",
            ]
        )
    }

    private static func screenSection() -> String {
        let mouse = NSEvent.mouseLocation
        let mainDisplayID = CGMainDisplayID()
        var items: [String] = [
            "mouse=\(format(mouse))",
            "nsscreenCount=\(NSScreen.screens.count)",
            "cgOnlineDisplayCount=\(onlineDisplayIDs().count)",
            "cgMainDisplayID=\(mainDisplayID)",
        ]

        for (index, screen) in NSScreen.screens.enumerated() {
            let displayID = ScreenCoordinateSpace.displayID(for: screen)
            let pixelSize = displayID.map { ScreenCoordinateSpace.displayPixelSize(for: $0, fallbackScreen: screen) } ?? .zero
            let isMain = displayID == mainDisplayID
            items.append(
                "nsscreen[\(index)]=displayID:\(displayID.map(String.init) ?? "nil"),name:\(screen.localizedName),frame:\(format(screen.frame)),visibleFrame:\(format(screen.visibleFrame)),scale:\(ScreenCoordinateSpace.backingScale(for: screen)),pixels:\(format(pixelSize)),isMain:\(isMain)"
            )
        }

        for (index, displayID) in onlineDisplayIDs().enumerated() {
            items.append(
                "cgDisplay[\(index)]=displayID:\(displayID),bounds:\(format(CGDisplayBounds(displayID))),pixels:\(CGDisplayPixelsWide(displayID))x\(CGDisplayPixelsHigh(displayID)),active:\(CGDisplayIsActive(displayID) != 0),asleep:\(CGDisplayIsAsleep(displayID) != 0),rotation:\(CGDisplayRotation(displayID))"
            )
        }

        return lines(title: "Screens", items)
    }

    private static func diagnosticsSection(_ diagnosticsLogTail: String?) -> String {
        let header = lines(
            title: "Capture Diagnostics",
            [
                "logPath=\(CaptureDiagnostics.logURL.path)",
                "logPresent=\(diagnosticsLogTail != nil)",
            ]
        )
        guard let diagnosticsLogTail, !diagnosticsLogTail.isEmpty else {
            return header + "\n(no capture diagnostics log found)"
        }
        return header + "\n" + diagnosticsLogTail
    }

    private static func onlineDisplayIDs() -> [CGDirectDisplayID] {
        var count: UInt32 = 0
        guard CGGetOnlineDisplayList(0, nil, &count) == .success, count > 0 else { return [] }
        var displays = Array(repeating: CGDirectDisplayID(), count: Int(count))
        let status = displays.withUnsafeMutableBufferPointer { buffer in
            CGGetOnlineDisplayList(count, buffer.baseAddress, &count)
        }
        guard status == .success else { return [] }
        return Array(displays.prefix(Int(count)))
    }

    private static func permissionStatus(_ status: PermissionStatus) -> String {
        switch status {
        case .granted: return "granted"
        case .denied: return "denied"
        case .notDetermined: return "notDetermined"
        case let .unavailable(message): return "unavailable(\(message))"
        }
    }

    private static func lines(title: String, _ values: [String]) -> String {
        (["== \(title) =="] + values).joined(separator: "\n")
    }

    private static func format(_ point: CGPoint) -> String {
        "\(round(point.x)),\(round(point.y))"
    }

    private static func format(_ rect: CGRect) -> String {
        "\(round(rect.origin.x)),\(round(rect.origin.y)),\(round(rect.size.width)),\(round(rect.size.height))"
    }

    private static func format(_ size: CGSize) -> String {
        "\(round(size.width))x\(round(size.height))"
    }

    private static func round(_ value: CGFloat) -> String {
        let rounded = value.rounded()
        if rounded == value {
            return String(Int(rounded))
        }
        return String(format: "%.2f", Double(value))
    }
}
