import AppKit
import AVKit
import ImageIO
import SwiftUI
import UniformTypeIdentifiers

/// The GIF settings card shared by the recording review and the standalone
/// converter: trim, frame rate, longest edge, looping, and an honest estimate
/// (frames, size in pixels, length). File size is not predicted; it is shown
/// once the GIF exists.
struct GIFExportPanel: View {
    static let preferredSize = CGSize(width: 480, height: 330)

    var title: String
    var subtitle: String
    @Binding var options: GIFExportOptions
    var sourceInfo: GIFSourceInfo?
    var plan: GIFExportPlan?
    var isConverting: Bool
    var progress: Double
    /// Called as the user moves a trim handle so the host can show that moment.
    var onSeek: (TimeInterval) -> Void = { _ in }
    var onConvert: () -> Void
    var onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PopupHeader(icon: "photo.stack", title: title, subtitle: subtitle, onClose: onCancel)

            if let info = sourceInfo {
                GIFTrimControls(options: $options, duration: info.duration, onSeek: onSeek)
                    .disabled(isConverting)
            } else {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Reading the movie…").font(.caption).foregroundStyle(BoxTheme.secondaryText)
                }
                .frame(height: 44)
            }

            GIFFormatControls(options: $options)
                .disabled(isConverting)

            VStack(alignment: .leading, spacing: 3) {
                Text(plan?.summary ?? "Choose a clip to see the estimate")
                    .font(.caption.monospacedDigit())
                    .accessibilityLabel("GIF estimate")
                Text("Silent · file size depends on the content and is shown after writing")
                    .font(.caption2).foregroundStyle(BoxTheme.secondaryText)
            }

            if isConverting {
                ProgressView(value: min(max(progress, 0), 1)).progressViewStyle(.linear).tint(BoxTheme.accent)
                    .accessibilityLabel("GIF progress").accessibilityValue("\(Int((progress * 100).rounded())) percent")
            }

            HStack {
                Spacer()
                Button("Cancel") { onCancel() }
                    .buttonStyle(SecondaryButtonStyle())
                    .keyboardShortcut(.cancelAction)
                Button(isConverting ? "Writing…" : "Convert to GIF…") { onConvert() }
                    .buttonStyle(PrimaryButtonStyle())
                    .keyboardShortcut(.defaultAction)
                    .disabled(isConverting || plan == nil)
                    .help("Choose where to save the GIF, then write it")
            }
        }
        .padding(18)
        .frame(width: Self.preferredSize.width)
        .popupCard()
        .shadow(color: .black.opacity(0.16), radius: 18, y: 8)
        .onExitCommand(perform: onCancel)
    }

    static func timeText(_ seconds: TimeInterval) -> String {
        let clamped = max(0, seconds)
        let whole = Int(clamped)
        let tenths = Int(((clamped - Double(whole)) * 10).rounded(.down))
        return String(format: "%d:%02d.%d", whole / 60, whole % 60, tenths)
    }
}

/// Frame rate, longest edge and looping; shared by every GIF surface.
struct GIFFormatControls: View {
    @Binding var options: GIFExportOptions

    var body: some View {
        HStack(spacing: 14) {
            ToolMenuPicker("Frame rate", value: "\(options.framesPerSecond) fps", showsLabel: false, compact: true, selection: $options.framesPerSecond) {
                ForEach(GIFExportOptions.frameRateChoices, id: \.self) { rate in Text("\(rate) fps").tag(rate) }
            }
            .fixedSize()
            .help("Frames per second in the GIF; fewer frames make a smaller file")
            ToolMenuPicker("Longest edge", value: "\(options.maxWidth) px", showsLabel: false, compact: true, selection: $options.maxWidth) {
                ForEach(GIFExportOptions.widthChoices, id: \.self) { width in Text("\(width) px").tag(width) }
            }
            .fixedSize()
            .help("The longer side of the GIF; the movie is scaled down to it, never up")
            Toggle("Loop", isOn: $options.loops)
                .help("Play again from the start when the GIF ends; off plays it once")
        }
        .font(.caption)
    }
}

