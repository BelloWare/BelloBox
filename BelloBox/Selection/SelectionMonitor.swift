import AppKit

/// Watches for selection gestures system-wide (a left mouse-up that leaves text
/// selected) and a manual global hotkey, then reports the captured selection.
@MainActor
final class SelectionMonitor {
    private let accessibility: AccessibilityService

    private var mouseUpMonitor: Any?
    private var globalKeyMonitor: Any?
    private var localKeyMonitor: Any?
    private var debounce: DispatchWorkItem?

    /// Virtual key code for "B" — the hotkey is ⌃⌥⌘B.
    private let hotkeyKeyCode: UInt16 = 11

    var isEnabled = true
    var onSelection: ((TextSelection) -> Void)?
    var onHotkey: (() -> Void)?

    init(accessibility: AccessibilityService) {
        self.accessibility = accessibility
    }

    func start() {
        stop()
        mouseUpMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseUp]) { [weak self] _ in
            self?.scheduleSelectionRead()
        }
        globalKeyMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.keyDown]) { [weak self] event in
            self?.handleKey(event)
        }
        localKeyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { [weak self] event in
            self?.handleKey(event)
            return event
        }
    }

    func stop() {
        debounce?.cancel()
        for monitor in [mouseUpMonitor, globalKeyMonitor, localKeyMonitor] {
            if let monitor { NSEvent.removeMonitor(monitor) }
        }
        mouseUpMonitor = nil
        globalKeyMonitor = nil
        localKeyMonitor = nil
    }

    private func scheduleSelectionRead() {
        guard isEnabled else { return }
        debounce?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.isEnabled else { return }
            if let selection = self.accessibility.readSelection() {
                self.onSelection?(selection)
            }
        }
        debounce = work
        // Give the host app a beat to finalize the selection after mouse-up.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.18, execute: work)
    }

    private func handleKey(_ event: NSEvent) {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard flags == [.control, .option, .command], event.keyCode == hotkeyKeyCode else { return }
        onHotkey?()
    }
}
