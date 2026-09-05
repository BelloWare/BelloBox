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
    @Published var showDiscardConfirmation = false
    private var discardRequested = false
    var onClose: () -> Void = {}

    init(fileURL: URL, removeRecording: @escaping (URL) throws -> Void = { try FileManager.default.trashItem(at: $0, resultingItemURL: nil) }) {
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
        guard discardRequested else { return }
        discardRequested = false
        showDiscardConfirmation = false
        statusMessage = nil
        errorMessage = nil
        player.pause()
        do {
            try removeRecording(fileURL)
            onClose()
        } catch {
            errorMessage = "Could not move recording to Trash: \(error.localizedDescription)"
        }
    }

    func requestDiscard() {
        player.pause()
        discardRequested = true
        showDiscardConfirmation = true
    }

    func cancelDiscard() {
        discardRequested = false
        showDiscardConfirmation = false
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

            RecordingPlayerView(player: viewModel.player)
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
                    .keyboardShortcut("s", modifiers: [.command, .shift])
                Button("Copy File") { viewModel.copyFile() }
                    .buttonStyle(SecondaryButtonStyle())
                Button("Show in Finder") { viewModel.revealInFinder() }
                    .buttonStyle(SecondaryButtonStyle())
                Button("Move to Trash…", role: .destructive) { viewModel.requestDiscard() }
                    .buttonStyle(SecondaryButtonStyle())
            }
        }
        .padding(18)
        .frame(width: 760, height: 430)
        .popupCard()
        .onDisappear { viewModel.player.pause() }
        .alert("Move this recording to Trash?", isPresented: $viewModel.showDiscardConfirmation) {
            Button("Keep Recording", role: .cancel) { viewModel.cancelDiscard() }
            Button("Move to Trash", role: .destructive) { viewModel.discard() }
        } message: {
            Text("\(viewModel.fileName) will be moved to Trash. You can restore it from Finder.")
        }
    }
}

/// Refer to AVPlayerView directly so AVKit is linked and loaded before its player
/// controls are created, including when review is the first window opened.
struct RecordingPlayerView: NSViewRepresentable {
    let player: AVPlayer

    func makeNSView(context: Context) -> AVPlayerView {
        let view = AVPlayerView()
        view.controlsStyle = .inline
        view.player = player
        return view
    }

    func updateNSView(_ nsView: AVPlayerView, context: Context) {
        if nsView.player !== player { nsView.player = player }
    }

    static func dismantleNSView(_ nsView: AVPlayerView, coordinator: ()) {
        nsView.player = nil
    }
}