/// The clip range. The two sliders hold each other to a valid, bounded clip
/// without leapfrogging: moving Start past End pushes End along (keeping the
/// clip length where it can), moving End before Start pulls Start along, and a
/// clip longer than the limit shortens from the other end. So the last minute
/// of a long movie is one drag of Start away.
struct GIFTrimControls: View {
    @Binding var options: GIFExportOptions
    var duration: TimeInterval
    var onSeek: (TimeInterval) -> Void = { _ in }

    static let minimumClip: TimeInterval = 0.1

    /// Pure trim rules, shared with tests.
    static func movingStart(to value: TimeInterval, options: GIFExportOptions, duration: TimeInterval) -> GIFExportOptions {
        var updated = options
        let total = max(Self.minimumClip, duration)
        let currentEnd = min(options.trimEnd ?? total, total)
        let length = max(Self.minimumClip, currentEnd - options.trimStart)
        let start = min(max(0, value), total - Self.minimumClip)
        var end = currentEnd
        if end < start + Self.minimumClip { end = min(total, start + length) }
        if end - start > GIFExportOptions.maxDuration { end = start + GIFExportOptions.maxDuration }
        updated.trimStart = start
        updated.trimEnd = end
        return updated
    }

    static func movingEnd(to value: TimeInterval, options: GIFExportOptions, duration: TimeInterval) -> GIFExportOptions {
        var updated = options
        let total = max(Self.minimumClip, duration)
        let end = min(max(Self.minimumClip, value), total)
        var start = options.trimStart
        if start > end - Self.minimumClip { start = max(0, end - Self.minimumClip) }
        if end - start > GIFExportOptions.maxDuration { start = end - GIFExportOptions.maxDuration }
        updated.trimStart = start
        updated.trimEnd = end
        return updated
    }

