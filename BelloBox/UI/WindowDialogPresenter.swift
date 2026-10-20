import AppKit
import SwiftUI

/// File panels and confirmations belong to the window that invoked them. In
/// particular, an app-modal panel at the normal level is hidden by capture's
/// screen-saver-level shields. Keep those shields in place and raise the sheet,
/// including above shields on other displays if the user moves the dialog.
@MainActor
enum WindowDialogPresenter {
    private static var activeDialogs = Set<ObjectIdentifier>()

    static var ownsKeyboard: Bool {
        !activeDialogs.isEmpty || NSApp.modalWindow != nil
            || NSApp.windows.contains { $0.attachedSheet != nil }
    }

    static func present(
        _ panel: NSSavePanel,
        for window: NSWindow? = nil,
        completion: @escaping (NSApplication.ModalResponse) -> Void
    ) {
        if let window, !window.isVisible { completion(.cancel); return }
        let owner = presentationWindow(window)
        guard owner?.attachedSheet == nil else { completion(.cancel); return }
        let finish = prepare(panel, completion: completion)
        if let owner {
            panel.beginSheetModal(for: owner, completionHandler: finish)
        } else {
            panel.begin(completionHandler: finish)
        }
        elevate(panel)
    }

    static func present(
        _ alert: NSAlert,
        for window: NSWindow,
        completion: @escaping (NSApplication.ModalResponse) -> Void
    ) {
        guard window.isVisible, window.attachedSheet == nil else { completion(.cancel); return }
        let finish = prepare(alert.window, completion: completion)
        alert.beginSheetModal(for: window, completionHandler: finish)
        elevate(alert.window)
    }

    static func cancelDialogs(for window: NSWindow) {
        guard let sheet = window.attachedSheet else { return }
        if let panel = sheet as? NSSavePanel {
            panel.cancel(nil)
        } else {
            window.endSheet(sheet, returnCode: .cancel)
        }
    }

    private static func presentationWindow(_ requested: NSWindow?) -> NSWindow? {
        // An explicit owner that has closed must not lend its dialog to some
        // unrelated window. Callers launch these only from an explicit action.
        if let requested { return requested.isVisible ? requested : nil }
        return [NSApp.keyWindow, NSApp.mainWindow].compactMap { $0 }.first {
            $0.isVisible && $0.sheetParent == nil && !($0 is NSSavePanel)
        }
    }

    private static func prepare(
        _ dialog: NSWindow,
        completion: @escaping (NSApplication.ModalResponse) -> Void
    ) -> (NSApplication.ModalResponse) -> Void {
        let identifier = ObjectIdentifier(dialog)
        let originalLevel = dialog.level
        activeDialogs.insert(identifier)
        elevate(dialog)
        return { response in
            dialog.level = originalLevel
            activeDialogs.remove(identifier)
            // Restore before any write/conversion can fail or open another dialog.
            completion(response)
        }
    }

    private static func elevate(_ dialog: NSWindow) {
        // A failed begin can finish synchronously. Do not raise it again after
        // its completion has already restored the original level.
        guard activeDialogs.contains(ObjectIdentifier(dialog)) else { return }
        let highest = NSApp.windows.filter { $0 !== dialog && $0.isVisible }
            .map(\.level.rawValue).max() ?? NSWindow.Level.normal.rawValue
        dialog.level = NSWindow.Level(rawValue: max(NSWindow.Level.modalPanel.rawValue, highest + 1))
    }
}

/// Read the actual hosting window rather than guessing from a key window that
/// may already have changed (nonactivating popups and multiple displays).
struct DialogWindowReader: NSViewRepresentable {
    var onChange: (NSWindow?) -> Void

    func makeNSView(context: Context) -> Reader {
        let view = Reader()
        view.onChange = onChange
        return view
    }

    func updateNSView(_ view: Reader, context: Context) { view.onChange = onChange }

    final class Reader: NSView {
        var onChange: (NSWindow?) -> Void = { _ in }
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            onChange(window)
        }
    }
}

@MainActor
final class DialogWindowReference {
    weak var window: NSWindow?
}

/// SwiftUI's default alerts do not give us control of their level. Use the
/// same sheet policy as file panels for destructive confirmations, retaining
/// the existing binding and requiring an explicit affirmative button.
struct WindowConfirmation: NSViewRepresentable {
    @Binding var isPresented: Bool
    var title: String
    var message: String
    var confirmTitle: String
    var cancelTitle: String
    var onConfirm: () -> Void
    var onCancel: () -> Void

    func makeNSView(context: Context) -> Host { Host() }

    func updateNSView(_ view: Host, context: Context) {
        view.update = { [weak view] in
            guard let view, isPresented, !view.presenting, let window = view.window else { return }
            view.presenting = true
            // Present after the SwiftUI update finishes; binding writes belong
            // to the completion, never to updateNSView.
            DispatchQueue.main.async { [weak view, weak window] in
                guard let view, let window, window.isVisible, isPresented else {
                    view?.presenting = false
                    return
                }
                let alert = NSAlert()
                alert.messageText = title
                alert.informativeText = message
                alert.alertStyle = .warning
                alert.addButton(withTitle: cancelTitle).keyEquivalent = "\u{1b}"
                let confirm = alert.addButton(withTitle: confirmTitle)
                confirm.keyEquivalent = ""
                confirm.hasDestructiveAction = true
                WindowDialogPresenter.present(alert, for: window) { response in
                    view.presenting = false
                    isPresented = false
                    if response == .alertSecondButtonReturn { onConfirm() } else { onCancel() }
                }
            }
        }
        view.update()
    }

    final class Host: NSView {
        var presenting = false
        var update: () -> Void = {}
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            update()
        }
    }
}

extension View {
    func windowConfirmation(
        _ title: String, isPresented: Binding<Bool>, message: String = "",
        confirmTitle: String, cancelTitle: String = "Cancel",
        onConfirm: @escaping () -> Void, onCancel: @escaping () -> Void = {}
    ) -> some View {
        background(WindowConfirmation(
            isPresented: isPresented, title: title, message: message,
            confirmTitle: confirmTitle, cancelTitle: cancelTitle,
            onConfirm: onConfirm, onCancel: onCancel
        ))
    }
}
