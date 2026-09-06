import AppKit
import SwiftUI

@MainActor
final class WorldClockWindowController: NSObject, NSWindowDelegate {
    private var panel: WorldClockPanel?
    private var viewModel: WorldClockViewModel?

    func show(
        settings: AppSettings,
        seedDate: Date? = nil,
        onOpenSettings: @escaping () -> Void
    ) {
        if let panel, let viewModel {
            if let seedDate { viewModel.focus(on: seedDate) }
            AppActivation.bringAppForward()
            panel.makeKeyAndOrderFront(nil)
            return
        }

        let viewModel = WorldClockViewModel(settings: settings, seedDate: seedDate)
        let rootView = WorldClockView(viewModel: viewModel, onOpenSettings: onOpenSettings)
        let hosting = NSHostingController(rootView: rootView)
        let panel = WorldClockPanel(contentViewController: hosting)
        panel.delegate = self
        panel.titlebarAppearsTransparent = true
        panel.backgroundColor = NSColor(BoxTheme.background)
        panel.setContentSize(NSSize(width: 860, height: 650))
        panel.minSize = NSSize(width: 760, height: 590)
        panel.setFrameAutosaveName("BelloBoxWorldClockWindow")
        if !Self.hasSavedFrame {
            panel.center()
        }

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
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .utilityWindow],
            backing: .buffered,
            defer: false
        )
        self.contentViewController = contentViewController
        title = "World Clock"
        isFloatingPanel = true
        level = .floating
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        animationBehavior = .utilityWindow
    }
}
