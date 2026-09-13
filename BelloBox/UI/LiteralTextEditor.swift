import AppKit
import SwiftUI

/// Utility inputs must preserve literal characters. Native text editing keeps
/// selection, undo, input methods and accessibility while disabling prose-only
/// substitutions such as curly quotes, em dashes and automatic replacements.
struct LiteralTextEditor: NSViewRepresentable {
    @Binding var text: String
    var label: String
    var monospaced = false
    var focusesWhenAttached = false
    var fontSize: CGFloat? = nil

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        let editor = LiteralTextView(frame: .zero)
        editor.isRichText = false
        editor.importsGraphics = false
        editor.allowsUndo = true
        editor.drawsBackground = false
        editor.textColor = NSColor(BoxTheme.primaryText)
        editor.font = monospaced ? .monospacedSystemFont(ofSize: fontSize ?? 12, weight: .regular) : .systemFont(ofSize: fontSize ?? 13)
        editor.textContainerInset = NSSize(width: 2, height: 4)
        editor.minSize = .zero
        editor.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        editor.isVerticallyResizable = true
        editor.isHorizontallyResizable = false
        editor.autoresizingMask = [.width]
        editor.textContainer?.widthTracksTextView = true
        editor.configureLiteralInput()
        editor.string = text
        editor.setAccessibilityLabel(label)
        editor.focusesWhenAttached = focusesWhenAttached
        editor.delegate = context.coordinator
        scroll.documentView = editor
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        context.coordinator.parent = self
        guard let editor = scroll.documentView as? LiteralTextView else { return }
        editor.isEditable = context.environment.isEnabled
        guard editor.string != text, !editor.hasMarkedText() else { return }
        // External actions (Example, Reset, Paste) replace the draft. Old undo
        // ranges must not be applied to the replacement document.
        editor.string = text
        editor.undoManager?.removeAllActions()
    }

    final class Coordinator: NSObject, NSTextViewDelegate {
        var parent: LiteralTextEditor
        init(_ parent: LiteralTextEditor) { self.parent = parent }
        func textDidChange(_ notification: Notification) {
            guard let editor = notification.object as? NSTextView else { return }
            parent.text = editor.string
        }
    }
}

final class LiteralTextView: NSTextView {
    var focusesWhenAttached = false
    private var didSetInitialFocus = false
    override var needsPanelToBecomeKey: Bool { true }

    func configureLiteralInput() {
        isAutomaticQuoteSubstitutionEnabled = false
        isAutomaticDashSubstitutionEnabled = false
        isAutomaticTextReplacementEnabled = false
        isAutomaticSpellingCorrectionEnabled = false
        isAutomaticLinkDetectionEnabled = false
        isAutomaticDataDetectionEnabled = false
        isContinuousSpellCheckingEnabled = false
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil { didSetInitialFocus = false }
        guard window != nil, focusesWhenAttached, !didSetInitialFocus else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, let window = self.window, window.isVisible, !self.didSetInitialFocus else { return }
            self.didSetInitialFocus = true
            window.makeFirstResponder(self)
        }
    }

    static func initialEditor(in view: NSView) -> LiteralTextView? {
        if let editor = view as? LiteralTextView, editor.focusesWhenAttached { return editor }
        return view.subviews.lazy.compactMap { initialEditor(in: $0) }.first
    }
}
