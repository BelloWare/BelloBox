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

    struct SelectionSource {
        let app: NSRunningApplication
        let window: AXUIElement?
    }

    func selectionSource() -> SelectionSource? {
        guard AXIsProcessTrusted(), let app = NSWorkspace.shared.frontmostApplication,
              app.processIdentifier != ProcessInfo.processInfo.processIdentifier else { return nil }
        let element = AXUIElementCreateApplication(app.processIdentifier)
        AXUIElementSetMessagingTimeout(element, 0.08)
        return SelectionSource(app: app, window: Self.axElement(from: attribute(element, kAXFocusedWindowAttribute)))
    }

    func isCurrent(_ source: SelectionSource) -> Bool {
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == source.app.processIdentifier else { return false }
        guard let window = source.window else { return true }
        let app = AXUIElementCreateApplication(source.app.processIdentifier)
        AXUIElementSetMessagingTimeout(app, 0.08)
        guard let current = Self.axElement(from: attribute(app, kAXFocusedWindowAttribute)) else { return false }
        return CFEqual(window, current)
    }

    func readSelection() -> TextSelection? {
        guard let source = selectionSource() else { return nil }
        return readSelection(from: source)
    }

    /// Read from the captured source application, not a possibly newer
    /// system-wide focus paired with stale frontmost-app metadata.
    func readSelection(from source: SelectionSource) -> TextSelection? {
        guard isCurrent(source) else { return nil }
        let app = AXUIElementCreateApplication(source.app.processIdentifier)
        AXUIElementSetMessagingTimeout(app, 0.08)
        guard let focused = Self.axElement(from: attribute(app, kAXFocusedUIElementAttribute)) else { return nil }
        var candidate: AXUIElement? = focused
        var visited: [AXUIElement] = []
        // Some web content exposes selection on its text container instead of
        // the focused link/static-text child. Walk only that ancestor chain.
        while let element = candidate, visited.count < 6, !visited.contains(where: { CFEqual($0, element) }) {
            visited.append(element)
            var pid: pid_t = 0
            guard AXUIElementGetPid(element, &pid) == .success, pid == source.app.processIdentifier else { break }
            if isProtected(element) { return nil }
            let range = selectedRange(of: element)
            let direct = attribute(element, kAXSelectedTextAttribute) as? String
            let text = SelectionTextResolver.resolve(direct: direct, range: range,
                stringForRange: { range in
                    var cfRange = CFRange(location: range.location, length: range.length)
                    guard let value = AXValueCreate(.cfRange, &cfRange) else { return nil }
                    var result: CFTypeRef?
                    guard AXUIElementCopyParameterizedAttributeValue(element, kAXStringForRangeParameterizedAttribute as CFString, value, &result) == .success else { return nil }
                    return result as? String
                }, value: { self.attribute(element, kAXValueAttribute) as? String })
            if let text, isCurrent(source) {
                return TextSelection(text: text, anchorRect: selectionBounds(of: element),
                    appName: source.app.localizedName, bundleID: source.app.bundleIdentifier, pid: source.app.processIdentifier)
            }
            // Chromium/Electron can keep keyboard focus on a message action
            // while its document has a selection elsewhere. Such a control
            // reports a zero AXSelectedTextRange: that is not a document caret.
            // Ask for the actual document selection before accepting that zero.
            if let selection = markerSelection(of: element, source: source), isCurrent(source) {
                return selection
            }
            // Without a validated document selection, an explicit caret/empty
            // selection is authoritative. Never search other fields/windows.
            if range != nil || direct != nil { return nil }
            candidate = Self.axElement(from: attribute(element, kAXParentAttribute))
        }
        return nil
    }

    /// Text markers identify the selected span in a web document independently
    /// of keyboard focus. No tree search, clipboard access or full-value read.
    func markerSelection(of element: AXUIElement, source: SelectionSource) -> TextSelection? {
        guard !isProtected(element),
              let markerRange = attribute(element, "AXSelectedTextMarkerRange") else { return nil }
        let deadline = Date().addingTimeInterval(0.16)
        let text = SelectionMarkerResolver.resolve(markerRange,
            isSafeEndpoint: { marker in
                guard let owner = Self.axElement(from: self.parameterizedAttribute(element,
                    "AXUIElementForTextMarker", marker)) else { return false }
                return self.isSafeSelectionOwner(owner, source: source, deadline: deadline)
            }, stringForRange: { range in
                guard Date() < deadline else { return nil }
                return self.parameterizedAttribute(element, "AXStringForTextMarkerRange", range) as? String
            })
        guard let text else { return nil }
        var bounds: CGRect?
        if let value = Self.axValue(from: parameterizedAttribute(element, "AXBoundsForTextMarkerRange", markerRange)),
           AXValueGetType(value) == .cgRect {
            var rect = CGRect.zero
            if AXValueGetValue(value, .cgRect, &rect), !rect.isNull, !rect.isInfinite,
               rect.width > 0, rect.height > 0 { bounds = Self.cocoaRect(fromAXRect: rect) }
        }
        return TextSelection(text: text, anchorRect: bounds, appName: source.app.localizedName,
            bundleID: source.app.bundleIdentifier, pid: source.app.processIdentifier)
    }

    private func isProtected(_ element: AXUIElement) -> Bool {
        attribute(element, kAXSubroleAttribute) as? String == kAXSecureTextFieldSubrole
            || attribute(element, "AXProtectedContent") as? Bool == true
    }

    private func isSafeSelectionOwner(_ owner: AXUIElement, source: SelectionSource, deadline: Date) -> Bool {
        // A marker must resolve to this captured window, never another tab's
        // window or another app. Check ancestors too: static text may sit in a
        // protected field. Reject an uninspectable or excessively deep path.
        guard Date() < deadline, let window = source.window,
              let ownerWindow = Self.axElement(from: attribute(owner, kAXWindowAttribute)),
              CFEqual(window, ownerWindow) else { return false }
        var candidate: AXUIElement? = owner
        var visited: [AXUIElement] = []
        while Date() < deadline, let element = candidate, visited.count < 16,
              !visited.contains(where: { CFEqual($0, element) }) {
            visited.append(element)
            AXUIElementSetMessagingTimeout(element, 0.08)
            var pid: pid_t = 0
            guard AXUIElementGetPid(element, &pid) == .success,
                  pid == source.app.processIdentifier, !isProtected(element) else { return false }
            if CFEqual(element, window) || attribute(element, kAXRoleAttribute) as? String == "AXWebArea" { return true }
            candidate = Self.axElement(from: attribute(element, kAXParentAttribute))
        }
        return false
    }

    private func parameterizedAttribute(_ element: AXUIElement, _ name: String, _ parameter: CFTypeRef) -> CFTypeRef? {
        var value: CFTypeRef?
        guard AXUIElementCopyParameterizedAttributeValue(element, name as CFString, parameter, &value) == .success else { return nil }
        return value
    }

    private func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
        return value
    }

    private func selectedRange(of element: AXUIElement) -> NSRange? {
        guard let value = Self.axValue(from: attribute(element, kAXSelectedTextRangeAttribute)), AXValueGetType(value) == .cfRange else { return nil }
        var range = CFRange()
        guard AXValueGetValue(value, .cfRange, &range), range.location >= 0, range.length >= 0 else { return nil }
        return NSRange(location: range.location, length: range.length)
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

/// AX ranges index UTF-16, not Swift Characters. Never substitute the complete
/// field value when its selected range is empty, invalid, or unsupported.
enum SelectionTextResolver {
    static func resolve(direct: String?, range: NSRange?, stringForRange: (NSRange) -> String?, value: () -> String?) -> String? {
        if let range, range.length == 0 { return nil }
        func nonempty(_ text: String?) -> String? {
            guard let text, text.unicodeScalars.contains(where: { !CharacterSet.whitespacesAndNewlines.contains($0) }) else { return nil }
            return text
        }
        if let text = nonempty(direct) { return text }
        guard let range, range.location != NSNotFound, range.length > 0 else { return nil }
        if let text = nonempty(stringForRange(range)) { return text }
        guard let text = value() else { return nil }
        let utf16 = text as NSString
        guard range.location <= utf16.length, range.length <= utf16.length - range.location else { return nil }
        return nonempty(utf16.substring(with: range))
    }
}

/// Markers are opaque CF objects. Validate their type and both endpoints before
/// asking an app for text. A collapsed marker is a caret, even if a buggy app
/// would return a stale string for it.
enum SelectionMarkerResolver {
    static func resolve(_ value: CFTypeRef, isSafeEndpoint: (AXTextMarker) -> Bool,
                        stringForRange: (AXTextMarkerRange) -> String?) -> String? {
        guard CFGetTypeID(value) == AXTextMarkerRangeGetTypeID() else { return nil }
        let range = unsafeBitCast(value, to: AXTextMarkerRange.self)
        let start = AXTextMarkerRangeCopyStartMarker(range)
        let end = AXTextMarkerRangeCopyEndMarker(range)
        guard !CFEqual(start, end), isSafeEndpoint(start), isSafeEndpoint(end),
              let text = stringForRange(range),
              text.unicodeScalars.contains(where: { !CharacterSet.whitespacesAndNewlines.contains($0) }) else { return nil }
        return text
    }
}
