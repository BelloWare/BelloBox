import AppKit
import Carbon

struct GlobalHotkey: Equatable {
    static let `default` = GlobalHotkey(keyCode: 11, modifiers: [.control, .option, .command])
    static let defaultScreenshot = GlobalHotkey(keyCode: 1, modifiers: [.control, .option, .command])
    static let defaultRecording = GlobalHotkey(keyCode: 15, modifiers: [.control, .option, .command])

    let keyCode: UInt16
    let modifiers: NSEvent.ModifierFlags

    static func == (lhs: GlobalHotkey, rhs: GlobalHotkey) -> Bool {
        lhs.keyCode == rhs.keyCode && lhs.normalizedModifiers == rhs.normalizedModifiers
    }

    var displayString: String {
        let orderedModifiers: [(NSEvent.ModifierFlags, String)] = [
            (.control, "⌃"),
            (.option, "⌥"),
            (.shift, "⇧"),
            (.command, "⌘")
        ]
        let prefix = orderedModifiers
            .filter { normalizedModifiers.contains($0.0) }
            .map(\.1)
            .joined()
        return prefix + Self.keyName(for: keyCode)
    }

    var isValid: Bool {
        !Self.modifierKeys.contains(keyCode)
            && !normalizedModifiers.isEmpty
    }

    var carbonModifiers: UInt32 {
        var result: UInt32 = 0
        if normalizedModifiers.contains(.command) { result |= UInt32(cmdKey) }
        if normalizedModifiers.contains(.option) { result |= UInt32(optionKey) }
        if normalizedModifiers.contains(.control) { result |= UInt32(controlKey) }
        if normalizedModifiers.contains(.shift) { result |= UInt32(shiftKey) }
        return result
    }

    func matches(_ event: NSEvent) -> Bool {
        guard event.keyCode == keyCode, !event.isARepeat else { return false }
        let pressed = event.modifierFlags.intersection(Self.shortcutModifiers)
        return pressed == normalizedModifiers
    }

    static func from(event: NSEvent) -> GlobalHotkey? {
        let pressed = event.modifierFlags.intersection(shortcutModifiers)
        let hotkey = GlobalHotkey(keyCode: event.keyCode, modifiers: pressed)
        return hotkey.isValid ? hotkey : nil
    }

    private var normalizedModifiers: NSEvent.ModifierFlags {
        modifiers.intersection(Self.shortcutModifiers)
    }

    private static let shortcutModifiers: NSEvent.ModifierFlags = [.control, .option, .shift, .command]

    private static let modifierKeys: Set<UInt16> = [
        53, // Esc is reserved for cancel/close throughout the app.
        54, 55, 56, 57, 58, 59, 60, 61, 62
    ]

    private static func keyName(for keyCode: UInt16) -> String {
        keyNames[keyCode] ?? "Key \(keyCode)"
    }

    private static let keyNames: [UInt16: String] = [
        0: "A", 1: "S", 2: "D", 3: "F", 4: "H", 5: "G", 6: "Z", 7: "X",
        8: "C", 9: "V", 11: "B", 12: "Q", 13: "W", 14: "E", 15: "R",
        16: "Y", 17: "T", 18: "1", 19: "2", 20: "3", 21: "4", 22: "6",
        23: "5", 24: "=", 25: "9", 26: "7", 27: "-", 28: "8", 29: "0",
        30: "]", 31: "O", 32: "U", 33: "[", 34: "I", 35: "P", 36: "Return",
        37: "L", 38: "J", 39: "'", 40: "K", 41: ";", 42: "\\", 43: ",",
        44: "/", 45: "N", 46: "M", 47: ".", 48: "Tab", 49: "Space",
        50: "`", 51: "Delete", 53: "Esc", 65: ".", 67: "*", 69: "+", 71: "Clear",
        75: "/", 76: "Enter", 78: "-", 81: "=", 82: "0", 83: "1", 84: "2",
        85: "3", 86: "4", 87: "5", 88: "6", 89: "7", 91: "8", 92: "9",
        96: "F5", 97: "F6", 98: "F7", 99: "F3", 100: "F8", 101: "F9",
        103: "F11", 105: "F13", 106: "F16", 107: "F14", 109: "F10",
        111: "F12", 113: "F15", 114: "Help", 115: "Home", 116: "Page Up",
        117: "Forward Delete", 118: "F4", 119: "End", 120: "F2",
        121: "Page Down", 122: "F1", 123: "←", 124: "→", 125: "↓", 126: "↑"
    ]
}
