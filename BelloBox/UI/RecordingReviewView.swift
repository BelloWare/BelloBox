import AVKit
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class RecordingReviewViewModel: ObservableObject {
    typealias GIFTranscode = (URL, URL, GIFExportOptions, @escaping GIFTranscoder.Progress) async throws -> GIFExportResult

    let fileURL: URL
    let recoveryWarning: String?
    let player: AVPlayer
    private let removeRecording: (URL) throws -> Void
    private let transcodeGIF: GIFTranscode
    @Published var statusMessage: String?
    @Published var errorMessage: String?
    @Published var showDiscardConfirmation = false
    @Published private(set) var isSaving = false
    /// The GIF made from this recording, either when it was recorded in GIF mode or
    /// after an export from the review; the movie stays on disk either way.
    @Published private(set) var gifURL: URL?
    @Published private(set) var isConverting = false
    @Published private(set) var conversionProgress: Double = 0
    @Published var showsGIFExport = false
    @Published var gifOptions: GIFExportOptions
    @Published private(set) var sourceInfo: GIFSourceInfo?
    /// Which file the player area shows: the GIF preview or the movie.
    @Published var showsGIFPreview: Bool
    /// Changes with every finished export so the preview reloads a rewritten path.
    @Published private(set) var gifRevision = 0
    private var discardRequested = false
    private var conversionTask: Task<GIFExportResult, Error>?
    private var infoTask: Task<Void, Never>?
    /// Bumped by close and by every export so late results are dropped.
    private var generation = 0
    var onClose: () -> Void = {}

    init(fileURL: URL, recoveryWarning: String? = nil, gifURL: URL? = nil, gifOptions: GIFExportOptions = .default,
         removeRecording: @escaping (URL) throws -> Void = { try FileManager.default.trashItem(at: $0, resultingItemURL: nil) },
         transcodeGIF: @escaping GIFTranscode = { try await GIFTranscoder.transcode(sourceURL: $0, to: $1, options: $2, progress: $3) }) {
        self.fileURL = fileURL
        self.recoveryWarning = recoveryWarning
        self.player = AVPlayer(url: fileURL)
        self.removeRecording = removeRecording
        self.transcodeGIF = transcodeGIF
        self.gifURL = gifURL
        self.gifOptions = gifOptions
        self.showsGIFPreview = gifURL != nil
    }

    deinit {
        conversionTask?.cancel()
        infoTask?.cancel()
    }

    var fileName: String { fileURL.lastPathComponent }
    var gifFileName: String? { gifURL?.lastPathComponent }
    var hasGIF: Bool { gifURL != nil }

    var fileSizeText: String { Self.sizeText(of: fileURL) }
    var gifSizeText: String? { gifURL.map(Self.sizeText(of:)) }

    private static func sizeText(of url: URL) -> String {
        let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.int64Value ?? 0
        return ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
    }

    /// "12 s · 1280 × 720" once the movie has been inspected.
    var mediaSummary: String? {
        guard let info = sourceInfo else { return nil }
        let seconds = info.duration < 10 ? String(format: "%.1f s", info.duration) : "\(Int(info.duration.rounded())) s"
        return "\(seconds) · \(Int(info.displaySize.width)) × \(Int(info.displaySize.height))"
    }

    /// The plan the current GIF options would produce, for the estimate line.
    var gifPlan: GIFExportPlan? {
        guard let sourceInfo else { return nil }
        return try? GIFExportPlan.make(source: sourceInfo, options: gifOptions)
    }

    func loadSourceInfo() {
        guard sourceInfo == nil, infoTask == nil else { return }
        let url = fileURL
        let generation = generation
        infoTask = Task { [weak self] in
            let info = try? await GIFSourceInfo.load(url: url)
            guard let self, !Task.isCancelled, self.generation == generation else { return }
            self.infoTask = nil
            guard let info else { return }
            self.sourceInfo = info
            if self.gifOptions.trimEnd == nil || self.gifOptions.trimEnd! > info.duration {
                self.gifOptions.trimEnd = min(info.duration, GIFExportOptions.maxDuration)
            }
        }
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
        await save(fileURL, to: destination, label: "recording")
    }

    func saveGIFAs() {
        guard let gifURL, !isSaving else { return }
        statusMessage = nil
        errorMessage = nil
        let panel = NSSavePanel()
        panel.nameFieldStringValue = gifURL.lastPathComponent
        panel.allowedContentTypes = [.gif]
        guard panel.runModal() == .OK, let destination = panel.url else { return }
        Task { await saveGIF(to: destination) }
    }

    func saveGIF(to destination: URL) async {
        guard let gifURL else { return }
        await save(gifURL, to: destination, label: "GIF")
    }

    private func save(_ source: URL, to destination: URL, label: String) async {
        guard !isSaving, !Task.isCancelled else { return }
        isSaving = true
        statusMessage = nil
        errorMessage = nil
        defer { isSaving = false }
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
            errorMessage = "Could not save \(label): \(error.localizedDescription)"
        }
    }

    func copyRecording(to destination: URL) throws {
        try RecordingFileStore.copy(from: fileURL, to: destination)
    }

    func copyFile() {
        copy(fileURL, label: "recording")
    }

    func copyGIFFile() {
        guard let gifURL else { return }
        copy(gifURL, label: "GIF")
    }

    private func copy(_ url: URL, label: String) {
        statusMessage = nil
        errorMessage = nil
        NSPasteboard.general.clearContents()
        if NSPasteboard.general.writeObjects([url as NSURL]) {
            statusMessage = "Copied \(label) file."
        } else {
            errorMessage = "Could not copy the \(label) file."
        }
    }

    func revealInFinder() {
        NSWorkspace.shared.activateFileViewerSelecting([gifURL ?? fileURL, fileURL])
    }

    // MARK: GIF export from the review

    func requestGIFExport() {
        guard !isConverting else { return }
        loadSourceInfo()
        showsGIFExport = true
    }

    func cancelGIFExportRequest() {
        showsGIFExport = false
    }

    /// Asks where to put the GIF, then converts. The movie is never touched.
    func exportGIF() {
        guard !isConverting else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.gif]
        panel.canCreateDirectories = true
        panel.nameFieldStringValue = fileURL.deletingPathExtension().lastPathComponent + ".gif"
        guard panel.runModal() == .OK, let destination = panel.url else { return }
        Task { await exportGIF(to: destination) }
    }

    func exportGIF(to destination: URL) async {
        guard !isConverting else { return }
        generation += 1
        let generation = generation
        isConverting = true
        conversionProgress = 0
        statusMessage = nil
        errorMessage = nil
        showsGIFExport = false
        player.pause()
        let source = fileURL, options = gifOptions
        let reporter = ProgressThrottle()
        let task = Task { [weak self] () -> GIFExportResult in
            guard let self else { throw CancellationError() }
            return try await self.transcodeGIF(source, destination, options) { progress in
                guard reporter.shouldReport(progress) else { return }
                Task { @MainActor [weak self] in
                    guard let self, self.generation == generation, self.isConverting else { return }
                    self.conversionProgress = progress
                }
            }
        }
        conversionTask = task
        do {
            let result = try await task.value
            guard self.generation == generation else { return }
            gifURL = result.url
            gifRevision += 1
            showsGIFPreview = true
            statusMessage = "GIF saved to \(result.url.lastPathComponent) · \(ByteCountFormatter.string(fromByteCount: result.fileSize, countStyle: .file))."
        } catch is CancellationError {
            guard self.generation == generation else { return }
            statusMessage = "GIF export cancelled. The movie is unchanged."
        } catch {
            guard self.generation == generation else { return }
            errorMessage = "Could not write the GIF: \(error.localizedDescription)"
        }
        guard self.generation == generation else { return }
        conversionTask = nil
        isConverting = false
    }

    func cancelGIFExport() {
        conversionTask?.cancel()
    }

    /// Shows the frame at `time` while a trim handle moves.
    func seek(to time: TimeInterval) {
        player.pause()
        player.seek(to: CMTime(seconds: max(0, time), preferredTimescale: 600), toleranceBefore: .zero, toleranceAfter: .zero)
    }

    /// Closing the review cancels an export in flight; whatever it reports later is ignored.
    func close() {
        generation += 1
        conversionTask?.cancel()
        conversionTask = nil
        infoTask?.cancel()
        infoTask = nil
        isConverting = false
        player.pause()
        onClose()
    }

    func discard() {
        guard discardRequested, !isSaving else { return }
        discardRequested = false
        showDiscardConfirmation = false
        statusMessage = nil
        errorMessage = nil
        player.pause()
        conversionTask?.cancel()
        do {
            try removeRecording(fileURL)
            if let gifURL { try? removeRecording(gifURL) }
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

/// Review after a recording: the movie (or the GIF made from it), a compact fact
/// row, and one row of actions. GIF export opens as an overlay panel on top so the
/// review never has to grow.
struct RecordingReviewView: View {
    static let preferredSize = CGSize(width: 760, height: 520)
    @ObservedObject var viewModel: RecordingReviewViewModel

    var body: some View {
        ZStack {
            VStack(alignment: .leading, spacing: 12) {
                PopupHeader(
                    icon: viewModel.hasGIF ? "photo.stack" : "play.rectangle",
                    title: viewModel.hasGIF ? "Recording · GIF ready" : "Recording",
                    subtitle: viewModel.hasGIF ? (viewModel.gifFileName ?? viewModel.fileName) : viewModel.fileName,
                    onClose: viewModel.close
                )

                if let warning = viewModel.recoveryWarning {
                    Label(warning, systemImage: "exclamationmark.triangle.fill")
                        .font(.caption)
                        .foregroundStyle(BoxTheme.warning)
                        .fixedSize(horizontal: false, vertical: true)
                        .textSelection(.enabled)
                }

                preview
                    .frame(maxWidth: .infinity)
                    .frame(height: viewModel.recoveryWarning == nil ? 300 : 250)
                    .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.primary.opacity(0.08), lineWidth: 1))

                statusRow

                facts

                actions
            }
            .padding(18)
            .frame(width: Self.preferredSize.width, height: Self.preferredSize.height)
            .popupCard()
            .onAppear { viewModel.loadSourceInfo() }
            .onDisappear { viewModel.player.pause() }
            .alert("Move this recording to Trash?", isPresented: $viewModel.showDiscardConfirmation) {
                Button("Keep Recording", role: .cancel) { viewModel.cancelDiscard() }
                Button("Move to Trash", role: .destructive) { viewModel.discard() }
            } message: {
                Text(viewModel.hasGIF
                     ? "\(viewModel.fileName) and its GIF will be moved to Trash. You can restore them from Finder."
                     : "\(viewModel.fileName) will be moved to Trash. You can restore it from Finder.")
            }

            if viewModel.showsGIFExport {
                GIFExportPanel(
                    title: "Make a GIF",
                    subtitle: viewModel.fileName,
                    options: $viewModel.gifOptions,
                    sourceInfo: viewModel.sourceInfo,
                    plan: viewModel.gifPlan,
                    isConverting: false,
                    progress: 0,
                    onSeek: viewModel.seek(to:),
                    onConvert: viewModel.exportGIF,
                    onCancel: viewModel.cancelGIFExportRequest
                )
                .padding(32)
            }
        }
    }

    @ViewBuilder private var preview: some View {
        if viewModel.isConverting {
            VStack(spacing: 10) {
                ProgressView(value: viewModel.conversionProgress).progressViewStyle(.linear).tint(BoxTheme.accent).frame(width: 260)
                Text("Writing GIF · \(Int((viewModel.conversionProgress * 100).rounded()))%").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                Button("Cancel", action: viewModel.cancelGIFExport).buttonStyle(SecondaryButtonStyle())
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(BoxTheme.well)
        } else if viewModel.showsGIFPreview, let gif = viewModel.gifURL {
            AnimatedGIFView(url: gif, revision: viewModel.gifRevision)
                .background(BoxTheme.well)
                .accessibilityLabel("GIF preview")
        } else {
            RecordingPlayerView(player: viewModel.player)
        }
    }

    private var statusRow: some View {
        ZStack(alignment: .leading) {
            if viewModel.isSaving {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Saving…").font(.callout)
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
        .frame(height: 20)
    }

    private var facts: some View {
        HStack(spacing: 10) {
            if viewModel.hasGIF {
                Picker("Preview", selection: $viewModel.showsGIFPreview) {
                    Text("GIF").tag(true)
                    Text("Movie").tag(false)
                }
                .pickerStyle(.segmented).labelsHidden().fixedSize()
                .help("Switch the preview between the GIF and the movie it came from")
            }
            Label(viewModel.hasGIF ? "GIF \(viewModel.gifSizeText ?? "")" : viewModel.fileSizeText, systemImage: viewModel.hasGIF ? "photo.stack" : "doc")
                .foregroundStyle(.secondary)
            if viewModel.hasGIF {
                Label("Movie \(viewModel.fileSizeText)", systemImage: "film")
                    .foregroundStyle(.secondary)
            }
            if let summary = viewModel.mediaSummary {
                Text(summary).foregroundStyle(.secondary).monospacedDigit()
            }
            Spacer()
        }
        .font(.caption)
        .lineLimit(1)
    }

    /// One row that fits the 760-point review with a GIF present: the two saves and
    /// the GIF action on the left, copy as a menu, then reveal and trash.
    private var actions: some View {
        HStack(spacing: 8) {
            if viewModel.hasGIF {
                Button("Save GIF As…") { viewModel.saveGIFAs() }
                    .buttonStyle(PrimaryButtonStyle())
                    .keyboardShortcut("s", modifiers: [.command, .shift])
                    .disabled(viewModel.isSaving || viewModel.isConverting)
                Button("Save Movie As…") { viewModel.saveAs() }
                    .buttonStyle(SecondaryButtonStyle())
                    .disabled(viewModel.isSaving)
            } else {
                Button("Save As…") { viewModel.saveAs() }
                    .buttonStyle(SecondaryButtonStyle())
                    .keyboardShortcut("s", modifiers: [.command, .shift])
                    .disabled(viewModel.isSaving)
            }
            Button(viewModel.hasGIF ? "Another GIF…" : "Make GIF…") { viewModel.requestGIFExport() }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(viewModel.isConverting)
                .help("Turn this movie into a silent GIF with your own clip, frame rate and longest edge")
            Spacer(minLength: 4)
            if viewModel.hasGIF {
                Menu {
                    Button("Copy GIF File") { viewModel.copyGIFFile() }
                    Button("Copy Movie File") { viewModel.copyFile() }
                } label: {
                    Text("Copy")
                }
                .menuStyle(.borderlessButton).fixedSize()
                .accessibilityLabel("Copy file")
            } else {
                Button("Copy File") { viewModel.copyFile() }
                    .buttonStyle(SecondaryButtonStyle())
            }
            Button("Show in Finder") { viewModel.revealInFinder() }
                .buttonStyle(SecondaryButtonStyle())
            Button("Move to Trash…", role: .destructive) { viewModel.requestDiscard() }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(viewModel.isSaving || viewModel.isConverting)
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