    var body: some View {
        let total = max(Self.minimumClip, duration)
        let end = min(options.trimEnd ?? total, total)
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text("Clip").font(.caption.weight(.semibold)).foregroundStyle(BoxTheme.secondaryText)
                Spacer()
                Text("\(GIFExportPanel.timeText(options.trimStart)) – \(GIFExportPanel.timeText(end)) · \(GIFExportPanel.timeText(end - options.trimStart)) of \(GIFExportPanel.timeText(total))")
                    .font(.caption.monospacedDigit()).foregroundStyle(BoxTheme.secondaryText)
            }
            HStack(spacing: 8) {
                Text("Start").font(.caption2).foregroundStyle(BoxTheme.secondaryText).frame(width: 30, alignment: .leading)
                Slider(value: Binding(
                    get: { options.trimStart },
                    set: { value in
                        options = Self.movingStart(to: value, options: options, duration: duration)
                        onSeek(options.trimStart)
                    }
                ), in: 0...total)
                .accessibilityLabel("Clip start")
                .accessibilityValue(GIFExportPanel.timeText(options.trimStart))
            }
            HStack(spacing: 8) {
                Text("End").font(.caption2).foregroundStyle(BoxTheme.secondaryText).frame(width: 30, alignment: .leading)
                Slider(value: Binding(
                    get: { end },
                    set: { value in
                        options = Self.movingEnd(to: value, options: options, duration: duration)
                        onSeek(options.trimEnd ?? total)
                    }
                ), in: 0...total)
                .accessibilityLabel("Clip end")
                .accessibilityValue(GIFExportPanel.timeText(end))
            }
            if total > GIFExportOptions.maxDuration {
                Text("GIF clips are limited to \(Int(GIFExportOptions.maxDuration)) seconds; the other end follows to keep within it.")
                    .font(.caption2).foregroundStyle(BoxTheme.secondaryText)
                    .lineLimit(2).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// The standalone converter: pick a movie with the file chooser, set the GIF
/// options while watching the source, choose where to save, watch real
/// progress, then preview the result. Everything stays on this Mac; no path is
/// ever interpreted as a command.
@MainActor
final class VideoToGIFViewModel: ObservableObject {
    typealias GIFTranscode = (URL, URL, GIFExportOptions, @escaping GIFTranscoder.Progress) async throws -> GIFExportResult

    @Published private(set) var sourceURL: URL?
    @Published private(set) var sourceInfo: GIFSourceInfo?
    @Published private(set) var player: AVPlayer?
    @Published var options: GIFExportOptions
    @Published private(set) var isConverting = false
    @Published private(set) var progress: Double = 0
    @Published private(set) var result: GIFExportResult?
    /// Changes with every finished conversion, so a preview reloads even when the
    /// GIF was written over the same path.
    @Published private(set) var resultRevision = 0
    @Published var showsResult = true
    @Published var errorMessage: String?
    @Published var statusMessage: String?
    private let transcodeGIF: GIFTranscode
    private var task: Task<GIFExportResult, Error>?
    private var infoTask: Task<Void, Never>?
    /// Bumped on every load, convert and close so late results are dropped.
    private var generation = 0
    var onClose: () -> Void = {}

    init(options: GIFExportOptions = .default,
         transcodeGIF: @escaping GIFTranscode = { try await GIFTranscoder.transcode(sourceURL: $0, to: $1, options: $2, progress: $3) }) {
        self.options = options
        self.transcodeGIF = transcodeGIF
    }

    deinit {
        task?.cancel()
        infoTask?.cancel()
    }

    var plan: GIFExportPlan? {
        guard let sourceInfo else { return nil }
        return try? GIFExportPlan.make(source: sourceInfo, options: options)
    }

    func chooseVideo() {
        guard !isConverting else { return }
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.movie, .quickTimeMovie, .mpeg4Movie]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.message = "Choose a movie to turn into a GIF"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        load(url)
    }

    /// Uses a movie the caller already chose (review fixtures, tests).
    func load(_ url: URL) {
        guard !isConverting else { return }
        generation += 1
        let generation = generation
        infoTask?.cancel()
        player?.pause()
        sourceURL = url
        sourceInfo = nil
        result = nil
        showsResult = true
        errorMessage = nil
        statusMessage = nil
        options.trimStart = 0
        options.trimEnd = nil
        let player = AVPlayer(url: url)
        player.actionAtItemEnd = .pause
        self.player = player
        infoTask = Task { [weak self] in
            do {
                let info = try await GIFSourceInfo.load(url: url)
                guard let self, !Task.isCancelled, self.generation == generation else { return }
                self.sourceInfo = info
                self.options.trimEnd = min(info.duration, GIFExportOptions.maxDuration)
            } catch {
                guard let self, !Task.isCancelled, self.generation == generation else { return }
                self.errorMessage = error.localizedDescription
            }
        }
    }

    /// Shows the frame at `time` in the source preview.
    func seek(to time: TimeInterval) {
        player?.pause()
        player?.seek(to: CMTime(seconds: max(0, time), preferredTimescale: 600), toleranceBefore: .zero, toleranceAfter: .zero)
    }

    func convert() {
        guard let sourceURL, !isConverting, plan != nil else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.gif]
        panel.canCreateDirectories = true
        panel.nameFieldStringValue = sourceURL.deletingPathExtension().lastPathComponent + ".gif"
        guard panel.runModal() == .OK, let destination = panel.url else { return }
        Task { await convert(to: destination) }
    }

    func convert(to destination: URL) async {
        guard let sourceURL, !isConverting else { return }
        generation += 1
        let generation = generation
        isConverting = true
        progress = 0
        errorMessage = nil
        statusMessage = nil
        player?.pause()
        let options = options
        let reporter = ProgressThrottle()
        let task = Task { [weak self] () -> GIFExportResult in
            guard let self else { throw CancellationError() }
            return try await self.transcodeGIF(sourceURL, destination, options) { progress in
                guard reporter.shouldReport(progress) else { return }
                Task { @MainActor [weak self] in
                    guard let self, self.generation == generation, self.isConverting else { return }
                    self.progress = progress
                }
            }
        }
        self.task = task
        do {
            let result = try await task.value
            guard self.generation == generation else { return }
            self.result = result
            resultRevision += 1
            showsResult = true
            statusMessage = "Saved \(result.url.lastPathComponent) · \(ByteCountFormatter.string(fromByteCount: result.fileSize, countStyle: .file))"
        } catch is CancellationError {
            guard self.generation == generation else { return }
            statusMessage = "Conversion cancelled. Nothing was written."
        } catch {
            guard self.generation == generation else { return }
            errorMessage = error.localizedDescription
        }
        guard self.generation == generation else { return }
        self.task = nil
        isConverting = false
    }

