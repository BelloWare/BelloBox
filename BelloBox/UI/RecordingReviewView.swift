import AVKit
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class RecordingReviewViewModel: ObservableObject {
    let fileURL: URL
    let recoveryWarning: String?
    let player: AVPlayer
    private let removeRecording: (URL) throws -> Void
    @Published var statusMessage: String?
    @Published var errorMessage: String?
    @Published var showDiscardConfirmation = false
    @Published private(set) var isSaving = false
    private var discardRequested = false
    var onClose: () -> Void = {}

    init(fileURL: URL, recoveryWarning: String? = nil, removeRecording: @escaping (URL) throws -> Void = { try FileManager.default.trashItem(at: $0, resultingItemURL: nil) }) {
        self.fileURL = fileURL
        self.recoveryWarning = recoveryWarning
        self.player = AVPlayer(url: fileURL)
        self.removeRecording = removeRecording
    }

    var fileName: String { fileURL.lastPathComponent }

    var fileSizeText: String {
        let size = (try? FileManager.default.attributesOfItem(atPath: fileURL.path)[.size] as? NSNumber)?.int64Value ?? 0
        return ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
    }

    func saveAs() {
        guard !isSaving else { return }
        statusMessage = nil
        errorMessage = nil
        let panel = NSSavePanel()
        panel.nameFieldStringValue = fileURL.lastPathComponent
        panel.allowedContentTypes = [.quickTimeMovie]
        guard panel.runModal() == .OK, let destination = panel.url else { return }
        Task { await saveRecording(to: destination) }
    }

    func saveRecording(to destination: URL) async {
        guard !isSaving, !Task.isCancelled else { return }
        isSaving = true
        statusMessage = nil
        errorMessage = nil
        defer { isSaving = false }
        let source = fileURL
        do {
            let copyTask = Task.detached(priority: .userInitiated) {
                try RecordingFileStore.copy(from: source, to: destination)
            }
            try await withTaskCancellationHandler {
                try await copyTask.value
            } onCancel: {
                copyTask.cancel()
            }
            statusMessage = "Saved to \(destination.lastPathComponent)."
        } catch {
            errorMessage = "Could not save recording: \(error.localizedDescription)"
        }
    }

    func copyRecording(to destination: URL) throws {
        try RecordingFileStore.copy(from: fileURL, to: destination)
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
        guard discardRequested, !isSaving else { return }
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
        guard !isSaving else { return }
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
    static let preferredSize = CGSize(width: 760, height: 490)
    @ObservedObject var viewModel: RecordingReviewViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            PopupHeader(
                icon: "play.rectangle",
                title: "Recording",
                subtitle: viewModel.fileName,
                onClose: viewModel.onClose
            )

            if let warning = viewModel.recoveryWarning {
                Label(warning, systemImage: "exclamationmark.triangle.fill")
                    .font(.callout)
                    .foregroundStyle(BoxTheme.warning)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
            }

            RecordingPlayerView(player: viewModel.player)
                .frame(height: viewModel.recoveryWarning == nil ? 300 : 220)
                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.primary.opacity(0.08), lineWidth: 1))

            ZStack(alignment: .leading) {
                if viewModel.isSaving {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Saving recording…").font(.callout)
                    }
                } else if let message = viewModel.errorMessage {
                    Label(message, systemImage: "exclamationmark.triangle.fill")
                        .font(.callout)
                        .foregroundStyle(BoxTheme.danger)
                        .textSelection(.enabled)
                } else if let message = viewModel.statusMessage {
                    Label(message, systemImage: "checkmark.circle.fill")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                }
            }
            .lineLimit(1)
            .help(viewModel.errorMessage ?? viewModel.statusMessage ?? "")
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(height: 22)

            HStack {
                Label(viewModel.fileSizeText, systemImage: "doc")
                    .foregroundStyle(.secondary)
                Spacer()
                Button("Save As…") { viewModel.saveAs() }
                    .buttonStyle(SecondaryButtonStyle())
                    .keyboardShortcut("s", modifiers: [.command, .shift])
                    .disabled(viewModel.isSaving)
                Button("Copy File") { viewModel.copyFile() }
                    .buttonStyle(SecondaryButtonStyle())
                Button("Show in Finder") { viewModel.revealInFinder() }
                    .buttonStyle(SecondaryButtonStyle())
                Button("Move to Trash…", role: .destructive) { viewModel.requestDiscard() }
                    .buttonStyle(SecondaryButtonStyle())
                    .disabled(viewModel.isSaving)
            }
        }
        .padding(18)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height)
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
