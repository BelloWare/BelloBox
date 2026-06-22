import AVKit
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class RecordingReviewViewModel: ObservableObject {
    let fileURL: URL
    let player: AVPlayer
    private let removeRecording: (URL) throws -> Void
    @Published var statusMessage: String?
    @Published var errorMessage: String?
    var onClose: () -> Void = {}

    init(fileURL: URL, removeRecording: @escaping (URL) throws -> Void = { try FileManager.default.removeItem(at: $0) }) {
        self.fileURL = fileURL
        self.player = AVPlayer(url: fileURL)
        self.removeRecording = removeRecording
    }

    var fileName: String { fileURL.lastPathComponent }

    var fileSizeText: String {
        let size = (try? FileManager.default.attributesOfItem(atPath: fileURL.path)[.size] as? NSNumber)?.int64Value ?? 0
        return ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
    }

    func saveAs() {
        statusMessage = nil
        errorMessage = nil
        let panel = NSSavePanel()
        panel.nameFieldStringValue = fileURL.lastPathComponent
        panel.allowedContentTypes = [.quickTimeMovie]
        guard panel.runModal() == .OK, let destination = panel.url else { return }
        do {
            try copyRecording(to: destination)
            statusMessage = "Saved to \(destination.lastPathComponent)."
        } catch {
            errorMessage = "Could not save recording: \(error.localizedDescription)"
        }
    }

    func copyRecording(to destination: URL) throws {
        let source = fileURL.standardizedFileURL
        let target = destination.standardizedFileURL
        guard source.path != target.path else { return }
        if FileManager.default.fileExists(atPath: destination.path) {
            try FileManager.default.removeItem(at: destination)
        }
        try FileManager.default.copyItem(at: fileURL, to: destination)
    }

    func copyFile() {
        statusMessage = nil
        errorMessage = nil
        NSPasteboard.general.clearContents()
        if NSPasteboard.general.writeObjects([fileURL as NSURL]) {
            statusMessage = "Copied recording file."
        } else {
            errorMessage = "Could not copy the recording file."
        }
    }

    func revealInFinder() {
        NSWorkspace.shared.activateFileViewerSelecting([fileURL])
    }

    func discard() {
        statusMessage = nil
        errorMessage = nil
        player.pause()
        do {
            try removeRecording(fileURL)
            onClose()
        } catch {
            errorMessage = "Could not discard recording: \(error.localizedDescription)"
        }
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

            if let message = viewModel.errorMessage {
                Label(message, systemImage: "exclamationmark.triangle.fill")
                    .font(.callout)
                    .foregroundStyle(.red)
                    .textSelection(.enabled)
            } else if let message = viewModel.statusMessage {
                Label(message, systemImage: "checkmark.circle.fill")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
            }

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