    func cancel() {
        task?.cancel()
    }

    /// Stops everything in flight; results that arrive afterwards are ignored.
    func close() {
        generation += 1
        task?.cancel()
        task = nil
        infoTask?.cancel()
        infoTask = nil
        player?.pause()
        isConverting = false
        onClose()
    }

    func revealResult() {
        guard let result else { return }
        NSWorkspace.shared.activateFileViewerSelecting([result.url])
    }

    func copyResultFile() {
        guard let result else { return }
        NSPasteboard.general.clearContents()
        statusMessage = NSPasteboard.general.writeObjects([result.url as NSURL]) ? "Copied GIF file." : "Could not copy the GIF file."
    }
}

/// The converter window: a fixed size, so `VideoToGIFContent` is what has to
/// fit. Only the preview is allowed to give up height, and it never goes below
/// its minimum; every control, the header and the footer keep their natural
/// size and padding in every state.
struct VideoToGIFView: View {
    static let preferredSize = CGSize(width: 560, height: 600)
    /// The preview fills what is free (a taller movie to scrub before converting)
    /// and shrinks, never below the minimum, when the result row, a two-line
    /// status or the long-movie note need room.
    static let previewHeightRange: ClosedRange<CGFloat> = 120...240
    @ObservedObject var viewModel: VideoToGIFViewModel
    var onMinimize: () -> Void

    var body: some View {
        VideoToGIFContent(viewModel: viewModel, onMinimize: onMinimize)
            .frame(width: Self.preferredSize.width, height: Self.preferredSize.height)
            .popupCard()
            .onExitCommand(perform: viewModel.close)
    }
}

/// The converter at the window's width but its natural height (preview at its
/// maximum), so tests can measure what the fixed window has to hold.
struct VideoToGIFContent: View {
    @ObservedObject var viewModel: VideoToGIFViewModel
    var onMinimize: () -> Void

    var body: some View {
        let previewHeights = VideoToGIFView.previewHeightRange
        VStack(alignment: .leading, spacing: 12) {
            PopupHeader(icon: "film.stack", title: "Video to GIF",
                        subtitle: viewModel.sourceURL?.lastPathComponent ?? "Converted on this Mac",
                        onMinimize: onMinimize, onClose: viewModel.close)

            if viewModel.sourceURL == nil {
                VStack(spacing: 10) {
                    Image(systemName: "film").font(.system(size: 34)).foregroundStyle(BoxTheme.accent)
                    Text("Choose a movie to convert").font(.callout.weight(.semibold))
                    Text("MOV or MP4 from this Mac. The GIF is silent and can be trimmed, resized and looped.")
                        .font(.caption).foregroundStyle(BoxTheme.secondaryText).multilineTextAlignment(.center)
                    Button("Choose Video…", action: viewModel.chooseVideo)
                        .buttonStyle(PrimaryButtonStyle())
                        .keyboardShortcut("o", modifiers: .command)
                }
                .frame(maxWidth: .infinity)
                .frame(height: 240)
                .surfaceCard()
            } else {
                preview
                    .frame(maxWidth: .infinity)
                    .frame(minHeight: previewHeights.lowerBound, idealHeight: previewHeights.upperBound, maxHeight: previewHeights.upperBound)
                    .background(BoxTheme.well)
                    .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(.primary.opacity(0.08), lineWidth: 1))
                    // Laid out first, offered only what the rigid rows leave: it is the
                    // one child that can absorb a taller state.
                    .layoutPriority(1)
                VideoToGIFSourceCard(viewModel: viewModel)
                GIFExportPanelBody(
                    options: $viewModel.options,
                    sourceInfo: viewModel.sourceInfo,
                    plan: viewModel.plan,
                    isConverting: viewModel.isConverting,
                    progress: viewModel.progress,
                    onSeek: viewModel.seek(to:),
                    onConvert: viewModel.convert,
                    onCancel: viewModel.cancel
                )
            }

            if let error = viewModel.errorMessage {
                Label(error, systemImage: "exclamationmark.triangle.fill").font(.caption).foregroundStyle(BoxTheme.danger)
                    .lineLimit(2).fixedSize(horizontal: false, vertical: true)
            } else if let status = viewModel.statusMessage {
                Label(status, systemImage: "checkmark.circle.fill").font(.caption).foregroundStyle(BoxTheme.secondaryText)
                    .lineLimit(2).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
            Text("Converted on this Mac. Nothing is uploaded.")
                .font(.caption2).foregroundStyle(BoxTheme.secondaryText)
        }
        .padding(18)
        .frame(width: VideoToGIFView.preferredSize.width)
    }

