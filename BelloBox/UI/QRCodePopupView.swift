import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// Holds the (editable) text encoded in the QR popup.
@MainActor
final class QRCodePopupViewModel: ObservableObject {
    @Published var text: String {
        didSet {
            // Exact bytes: é and e + U+0301 are equal Strings but different codes.
            guard !text.utf8.elementsEqual(oldValue.utf8) else { return }
            statusMessage = nil
            errorMessage = nil
            renderImage()
        }
    }
    @Published var statusMessage: String?
    @Published var errorMessage: String?
    /// The full-size code for display, copying and saving: never fewer than
    /// four pixels per module, so dense codes stay readable. After an edit it
    /// is drawn off the main thread (up to ~30 ms for 2,000 bytes), so typing
    /// never waits on it; until it lands this is the previous text's code.
    /// Text that cannot be encoded clears it at once, and with no previous code
    /// (the first text, or after an unencodable one) it is drawn at once, so
    /// the card never flashes a placeholder.
    @Published private(set) var image: NSImage?
    private var imageText: String
    private let renders = QRRenderQueue()

    var onClose: () -> Void = {}
    weak var hostWindow: NSWindow?

    init(text: String) {
        self.text = text
        imageText = text
        image = Self.render(text)
    }

    private nonisolated static func render(_ text: String) -> NSImage? {
        QRCodeGenerator.image(for: text, pixelSize: QRCodeGenerator.exportPixelSize(for: text))
    }

    private func renderImage() {
        let text = text
        guard QRCodeGenerator.isEncodable(text), image != nil else {
            renders.cancel()
            image = Self.render(text)
            imageText = text
            return
        }
        renders.run({ Self.render(text) }) { [weak self] image in
            guard let self, self.text.utf8.elementsEqual(text.utf8) else { return }
            self.image = image
            self.imageText = text
        }
    }

    /// The code for the current text, drawn now if an edit's is still on its way.
    private var currentImage: NSImage? {
        if !imageText.utf8.elementsEqual(text.utf8) {
            renders.cancel()
            image = Self.render(text)
            imageText = text
        }
        return image
    }

    var byteCount: Int { text.utf8.count }
    var isEmpty: Bool { text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    var isTooLong: Bool { byteCount > QRCodeGenerator.maxByteCount }
    var capacityMessage: String {
        let remaining = QRCodeGenerator.maxByteCount - byteCount
        let unit = abs(remaining) == 1 ? "byte" : "bytes"
        return remaining >= 0
            ? "\(remaining.formatted()) \(unit) available"
            : "Remove at least \((-remaining).formatted()) \(unit) to create a QR code."
    }

    func copyImage() {
        statusMessage = nil
        errorMessage = nil
        guard let image = currentImage else {
            errorMessage = "There is no QR image to copy."
            return
        }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        if pasteboard.writeObjects([image]) {
            statusMessage = "Copied QR image."
        } else {
            errorMessage = "Could not copy the QR image."
        }
    }

    /// Both the popup and palette attach the save sheet to their own window.
    func save(in window: NSWindow? = nil) {
        statusMessage = nil
        errorMessage = nil
        guard let data = QRCodeGenerator.pngData(for: text, pixelSize: QRCodeGenerator.exportPixelSize(for: text)) else {
            errorMessage = "There is no QR image to save."
            return
        }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.png]
        panel.nameFieldStringValue = "qr-code.png"
        panel.canCreateDirectories = true
        WindowDialogPresenter.present(panel, for: window ?? hostWindow) { [weak self] response in
            guard let self else { return }
            guard response == .OK, let url = panel.url else { return }
            do {
                try data.write(to: url, options: .atomic)
                self.statusMessage = "Saved to \(url.lastPathComponent)."
            } catch {
                self.errorMessage = "Could not save QR image: \(error.localizedDescription)"
            }
        }
    }

    func pasteText() {
        guard let value = NSPasteboard.general.string(forType: .string), !value.isEmpty else {
            errorMessage = "The clipboard does not contain text."
            return
        }
        text = value
    }

    func close() { onClose() }
}

/// The QR popup: a live QR for the selection, an editable text field that
/// regenerates it, and copy/save actions.
struct QRCodePopupView: View {
    static let preferredSize = CGSize(width: 520, height: 620)

