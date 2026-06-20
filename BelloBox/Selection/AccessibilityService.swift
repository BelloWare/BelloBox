import AppKit
import ApplicationServices

/// A captured selection plus the context needed to act on it later.
struct TextSelection: Equatable {
    var text: String
    /// Selection bounds in Cocoa global screen coordinates (bottom-left origin),
    /// when the focused app exposes them. Nil falls back to the mouse location.
    var anchorRect: CGRect?
    var appName: String?
    var bundleID: String?
    var pid: pid_t?
}

/// Reads selected text from the frontmost application via the Accessibility API
/// and can paste a replacement back. Requires Accessibility permission.
final class AccessibilityService {
    // MARK: - Permission

    static var isTrusted: Bool { AXIsProcessTrusted() }

    static func requestPermissionPrompt() {
        let key = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
        _ = AXIsProcessTrustedWithOptions([key: true] as CFDictionary)
    }

    static func openAccessibilitySettings() {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") else { return }
        NSWorkspace.shared.open(url)
    }

    // MARK: - Reading selection

    /// Reads the current selection from the focused UI element. Returns nil when
    /// nothing is selected or the app does not expose its selection over AX.
    func readSelection() -> TextSelection? {
        guard AXIsProcessTrusted() else { return nil }
        guard let element = focusedElement() else { return nil }

        var textRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, kAXSelectedTextAttribute as CFString, &textRef) == .success,
              let text = textRef as? String,
              !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else { return nil }

        let front = NSWorkspace.shared.frontmostApplication
        return TextSelection(
            text: text,
            anchorRect: selectionBounds(of: element),
            appName: front?.localizedName,
            bundleID: front?.bundleIdentifier,
            pid: front?.processIdentifier
        )
    }

    private func focusedElement() -> AXUIElement? {
        let system = AXUIElementCreateSystemWide()
        var focused: CFTypeRef?
        guard AXUIElementCopyAttributeValue(system, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
              let element = Self.axElement(from: focused)
        else { return nil }
        return element
    }

    private func selectionBounds(of element: AXUIElement) -> CGRect? {
        var rangeRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, kAXSelectedTextRangeAttribute as CFString, &rangeRef) == .success,
              let rangeValue = rangeRef,
              CFGetTypeID(rangeValue) == AXValueGetTypeID()
        else { return nil }

        var boundsRef: CFTypeRef?
        guard AXUIElementCopyParameterizedAttributeValue(
            element,
            kAXBoundsForRangeParameterizedAttribute as CFString,
            rangeValue,
            &boundsRef
        ) == .success,
            let boundsValue = boundsRef,
            CFGetTypeID(boundsValue) == AXValueGetTypeID()
        else { return nil }

        var rect = CGRect.zero
        guard let axValue = Self.axValue(from: boundsValue),
              AXValueGetValue(axValue, .cgRect, &rect)
        else { return nil }
        guard rect.width > 0 || rect.height > 0 else { return nil }
        return Self.cocoaRect(fromAXRect: rect)
    }

    /// Converts an Accessibility rect (top-left origin, primary screen) into a
    /// Cocoa global screen rect (bottom-left origin).
    static func cocoaRect(fromAXRect ax: CGRect) -> CGRect {
        ScreenCoordinateSpace.topLeftRectToCocoaRect(ax)
    }

    // MARK: - Acting on selection

    /// Copies the current selection via a synthesized ⌘C, used as a fallback
    /// when the app does not expose AX selected text. Briefly blocks the caller.
    func copySelectionViaPasteboard(timeout: TimeInterval = 0.3) -> String? {
        let pasteboard = NSPasteboard.general
        let previousChange = pasteboard.changeCount
        Self.postCommandKey(Self.keyC)
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if pasteboard.changeCount != previousChange {
                return pasteboard.string(forType: .string)
            }
            usleep(20_000)
        }
        return nil
    }

    /// Places `text` on the pasteboard, re-activates the target app, and pastes
    /// it (⌘V), replacing the previous selection.
    func replaceSelection(with text: String, pid: pid_t?) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)

        if let pid, let app = NSRunningApplication(processIdentifier: pid) {
            app.activate(options: [])
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            Self.postCommandKey(Self.keyV)
        }
    }

    // MARK: - Synthetic keys

    static let keyC: CGKeyCode = 0x08
    static let keyV: CGKeyCode = 0x09

    static func postCommandKey(_ key: CGKeyCode) {
        let source = CGEventSource(stateID: .combinedSessionState)
        let down = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: true)
        down?.flags = .maskCommand
        let up = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: false)
        up?.flags = .maskCommand
        down?.post(tap: .cgAnnotatedSessionEventTap)
        up?.post(tap: .cgAnnotatedSessionEventTap)
    }

    static func axElement(from value: CFTypeRef?) -> AXUIElement? {
        guard let value, CFGetTypeID(value) == AXUIElementGetTypeID() else { return nil }
        return unsafeBitCast(value, to: AXUIElement.self)
    }

    static func axValue(from value: CFTypeRef?) -> AXValue? {
        guard let value, CFGetTypeID(value) == AXValueGetTypeID() else { return nil }
        return unsafeBitCast(value, to: AXValue.self)
    }
}
