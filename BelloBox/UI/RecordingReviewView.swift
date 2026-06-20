import AVKit
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class RecordingReviewViewModel: ObservableObject {
    let fileURL: URL
    let player: AVPlayer
    var onClose: () -> Void = {}

    init(fileURL: URL) {
        self.fileURL = fileURL
        self.player = AVPlayer(url: fileURL)
    }

    var fileName: String { fileURL.lastPathComponent }

    var fileSizeText: String {
        let size = (try? FileManager.default.attributesOfItem(atPath: fileURL.path)[.size] as? NSNumber)?.int64Value ?? 0
        return ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
    }

    func saveAs() {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = fileURL.lastPathComponent
        panel.allowedContentTypes = [.quickTimeMovie]
        guard panel.runModal() == .OK, let destination = panel.url else { return }
        try? FileManager.default.removeItem(at: destination)
        try? FileManager.default.copyItem(at: fileURL, to: destination)
    }

    func copyFile() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.writeObjects([fileURL as NSURL])
    }

    func revealInFinder() {
        NSWorkspace.shared.activateFileViewerSelecting([fileURL])
    }

    func discard() {
        player.pause()
        try? FileManager.default.removeItem(at: fileURL)
        onClose()
    }
}

struct RecordingReviewView: View {
    @ObservedObject var viewModel: RecordingReviewViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            PopupHeader(
                icon: "play.rectangle",
                title: "Recording",
                subtitle: viewModel.fileName,
                onClose: viewModel.onClose
            )

            VideoPlayer(player: viewModel.player)
                .frame(height: 300)
                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.primary.opacity(0.08), lineWidth: 1))

            HStack {
                Label(viewModel.fileSizeText, systemImage: "doc")
                    .foregroundStyle(.secondary)
                Spacer()
                Button("Save As…") { viewModel.saveAs() }
                    .buttonStyle(SecondaryButtonStyle())
                Button("Copy File") { viewModel.copyFile() }
                    .buttonStyle(SecondaryButtonStyle())
                Button("Reveal") { viewModel.revealInFinder() }
                    .buttonStyle(SecondaryButtonStyle())
                Button("Discard") { viewModel.discard() }
                    .buttonStyle(SecondaryButtonStyle())
            }
        }
        .padding(18)
        .frame(width: 760, height: 430)
        .popupCard()
        .onDisappear { viewModel.player.pause() }
    }
}