    @ObservedObject var viewModel: QRCodePopupViewModel
    var onMinimize: () -> Void = {}

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            qrArea.frame(maxHeight: .infinity)
            editor
            if viewModel.errorMessage != nil || viewModel.statusMessage != nil { messageArea }
            footer
        }
        .padding(12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .popupCard()
        .appearPop()
        .background(DialogWindowReader { viewModel.hostWindow = $0 })
        .onExitCommand { viewModel.close() }
    }

    private var header: some View {
        PopupHeader(icon: "qrcode", title: "QR Code", subtitle: "Text to a scannable image, instantly", onMinimize: onMinimize) { viewModel.close() }
    }

    @ViewBuilder
    private var qrArea: some View {
        ZStack {
            ToolSurface(role: .card).clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            if let image = viewModel.image {
                Image(nsImage: image)
                    .resizable()
                    .interpolation(.none)
                    .scaledToFit()
                    .padding(14)
                    .background(.white, in: RoundedRectangle(cornerRadius: 8))
            } else {
                VStack(spacing: 6) {
                    Image(systemName: viewModel.isEmpty ? "qrcode" : "exclamationmark.triangle")
                        .font(.system(size: 26))
                        .foregroundStyle(BoxTheme.secondaryText)
                    Text(viewModel.isEmpty
                        ? "Enter text to encode"
                        : (viewModel.isTooLong ? viewModel.capacityMessage : "Could not generate a QR code"))
                        .font(.caption)
                        .foregroundStyle(BoxTheme.secondaryText)
                        .multilineTextAlignment(.center)
                }
                .padding()
            }
        }
        .frame(minHeight: 160, maxHeight: 300)
        .shadow(color: .black.opacity(0.08), radius: 4, y: 1)
    }

    private var editor: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text("Encoded text").font(.system(size: 12, weight: .semibold))
                Spacer()
                Button("Clear") { viewModel.text = "" }
                    .buttonStyle(ToolLinkButtonStyle()).font(.caption).disabled(viewModel.text.isEmpty)
                Button("Paste Text", action: viewModel.pasteText)
                    .buttonStyle(ToolLinkButtonStyle()).font(.caption)
                    .help("Use text from your clipboard")
            }
            LiteralTextEditor(text: $viewModel.text, label: "Encoded text", focusesWhenAttached: true)
                .font(.callout)
                .frame(height: 100)
                .scrollContentBackground(.hidden)
                .padding(8)
                .toolSurface(.input, cornerRadius: 10)
                .accessibilityLabel("Encoded text")
            HStack {
                Text("\(viewModel.byteCount.formatted()) / \(QRCodeGenerator.maxByteCount.formatted()) bytes")
                Spacer()
                Text(viewModel.capacityMessage)
            }
            .font(.system(size: 11))
            .foregroundStyle(viewModel.isTooLong ? BoxTheme.danger : .secondary)
            .help("QR capacity is measured in UTF-8 bytes. Emoji and some characters use more than one byte.")
        }
    }

    @ViewBuilder
    private var messageArea: some View {
        ZStack(alignment: .leading) {
            if let error = viewModel.errorMessage {
                Label(error, systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(BoxTheme.warning)
                    .textSelection(.enabled)
            } else if let status = viewModel.statusMessage {
                Label(status, systemImage: "checkmark.circle.fill")
                    .font(.caption)
                    .foregroundStyle(BoxTheme.secondaryText)
                    .textSelection(.enabled)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .frame(minHeight: 20, maxHeight: 40, alignment: .leading)
    }

    private var footer: some View {
        HStack(spacing: 8) {
            Spacer()
            Button { viewModel.save() } label: { Label("Save…", systemImage: "square.and.arrow.down") }
                .buttonStyle(SecondaryButtonStyle())
                .keyboardShortcut("s", modifiers: .command)
                .help("Save QR image as PNG (⌘S)")
                .disabled(viewModel.image == nil)
            Button { viewModel.copyImage() } label: { Label("Copy Image", systemImage: "doc.on.doc") }
                .buttonStyle(PrimaryButtonStyle())
                .keyboardShortcut("c", modifiers: [.command, .shift])
                .help("Copy QR image (⇧⌘C)")
                .disabled(viewModel.image == nil)
        }
    }
}
