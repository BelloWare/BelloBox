import AppKit
import CoreMedia
import Foundation

final class RecordingOverlayEventStore {
    private let lock = NSLock()
    private var events: [TimedOverlayEvent] = []

    func add(_ event: TimedOverlayEvent) {
        lock.lock()
        events.append(event)
        lock.unlock()
    }

    func activeEvents(at time: CMTime) -> [TimedOverlayEvent] {
        lock.lock()
        defer { lock.unlock() }
        events.removeAll { CMTimeCompare($0.expiresAt, time) < 0 }
        return events.filter {
            CMTimeCompare($0.time, time) <= 0 && CMTimeCompare($0.expiresAt, time) >= 0
        }
    }

    func clear() {
        lock.lock()
        events.removeAll()
        lock.unlock()
    }
}

final class RecordingInputMonitor {
    let eventStore = RecordingOverlayEventStore()

    private let options: RecordingOptions
    private let privacyGuard: PrivacyGuard?
    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?

    init(options: RecordingOptions, privacyGuard: PrivacyGuard? = nil) {
        self.options = options
        self.privacyGuard = privacyGuard
    }

    func start() {
        guard eventTap == nil else { return }
        guard options.clickOverlayMode.isEnabled || options.keystrokeMode != .off else { return }
        guard InputMonitoringPermission.status() == .granted else { return }

        let mask =
            (1 << CGEventType.leftMouseDown.rawValue) |
            (1 << CGEventType.rightMouseDown.rawValue) |
            (1 << CGEventType.otherMouseDown.rawValue) |
            (1 << CGEventType.flagsChanged.rawValue) |
            (1 << CGEventType.keyDown.rawValue)

        let refcon = Unmanaged.passUnretained(self).toOpaque()
        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .listenOnly,
            eventsOfInterest: CGEventMask(mask),
            callback: { _, type, event, refcon in
                guard let refcon else { return Unmanaged.passUnretained(event) }
                let monitor = Unmanaged<RecordingInputMonitor>.fromOpaque(refcon).takeUnretainedValue()
                monitor.handle(type: type, event: event)
                return Unmanaged.passUnretained(event)
            },
            userInfo: refcon
        ) else { return }

        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        eventTap = tap
        runLoopSource = source
    }

    func stop() {
        if let tap = eventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
            CFMachPortInvalidate(tap)
        }
        if let source = runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes)
        }
        eventTap = nil
        runLoopSource = nil
        eventStore.clear()
    }

    private func handle(type: CGEventType, event: CGEvent) {
        switch type {
        case .leftMouseDown, .rightMouseDown, .otherMouseDown:
            addClick(type: type, event: event)
        case .keyDown:
            addKey(event: event)
        case .flagsChanged:
            break
        default:
            break
        }
    }

    private func addClick(type: CGEventType, event: CGEvent) {
        guard options.clickOverlayMode.isEnabled else { return }
        let button: ClickOverlayEvent.Button
        switch type {
        case .leftMouseDown:
            button = .left
        case .rightMouseDown:
            button = .right
        case .otherMouseDown:
            button = .middle
        default:
            button = .other
        }

        let now = CMClockGetTime(CMClockGetHostTimeClock())
        let clickCount = max(1, Int(event.getIntegerValueField(.mouseEventClickState)))
        eventStore.add(
            TimedOverlayEvent(
                id: UUID(),
                time: now,
                kind: .click(
                    ClickOverlayEvent(
                        button: button,
                        clickCount: clickCount,
                        locationInScreenPoints: Self.cocoaPoint(fromEventLocation: event.location)
                    )
                ),
                expiresAt: CMTimeAdd(now, CMTime(seconds: 0.7, preferredTimescale: 600))
            )
        )
    }

    private func addKey(event: CGEvent) {
        guard options.keystrokeMode != .off else { return }
        let now = CMClockGetTime(CMClockGetHostTimeClock())
        let modifierMask: NSEvent.ModifierFlags = [.command, .option, .control, .shift]
        let modifiers = NSEvent.ModifierFlags(rawValue: UInt(event.flags.rawValue))
            .intersection(modifierMask)
        let printable = printableString(from: event)
        let isShortcut = modifiers.contains(.command) || modifiers.contains(.option) || modifiers.contains(.control)
        let isStandalonePrintable = printable != nil && !isShortcut
        let sensitiveState = privacyGuard?.update(now: now) ?? .notSensitive
        let canShowStandalonePrintable = !isStandalonePrintable || (AccessibilityService.isTrusted && !sensitiveState.isSensitive)

        let kind: OverlayEventKind?
        switch options.keystrokeMode {
        case .off:
            kind = nil
        case .shortcutsOnly:
            kind = isShortcut
                ? .keystroke(keyEvent(
                    label: shortcutLabel(modifiers: modifiers, event: event, printable: printable),
                    isShortcut: true,
                    isPrintable: false,
                    modifiers: modifiers
                ))
                : nil
        case .maskedPrintable:
            if isShortcut {
                kind = .keystroke(keyEvent(
                    label: shortcutLabel(modifiers: modifiers, event: event, printable: printable),
                    isShortcut: true,
                    isPrintable: false,
                    modifiers: modifiers
                ))
            } else if isStandalonePrintable, sensitiveState.isSensitive {
                kind = .secureTypingHidden
            } else if isStandalonePrintable, !canShowStandalonePrintable {
                kind = nil
            } else {
                kind = .keystroke(keyEvent(
                    label: printable == nil ? specialKeyLabel(for: event) : "•",
                    isShortcut: false,
                    isPrintable: isStandalonePrintable,
                    modifiers: modifiers
                ))
            }
        case .allKeys:
            if isShortcut {
                kind = .keystroke(keyEvent(
                    label: shortcutLabel(modifiers: modifiers, event: event, printable: printable),
                    isShortcut: true,
                    isPrintable: false,
                    modifiers: modifiers
                ))
            } else if isStandalonePrintable, sensitiveState.isSensitive {
                kind = .secureTypingHidden
            } else if isStandalonePrintable, !canShowStandalonePrintable {
                kind = nil
            } else {
                kind = .keystroke(keyEvent(
                    label: shortcutLabel(modifiers: modifiers, event: event, printable: printable),
                    isShortcut: false,
                    isPrintable: isStandalonePrintable,
                    modifiers: modifiers
                ))
            }
        }

        guard let kind, !kind.isEmpty else { return }
        eventStore.add(
            TimedOverlayEvent(
                id: UUID(),
                time: now,
                kind: kind,
                expiresAt: CMTimeAdd(now, CMTime(seconds: 1.15, preferredTimescale: 600))
            )
        )
    }

    private func keyEvent(
        label: String,
        isShortcut: Bool,
        isPrintable: Bool,
        modifiers: NSEvent.ModifierFlags
    ) -> KeystrokeOverlayEvent {
        KeystrokeOverlayEvent(
            displayLabel: label,
            isShortcut: isShortcut,
            isPrintable: isPrintable,
            modifiers: modifiers
        )
    }

    private func shortcutLabel(modifiers: NSEvent.ModifierFlags, event: CGEvent, printable: String?) -> String {
        let parts = [
            modifiers.contains(.control) ? "⌃" : nil,
            modifiers.contains(.option) ? "⌥" : nil,
            modifiers.contains(.shift) ? "⇧" : nil,
            modifiers.contains(.command) ? "⌘" : nil,
            printable?.uppercased() ?? specialKeyLabel(for: event)
        ].compactMap { $0 }
        return parts.joined()
    }

    private func printableString(from event: CGEvent) -> String? {
        var length = 0
        var chars = [UniChar](repeating: 0, count: 8)
        event.keyboardGetUnicodeString(
            maxStringLength: chars.count,
            actualStringLength: &length,
            unicodeString: &chars
        )
        guard length > 0 else { return nil }
        let string = String(utf16CodeUnits: chars, count: length)
        guard string.rangeOfCharacter(from: .newlines) == nil,
              string.rangeOfCharacter(from: .controlCharacters) == nil,
              !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return nil }
        return String(string.prefix(2))
    }

    private func specialKeyLabel(for event: CGEvent) -> String {
        let keyCode = CGKeyCode(event.getIntegerValueField(.keyboardEventKeycode))
        switch keyCode {
        case 36: return "↩"
        case 48: return "⇥"
        case 49: return "Space"
        case 51: return "⌫"
        case 53: return "Esc"
        case 117: return "⌦"
        case 123: return "←"
        case 124: return "→"
        case 125: return "↓"
        case 126: return "↑"
        default: return "Key"
        }
    }

    private static func cocoaPoint(fromEventLocation location: CGPoint) -> CGPoint {
        let primaryMaxY = NSScreen.screens.first(where: { $0.frame.origin == .zero })?.frame.maxY
            ?? NSScreen.main?.frame.maxY
            ?? 0
        return CGPoint(x: location.x, y: primaryMaxY - location.y)
    }
}