    /// The source movie (seekable from the trim handles) or, once written, the GIF.
    @ViewBuilder private var preview: some View {
        if let result = viewModel.result, viewModel.showsResult {
            AnimatedGIFView(url: result.url, revision: viewModel.resultRevision)
                .accessibilityLabel("GIF preview")
        } else if let player = viewModel.player {
            RecordingPlayerView(player: player)
                .accessibilityLabel("Movie preview")
        }
    }
}

/// The card under the preview: the source row (file, length, size, Choose
/// Another…) looks the same before and after conversion, and a second row
/// appears with the result: preview switch, the GIF's frames, size and bytes,
/// and copy/reveal as icon buttons with full help and accessibility labels.
/// Only the file name is allowed to truncate; every other label is fixed-size.
struct VideoToGIFSourceCard: View {
    @ObservedObject var viewModel: VideoToGIFViewModel

    static func resultSummary(_ result: GIFExportResult) -> String {
        "\(result.frameCount) frames · \(Int(result.size.width)) × \(Int(result.size.height)) · \(ByteCountFormatter.string(fromByteCount: result.fileSize, countStyle: .file))"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Label(viewModel.sourceURL?.lastPathComponent ?? "", systemImage: "film")
                    .font(.caption).lineLimit(1).truncationMode(.middle)
                    .layoutPriority(-1)
                    .help(viewModel.sourceURL?.path ?? "")
                if let info = viewModel.sourceInfo {
                    Text("\(GIFExportPanel.timeText(info.duration)) · \(Int(info.displaySize.width)) × \(Int(info.displaySize.height))")
                        .font(.caption.monospacedDigit()).foregroundStyle(BoxTheme.secondaryText).lineLimit(1).fixedSize()
                        .accessibilityLabel("Movie length and size")
                }
                Spacer(minLength: 8)
                Button("Choose Another…", action: viewModel.chooseVideo)
                    .buttonStyle(SecondaryButtonStyle())
                    .fixedSize()
                    .disabled(viewModel.isConverting)
            }
            if let result = viewModel.result {
                Divider()
                HStack(spacing: 8) {
                    ToolChoiceBar(selection: $viewModel.showsResult, choices: [(true, "GIF"), (false, "Movie")], label: "Preview", compact: true).fixedSize()
                    .help("Switch the preview between the GIF and the movie it came from")
                    .accessibilityLabel("Preview")
                    Label(Self.resultSummary(result), systemImage: "photo.stack")
                        .font(.caption.monospacedDigit()).foregroundStyle(BoxTheme.secondaryText).lineLimit(1).fixedSize()
                        .accessibilityLabel("GIF result: \(Self.resultSummary(result))")
                    Spacer(minLength: 8)
                    Button(action: viewModel.copyResultFile) { Image(systemName: "doc.on.doc") }
                        .buttonStyle(SecondaryButtonStyle())
                        .help("Copy the GIF file")
                        .accessibilityLabel("Copy GIF file")
                    Button(action: viewModel.revealResult) { Image(systemName: "folder") }
                        .buttonStyle(SecondaryButtonStyle())
                        .help("Show the GIF in Finder")
                        .accessibilityLabel("Show GIF in Finder")
                }
            }
        }
        .padding(8).surfaceCard()
    }
}

