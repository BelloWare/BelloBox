import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// Holds the (editable) text encoded in the QR popup.
@MainActor
final class QRCodePopupViewModel: ObservableObject {
    @Published var text: String {
        didSet {
            guard text != oldValue else { return }
            cachedImageText = nil
            cachedImage = nil
            statusMessage = nil
            errorMessage = nil
        }
    }
    @Published var statusMessage: String?
    @Published var errorMessage: String?

    private var cachedImageText: String?
    private var cachedImage: NSImage?

    var onClose: () -> Void = {}

    init(text: String) {
        self.text = text
    }

    var image: NSImage? {
        if cachedImageText == text {
            return cachedImage
        }
        let image = QRCodeGenerator.image(for: text)
        cachedImageText = text
        cachedImage = image
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
        guard let image else {
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

    func save() {
        statusMessage = nil
        errorMessage = nil
        guard let data = QRCodeGenerator.pngData(for: text) else {
            errorMessage = "There is no QR image to save."
            return
        }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.png]
        panel.nameFieldStringValue = "qr-code.png"
        panel.canCreateDirectories = true
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            do {
                try data.write(to: url, options: .atomic)
                Task { @MainActor in self.statusMessage = "Saved to \(url.lastPathComponent)." }
            } catch {
                Task { @MainActor in self.errorMessage = "Could not save QR image: \(error.localizedDescription)" }
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
            qrArea
            editor
            messageArea
            footer
        }
        .padding(16)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height, alignment: .topLeading)
        .popupCard()
        .appearPop()
        .onExitCommand { viewModel.close() }
    }

    private var header: some View {
        PopupHeader(icon: "qrcode", title: "QR Code", subtitle: "Text to a scannable image, instantly", onMinimize: onMinimize) { viewModel.close() }
    }

    @ViewBuilder
    private var qrArea: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 12, style: .continuous).fill(.white)
            if let image = viewModel.image {
                Image(nsImage: image)
                    .resizable()
                    .interpolation(.none)
                    .scaledToFit()
                    .padding(14)
            } else {
                VStack(spacing: 6) {
                    Image(systemName: viewModel.isEmpty ? "qrcode" : "exclamationmark.triangle")
                        .font(.system(size: 26))
                        .foregroundStyle(Color.black.opacity(0.6))
                    Text(viewModel.isEmpty
                        ? "Enter text to encode"
                        : (viewModel.isTooLong ? viewModel.capacityMessage : "Could not generate a QR code"))
                        .font(.caption)
                        .foregroundStyle(Color.black.opacity(0.6))
                        .multilineTextAlignment(.center)
                }
                .padding()
            }
        }
        .frame(height: 300)
        .shadow(color: .black.opacity(0.08), radius: 4, y: 1)
    }

    private var editor: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text("Encoded text").font(.caption.bold()).foregroundStyle(.secondary)
                Spacer()
                Button("Clear") { viewModel.text = "" }
                    .buttonStyle(.link).font(.caption).disabled(viewModel.text.isEmpty)
                Button("Paste Text", action: viewModel.pasteText)
                    .buttonStyle(.link).font(.caption)
                    .help("Use text from your clipboard")
            }
            TextEditor(text: $viewModel.text)
                .font(.callout)
                .frame(height: 100)
                .scrollContentBackground(.hidden)
                .padding(6)
                .background(RoundedRectangle(cornerRadius: 10).fill(BoxTheme.well))
                .accessibilityLabel("Encoded text")
            HStack {
                Text("\(viewModel.byteCount.formatted()) / \(QRCodeGenerator.maxByteCount.formatted()) bytes")
                Spacer()
                Text(viewModel.capacityMessage)
            }
            .font(.caption2)
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
                    .foregroundStyle(.secondary)
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
