import AppKit
import SwiftUI

final class LauncherSearchTextField: NSTextField {
    override var needsPanelToBecomeKey: Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard window != nil else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, let window = self.window, window.isVisible else { return }
            if window.firstResponder !== self.currentEditor() { window.makeFirstResponder(self) }
        }
    }
}

/// Native first-responder ownership is reliable when opened from another app's hotkey.
struct LauncherSearchField: NSViewRepresentable {
    @Binding var text: String
    var onMove: (Int) -> Void
    var onSubmit: () -> Void
    var onEscape: () -> Void
    var onReady: (LauncherSearchTextField) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeNSView(context: Context) -> LauncherSearchTextField {
        let field = LauncherSearchTextField()
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = .systemFont(ofSize: 19, weight: .regular)
        field.textColor = .labelColor
        field.placeholderString = "Search tools and commands…"
        field.maximumNumberOfLines = 1
        field.cell?.isScrollable = true
        field.identifier = NSUserInterfaceItemIdentifier("launcherSearch")
        field.setAccessibilityIdentifier("launcherSearch")
        field.setAccessibilityLabel("Search tools and commands")
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        field.delegate = context.coordinator
        onReady(field)
        return field
    }
    func updateNSView(_ field: LauncherSearchTextField, context: Context) {
        context.coordinator.parent = self
        if field.stringValue != text { field.stringValue = text }
        onReady(field)
    }
    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: LauncherSearchField
        init(_ parent: LauncherSearchField) { self.parent = parent }
        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSTextField else { return }
            parent.text = field.stringValue
        }
        func control(_ control: NSControl, textView: NSTextView, doCommandBy command: Selector) -> Bool {
            guard !textView.hasMarkedText() else { return false }
            switch command {
            case #selector(NSResponder.moveDown(_:)): parent.onMove(1)
            case #selector(NSResponder.moveUp(_:)): parent.onMove(-1)
            case #selector(NSResponder.insertNewline(_:)): parent.onSubmit()
            case #selector(NSResponder.cancelOperation(_:)): parent.onEscape()
            default: return false
            }
            return true
        }
    }
}
