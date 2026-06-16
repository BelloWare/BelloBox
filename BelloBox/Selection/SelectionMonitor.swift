import AppKit
import Carbon

/// Watches for selection gestures system-wide (a left mouse-up that leaves text
/// selected) and a manual global hotkey, then reports the captured selection.
@MainActor
final class SelectionMonitor {
    private let accessibility: AccessibilityService

    private var mouseUpMonitor: Any?
    private var hotkeyRef: EventHotKeyRef?
    private var hotkeyHandlerRef: EventHandlerRef?
    private var debounce: DispatchWorkItem?
    private var isRunning = false

    private let hotkeySignature: OSType = 0x42425848 // "BBXH"
    private let hotkeyID: UInt32 = 1

    var selectionMonitoringEnabled = true
    var hotkeyEnabled = true {
        didSet { refreshHotkeyRegistrationIfNeeded() }
    }
    var hotkey = GlobalHotkey.default {
        didSet { refreshHotkeyRegistrationIfNeeded() }
    }
    var onSelection: ((TextSelection) -> Void)?
    var onHotkey: (() -> Void)?

    init(accessibility: AccessibilityService) {
        self.accessibility = accessibility
    }

    func start() {
        stop()
        isRunning = true
        mouseUpMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseUp]) { [weak self] _ in
            self?.scheduleSelectionRead()
        }
        installHotkeyIfNeeded()
    }

    func stop() {
        debounce?.cancel()
        if let mouseUpMonitor {
            NSEvent.removeMonitor(mouseUpMonitor)
        }
        mouseUpMonitor = nil
        unregisterHotkey()
        isRunning = false
    }

    private func scheduleSelectionRead() {
        guard selectionMonitoringEnabled else { return }
        debounce?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.selectionMonitoringEnabled else { return }
            if let selection = self.accessibility.readSelection() {
                self.onSelection?(selection)
            }
        }
        debounce = work
        // Give the host app a beat to finalize the selection after mouse-up.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.18, execute: work)
    }

    private func handleRegisteredHotkey() {
        onHotkey?()
    }

    private func refreshHotkeyRegistrationIfNeeded() {
        guard isRunning else { return }
        unregisterHotkey()
        installHotkeyIfNeeded()
    }

    private func installHotkeyIfNeeded() {
        guard hotkeyEnabled, hotkey.isValid else { return }

        var eventType = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed)
        )
        let userData = Unmanaged.passUnretained(self).toOpaque()
        let handlerStatus = InstallEventHandler(
            GetApplicationEventTarget(),
            Self.hotkeyEventHandler,
            1,
            &eventType,
            userData,
            &hotkeyHandlerRef
        )
        guard handlerStatus == noErr else {
            NSLog("Bello Box failed to install hotkey handler: \(handlerStatus)")
            hotkeyHandlerRef = nil
            return
        }

        var carbonHotkeyID = EventHotKeyID(signature: hotkeySignature, id: hotkeyID)
        let registerStatus = RegisterEventHotKey(
            UInt32(hotkey.keyCode),
            hotkey.carbonModifiers,
            carbonHotkeyID,
            GetApplicationEventTarget(),
            0,
            &hotkeyRef
        )
        guard registerStatus == noErr else {
            NSLog("Bello Box failed to register hotkey \(hotkey.displayString): \(registerStatus)")
            unregisterHotkey()
            return
        }
    }

    private func unregisterHotkey() {
        if let hotkeyRef {
            UnregisterEventHotKey(hotkeyRef)
            self.hotkeyRef = nil
        }
        if let hotkeyHandlerRef {
            RemoveEventHandler(hotkeyHandlerRef)
            self.hotkeyHandlerRef = nil
        }
    }

    private static let hotkeyEventHandler: EventHandlerUPP = { _, event, userData in
        guard let event, let userData else { return noErr }

        var carbonHotkeyID = EventHotKeyID()
        let status = GetEventParameter(
            event,
            EventParamName(kEventParamDirectObject),
            EventParamType(typeEventHotKeyID),
            nil,
            MemoryLayout<EventHotKeyID>.size,
            nil,
            &carbonHotkeyID
        )
        guard status == noErr, carbonHotkeyID.id == 1 else { return noErr }

        let monitor = Unmanaged<SelectionMonitor>.fromOpaque(userData).takeUnretainedValue()
        Task { @MainActor in
            monitor.handleRegisteredHotkey()
        }
        return noErr
    }
}
