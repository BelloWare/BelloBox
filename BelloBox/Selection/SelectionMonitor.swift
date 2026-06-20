import AppKit
import Carbon

/// Watches for selection gestures system-wide (a left mouse-up that leaves text
/// selected) and a manual global hotkey, then reports the captured selection.
@MainActor
final class SelectionMonitor {
    private let accessibility: AccessibilityService

    private var mouseUpMonitor: Any?
    private var hotkeyRefs: [UInt32: EventHotKeyRef] = [:]
    private var hotkeyHandlerRef: EventHandlerRef?
    private var debounce: DispatchWorkItem?
    private var isRunning = false

    private let hotkeySignature: OSType = 0x42425848 // "BBXH"
    private let boardHotkeyID: UInt32 = 1
    private let screenshotHotkeyID: UInt32 = 2
    private let recordingHotkeyID: UInt32 = 3

    var selectionMonitoringEnabled = true
    var hotkeyEnabled = true {
        didSet { refreshHotkeyRegistrationIfNeeded() }
    }
    var hotkey = GlobalHotkey.default {
        didSet { refreshHotkeyRegistrationIfNeeded() }
    }
    var screenshotHotkeyEnabled = false {
        didSet { refreshHotkeyRegistrationIfNeeded() }
    }
    var screenshotHotkey = GlobalHotkey.defaultScreenshot {
        didSet { refreshHotkeyRegistrationIfNeeded() }
    }
    var recordingHotkeyEnabled = false {
        didSet { refreshHotkeyRegistrationIfNeeded() }
    }
    var recordingHotkey = GlobalHotkey.defaultRecording {
        didSet { refreshHotkeyRegistrationIfNeeded() }
    }
    var onSelection: ((TextSelection) -> Void)?
    var onHotkey: (() -> Void)?
    var onScreenshotHotkey: (() -> Void)?
    var onRecordingHotkey: (() -> Void)?

    init(accessibility: AccessibilityService) {
        self.accessibility = accessibility
    }

    func start() {
        stop()
        isRunning = true
        mouseUpMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseUp]) { [weak self] _ in
            self?.scheduleSelectionRead()
        }
        installHotkeysIfNeeded()
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

    private func handleRegisteredHotkey(id: UInt32) {
        switch id {
        case boardHotkeyID:
            onHotkey?()
        case screenshotHotkeyID:
            onScreenshotHotkey?()
        case recordingHotkeyID:
            onRecordingHotkey?()
        default:
            break
        }
    }

    private func refreshHotkeyRegistrationIfNeeded() {
        guard isRunning else { return }
        unregisterHotkey()
        installHotkeysIfNeeded()
    }

    private func installHotkeysIfNeeded() {
        guard (hotkeyEnabled && hotkey.isValid)
            || (screenshotHotkeyEnabled && screenshotHotkey.isValid)
            || (recordingHotkeyEnabled && recordingHotkey.isValid)
        else { return }

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

        if hotkeyEnabled, hotkey.isValid {
            register(hotkey, id: boardHotkeyID, label: "board")
        }
        if screenshotHotkeyEnabled, screenshotHotkey.isValid {
            if hotkeyEnabled, screenshotHotkey == hotkey {
                NSLog("Bello Box screenshot hotkey matches the board hotkey; screenshot hotkey was not registered.")
            } else {
                register(screenshotHotkey, id: screenshotHotkeyID, label: "screenshot")
            }
        }
        if recordingHotkeyEnabled, recordingHotkey.isValid {
            if hotkeyEnabled, recordingHotkey == hotkey {
                NSLog("Bello Box recording hotkey matches the board hotkey; recording hotkey was not registered.")
            } else if screenshotHotkeyEnabled, recordingHotkey == screenshotHotkey {
                NSLog("Bello Box recording hotkey matches the screenshot hotkey; recording hotkey was not registered.")
            } else {
                register(recordingHotkey, id: recordingHotkeyID, label: "recording")
            }
        }

        if hotkeyRefs.isEmpty { unregisterHotkey() }
    }

    private func register(_ hotkey: GlobalHotkey, id: UInt32, label: String) {
        var ref: EventHotKeyRef?
        var carbonHotkeyID = EventHotKeyID(signature: hotkeySignature, id: id)
        let registerStatus = RegisterEventHotKey(
            UInt32(hotkey.keyCode),
            hotkey.carbonModifiers,
            carbonHotkeyID,
            GetApplicationEventTarget(),
            0,
            &ref
        )
        guard registerStatus == noErr, let ref else {
            NSLog("Bello Box failed to register \(label) hotkey \(hotkey.displayString): \(registerStatus)")
            return
        }
        hotkeyRefs[id] = ref
    }

    private func unregisterHotkey() {
        for ref in hotkeyRefs.values {
            UnregisterEventHotKey(ref)
        }
        hotkeyRefs.removeAll()
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
        guard status == noErr else { return noErr }

        let monitor = Unmanaged<SelectionMonitor>.fromOpaque(userData).takeUnretainedValue()
        Task { @MainActor in
            monitor.handleRegisteredHotkey(id: carbonHotkeyID.id)
        }
        return noErr
    }
}