/// The controls of `GIFExportPanel` without the card chrome, for hosts that
/// already have a header.
struct GIFExportPanelBody: View {
    @Binding var options: GIFExportOptions
    var sourceInfo: GIFSourceInfo?
    var plan: GIFExportPlan?
    var isConverting: Bool
    var progress: Double
    var onSeek: (TimeInterval) -> Void = { _ in }
    var onConvert: () -> Void
    var onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if let info = sourceInfo {
                GIFTrimControls(options: $options, duration: info.duration, onSeek: onSeek).disabled(isConverting)
            } else {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Reading the movie…").font(.caption).foregroundStyle(BoxTheme.secondaryText)
                }
            }
            GIFFormatControls(options: $options).disabled(isConverting)
            HStack(spacing: 10) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(plan?.summary ?? "Reading…").font(.caption.monospacedDigit())
                    Text("Silent · size shown after writing").font(.caption2).foregroundStyle(BoxTheme.secondaryText)
                }
                Spacer()
                if isConverting {
                    ProgressView(value: min(max(progress, 0), 1)).progressViewStyle(.linear).tint(BoxTheme.accent).frame(width: 120)
                    Text("\(Int((progress * 100).rounded()))%").font(.caption.monospacedDigit())
                    Button("Cancel", action: onCancel).buttonStyle(SecondaryButtonStyle())
                } else {
                    Button("Convert to GIF…", action: onConvert)
                        .buttonStyle(PrimaryButtonStyle())
                        .keyboardShortcut(.defaultAction)
                        .disabled(plan == nil)
                        .help("Choose where to save the GIF, then write it")
                }
            }
        }
        .padding(10).surfaceCard()
    }
}

/// Plays a GIF file frame by frame from an image source, decoding one frame at a
/// time so a long GIF never sits in memory as a stack of bitmaps. It honours the
/// file's repeat block the way browsers (Chromium, libwebp) do: no block plays
/// once, 0 repeats forever, N means N extra repeats (N + 1 plays). `revision`
/// forces a reload when the file at the same URL was rewritten.
struct AnimatedGIFView: NSViewRepresentable {
    var url: URL
    var revision: Int = 0

    func makeNSView(context: Context) -> AnimatedGIFNSView {
        let view = AnimatedGIFNSView()
        view.load(url, revision: revision)
        return view
    }

    func updateNSView(_ view: AnimatedGIFNSView, context: Context) {
        if view.url != url || view.revision != revision { view.load(url, revision: revision) }
    }

    static func dismantleNSView(_ view: AnimatedGIFNSView, coordinator: ()) {
        view.stop()
    }
}

final class AnimatedGIFNSView: NSView {
    private(set) var url: URL?
    private(set) var revision = 0
    private var source: CGImageSource?
    private(set) var frameCount = 0
    private(set) var frameIndex = 0
    /// How many complete plays remain, or nil for forever.
    private(set) var playsRemaining: Int?
    private(set) var loopCount: Int?
    private var timer: DispatchSourceTimer?
    private var current: CGImage?
    private let playButton = NSButton()
    private(set) var isPlaying = false
    /// GIFs beyond this many frames are shown as a still first frame with Play on demand.
    static let autoplayFrameLimit = 3_000
    /// Test seam: when set, overrides the system Reduce Motion setting.
    var reduceMotionOverride: Bool?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        playButton.bezelStyle = .rounded
        playButton.title = "Play"
        playButton.target = self
        playButton.action = #selector(togglePlayback)
        playButton.setAccessibilityLabel("Play or pause the GIF")
        addSubview(playButton)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    deinit { timer?.cancel() }

    /// The repeat count in the file's NETSCAPE block: nil when it has none (play
    /// once), 0 for forever, otherwise that many extra repeats. Read from the file
    /// structure, because ImageIO reports 1 for a file that has no block.
    static func loopCount(at url: URL) -> Int? {
        guard let data = try? Data(contentsOf: url, options: .mappedIfSafe) else { return nil }
        return GIFLoopBlock.locate(in: data)?.loopCount
    }

    /// Total plays for a repeat count, or nil for forever.
    static func plays(forLoopCount loopCount: Int?) -> Int? {
        guard let loopCount else { return 1 }
        return loopCount == 0 ? nil : loopCount + 1
    }

