import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// Holds the (editable) text encoded in the QR popup.
@MainActor
final class QRCodePopupViewModel: ObservableObject {
    @Published var text: String

    var onClose: () -> Void = {}

    init(text: String) {
        self.text = text
    }

    var image: NSImage? { QRCodeGenerator.image(for: text) }
    var byteCount: Int { text.utf8.count }
    var isEmpty: Bool { text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    var isTooLong: Bool { byteCount > QRCodeGenerator.maxByteCount }

    func copyImage() {
        guard let image else { return }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.writeObjects([image])
    }

    func save() {
        guard let data = QRCodeGenerator.pngData(for: text) else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.png]
        panel.nameFieldStringValue = "qr-code.png"
        panel.canCreateDirectories = true
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            try? data.write(to: url)
        }
    }

    func close() { onClose() }
}

/// The QR popup: a live QR for the selection, an editable text field that
/// regenerates it, and copy/save actions.
struct QRCodePopupView: View {
    static let preferredSize = CGSize(width: 320, height: 466)

    @ObservedObject var viewModel: QRCodePopupViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            qrArea
            editor
            footer
        }
        .padding(16)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height, alignment: .topLeading)
        .background(
            RoundedRectangle(cornerRadius: 16, style: .continuous).fill(.regularMaterial)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(.primary.opacity(0.08), lineWidth: 1)
        )
        .onExitCommand { viewModel.close() }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Image(systemName: "qrcode")
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(BoxTheme.accent)
            Text("QR Code").font(.headline)
            Spacer()
            Button { viewModel.close() } label: {
                Image(systemName: "xmark.circle.fill").font(.system(size: 14)).foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("Close")
        }
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
                        .foregroundStyle(.secondary)
                    Text(viewModel.isEmpty
                        ? "Enter text to encode"
                        : "Text is too long for a QR code")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                }
                .padding()
            }
        }
        .frame(height: 224)
        .shadow(color: .black.opacity(0.08), radius: 4, y: 1)
    }

    private var editor: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Encoded text").font(.caption.bold()).foregroundStyle(.secondary)
            TextEditor(text: $viewModel.text)
                .font(.callout)
                .frame(height: 56)
                .scrollContentBackground(.hidden)
                .padding(6)
                .background(RoundedRectangle(cornerRadius: 9).fill(.primary.opacity(0.05)))
        }
    }

    private var footer: some View {
        HStack(spacing: 8) {
            Text("\(viewModel.byteCount) bytes")
                .font(.caption2)
                .foregroundStyle(viewModel.isTooLong ? .red : .secondary)
            Spacer()
            Button { viewModel.save() } label: { Label("Save…", systemImage: "square.and.arrow.down") }
                .disabled(viewModel.image == nil)
            Button { viewModel.copyImage() } label: { Label("Copy", systemImage: "doc.on.doc") }
                .disabled(viewModel.image == nil)
        }
    }
}
