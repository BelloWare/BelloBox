import AppKit
import SwiftUI

@MainActor
final class WorldClockWindowController: NSObject, NSWindowDelegate {
    private var panel: WorldClockPanel?
    private var viewModel: WorldClockViewModel?

    func show(
        settings: AppSettings,
        handoff: WorldClockHandoff? = nil,
        onOpenSettings: @escaping () -> Void
    ) {
        if let panel, let viewModel {
            if let handoff { viewModel.adopt(handoff) }
            AppActivation.bringAppForward()
            AppWindowChrome.place(panel, on: panel.screen, centered: false)
            panel.makeKeyAndOrderFront(nil)
            return
        }

        var preferences = WorldClockPreferencesStore()
#if DEBUG
        if let fixture = WorldClockPreferencesStore.e2eFixture() { preferences = fixture }
#endif
        let viewModel = WorldClockViewModel(settings: settings, seedDate: handoff?.instant, preferences: preferences)
        if let handoff { viewModel.adopt(handoff) }
        let rootView = WorldClockView(viewModel: viewModel, onOpenSettings: onOpenSettings)
        let hosting = NSHostingController(rootView: ToolViewport(minimumSize: NSSize(width: 780, height: 640)) { rootView }
            .workspaceBackground().windowSurfacePreferences(settings))
        let panel = WorldClockPanel(contentViewController: hosting)
        panel.delegate = self
        AppWindowChrome.size(panel, content: NSSize(width: 920, height: 740), minimum: NSSize(width: 780, height: 640))
        panel.setFrameAutosaveName("BelloBoxWorldClockWindow")
        AppWindowChrome.place(panel, on: Self.hasSavedFrame ? panel.screen : nil, centered: !Self.hasSavedFrame)

        self.viewModel = viewModel
        self.panel = panel

        AppActivation.bringAppForward()
        panel.makeKeyAndOrderFront(nil)
#if DEBUG
        writeE2EMarker(panel: panel, viewModel: viewModel)
#endif
    }

    func windowWillClose(_ notification: Notification) {
        viewModel?.cancelAI()
        viewModel = nil
        panel = nil
    }

    private static var hasSavedFrame: Bool {
        UserDefaults.standard.string(forKey: "NSWindow Frame BelloBoxWorldClockWindow") != nil
    }

#if DEBUG
    private func writeE2EMarker(panel: NSPanel, viewModel: WorldClockViewModel) {
        guard
            let path = ProcessInfo.processInfo.environment["BELLOBOX_E2E_WORLD_CLOCK_MARKER"],
            !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return }

        panel.displayIfNeeded()
        let payload = [
            "kind=world-clock-window",
            "title=\(panel.title)",
            "visible=\(panel.isVisible)",
            "canJoinAllSpaces=\(panel.collectionBehavior.contains(.canJoinAllSpaces))",
            "fullScreenAuxiliary=\(panel.collectionBehavior.contains(.fullScreenAuxiliary))",
            "hidesOnDeactivate=\(panel.hidesOnDeactivate)",
            "selectedInstant=\(viewModel.selectedInstant.timeIntervalSince1970)",
            "anchorZoneID=\(viewModel.anchorZoneID)",
            "zoneIDs=\(viewModel.zoneIDs.joined(separator: ","))",
            "timelineBandCount=\(viewModel.timelineQualities.count)",
            "frame=\(NSStringFromRect(panel.frame))",
        ].joined(separator: "\n")
        let url = URL(fileURLWithPath: path)
        do {
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try payload.write(to: url, atomically: true, encoding: .utf8)
        } catch {
            NSLog("Bello Box world-clock E2E marker failed: \(error.localizedDescription)")
        }
    }
#endif
}

final class WorldClockPanel: NSPanel {
    init(contentViewController: NSViewController) {
        super.init(
            contentRect: .zero,
            styleMask: AppWindowChrome.styleMask,
            backing: .buffered,
            defer: false
        )
        self.contentViewController = contentViewController
        AppWindowChrome.apply(to: self, title: "World Clock")
        isFloatingPanel = true
        level = .floating
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        animationBehavior = .default
    }
}