    func load(_ url: URL, revision: Int) {
        stop()
        self.url = url
        self.revision = revision
        source = CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary)
        frameCount = source.map(CGImageSourceGetCount) ?? 0
        loopCount = Self.loopCount(at: url)
        playsRemaining = Self.plays(forLoopCount: loopCount)
        frameIndex = 0
        current = frame(at: 0)
        needsDisplay = true
        let reduceMotion = reduceMotionOverride ?? NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        if frameCount > 1, frameCount <= Self.autoplayFrameLimit, !reduceMotion { play() } else { updateButton() }
    }

    func stop() {
        timer?.cancel()
        timer = nil
        isPlaying = false
        updateButton()
    }

    func play() {
        guard frameCount > 1 else { return }
        if playsRemaining == 0 {
            // Finished its plays: Play starts it again from the top, once more.
            playsRemaining = 1
            frameIndex = 0
            current = frame(at: 0)
            needsDisplay = true
        }
        isPlaying = true
        updateButton()
        scheduleNext()
    }

    @objc private func togglePlayback() {
        if isPlaying { stop() } else { play() }
    }

    private func updateButton() {
        playButton.title = isPlaying ? "Pause" : (playsRemaining == 0 ? "Play Again" : "Play")
        playButton.isHidden = frameCount <= 1
    }

    private func scheduleNext() {
        timer?.cancel()
        let delay = delay(at: frameIndex)
        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now() + delay)
        timer.setEventHandler { [weak self] in
            guard let self, self.isPlaying else { return }
            self.advance()
        }
        self.timer = timer
        timer.resume()
    }

    /// Steps to the next frame; at the end of a play, counts it and stops when the
    /// file asked for a finite number of plays.
    func advance() {
        guard frameCount > 1 else { return }
        let last = frameCount - 1
        if frameIndex >= last {
            if let remaining = playsRemaining {
                let left = remaining - 1
                playsRemaining = left
                if left <= 0 {
                    stop()
                    return
                }
            }
            frameIndex = 0
        } else {
            frameIndex += 1
        }
        current = frame(at: frameIndex)
        needsDisplay = true
        if isPlaying { scheduleNext() }
    }

    private func frame(at index: Int) -> CGImage? {
        guard let source, index < frameCount else { return nil }
        return CGImageSourceCreateImageAtIndex(source, index, [kCGImageSourceShouldCache: false] as CFDictionary)
    }

    private func delay(at index: Int) -> TimeInterval {
        guard let source, index < frameCount,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any],
              let gif = properties[kCGImagePropertyGIFDictionary] as? [CFString: Any]
        else { return 0.1 }
        let unclamped = gif[kCGImagePropertyGIFUnclampedDelayTime] as? Double
        let clamped = gif[kCGImagePropertyGIFDelayTime] as? Double
        let value = unclamped ?? clamped ?? 0.1
        return max(0.02, value)
    }

    /// The pixel at the centre of the frame being shown, for tests.
    func currentCentrePixel() -> [UInt8]? {
        guard let current else { return nil }
        var data = [UInt8](repeating: 0, count: 4)
        guard let context = CGContext(data: &data, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        context.translateBy(x: CGFloat(-current.width / 2), y: CGFloat(-current.height / 2))
        context.draw(current, in: CGRect(x: 0, y: 0, width: current.width, height: current.height))
        return data
    }

    override func layout() {
        super.layout()
        playButton.sizeToFit()
        playButton.frame.origin = CGPoint(x: bounds.maxX - playButton.frame.width - 10, y: 10)
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let image = current, let context = NSGraphicsContext.current?.cgContext else { return }
        let imageSize = CGSize(width: image.width, height: image.height)
        guard imageSize.width > 0, imageSize.height > 0 else { return }
        let scale = min(bounds.width / imageSize.width, bounds.height / imageSize.height, 1)
        let size = CGSize(width: imageSize.width * scale, height: imageSize.height * scale)
        let rect = CGRect(x: (bounds.width - size.width) / 2, y: (bounds.height - size.height) / 2, width: size.width, height: size.height)
        context.interpolationQuality = .medium
        context.draw(image, in: rect)
    }
}
