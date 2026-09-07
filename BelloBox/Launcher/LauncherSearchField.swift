import AppKit
import SwiftUI

final class LauncherSearchTextField: NSTextField {
    /// The palette's search field claims focus as soon as it is attached.
    /// Secondary fields (the copilot question, preview inputs) wait for a click instead.
    var focusesWhenAttached = true

    override var needsPanelToBecomeKey: Bool { true }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard window != nil, focusesWhenAttached else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, let window = self.window, window.isVisible else { return }
            // Never interrupt typing that already started in another field.
            if let editor = window.firstResponder as? NSTextView, editor.delegate !== self { return }
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
    var placeholder = "Search tools and commands…"
    var accessibilityID = "launcherSearch"
    var accessibilityLabel = "Search tools and commands"
    var fontSize: CGFloat = 19
    var focusesWhenAttached = true
    var monospaced = false
    /// Whether Up/Down are reported through `onMove` (the search field) or
    /// left to the field's own editing (preview inputs).
    var consumesVerticalArrows = true
    /// The palette search field while a tool is open: still attached (so it
    /// can take focus at once when the tool closes) but disabled, so Tab
    /// skips it, and hidden from assistive technology.
    var parked = false

    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeNSView(context: Context) -> LauncherSearchTextField {
        let field = LauncherSearchTextField()
        field.focusesWhenAttached = focusesWhenAttached
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = monospaced ? .monospacedSystemFont(ofSize: fontSize, weight: .regular) : .systemFont(ofSize: fontSize, weight: .regular)
        field.textColor = .labelColor
        field.placeholderString = placeholder
        field.maximumNumberOfLines = 1
        field.cell?.isScrollable = true
        field.identifier = NSUserInterfaceItemIdentifier(accessibilityID)
        field.setAccessibilityIdentifier(accessibilityID)
        field.setAccessibilityLabel(accessibilityLabel)
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        field.delegate = context.coordinator
        onReady(field)
        return field
    }
    func updateNSView(_ field: LauncherSearchTextField, context: Context) {
        context.coordinator.parent = self
        if field.stringValue != text { field.stringValue = text }
        if field.placeholderString != placeholder { field.placeholderString = placeholder }
        field.isEnabled = context.environment.isEnabled && !parked
        field.setAccessibilityHidden(parked)
        onReady(field)
    }
    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: LauncherSearchField
        init(_ parent: LauncherSearchField) { self.parent = parent }
        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSTextField else { return }
            parent.text = field.stringValue
        }
        func controlTextDidBeginEditing(_ notification: Notification) {
            // Patterns, URLs and expressions must stay literal: no smart quotes,
            // dashes, or replacements from the shared field editor.
            guard let field = notification.object as? NSTextField, let editor = field.currentEditor() as? NSTextView else { return }
            editor.isAutomaticQuoteSubstitutionEnabled = false
            editor.isAutomaticDashSubstitutionEnabled = false
            editor.isAutomaticTextReplacementEnabled = false
            editor.isAutomaticSpellingCorrectionEnabled = false
            editor.isContinuousSpellCheckingEnabled = false
        }
        func control(_ control: NSControl, textView: NSTextView, doCommandBy command: Selector) -> Bool {
            guard !textView.hasMarkedText() else { return false }
            switch command {
            case #selector(NSResponder.moveDown(_:)):
                guard parent.consumesVerticalArrows else { return false }
                parent.onMove(1)
            case #selector(NSResponder.moveUp(_:)):
                guard parent.consumesVerticalArrows else { return false }
                parent.onMove(-1)
            case #selector(NSResponder.insertNewline(_:)): parent.onSubmit()
            case #selector(NSResponder.cancelOperation(_:)): parent.onEscape()
            default: return false
            }
            return true
        }
    }
}
