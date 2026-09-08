import AppKit
import SwiftUI

// MARK: - QR Code

/// A real, scannable code for the draft text, an editor that regenerates it,
/// and explicit Copy Image / Save… actions. Enter opens the popup with the
/// edited text.
struct LauncherQRPreviewView: View {
    @ObservedObject var model: LauncherQRPreview
    let height: CGFloat?
    var hostWindow: () -> NSWindow?
    var onEscape: () -> Void
    var onOpen: () -> Void = {}

    private var code: QRCodePopupViewModel { model.code }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !model.fitsPreviewLimit {
                LauncherDraftLimitNotice(byteCount: code.byteCount,
                    detail: "A QR code holds up to \(QRCodeGenerator.maxByteCount.formatted()) bytes. The complete text stays in the QR popup; shorten it there or replace the input to preview here again.",
                    openTitle: "Open QR Code", onOpen: onOpen)
            } else {
                header
                HStack(alignment: .top, spacing: 10) {
                    card
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Encoded text").previewCaption()
                        LauncherPreviewEditor(text: Binding(get: { code.text }, set: { code.text = $0 }), label: "Encoded text", monospaced: false)
                    }
                }.frame(maxHeight: .infinity)
                footer
            }
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: height, alignment: .top)
        .clipped()
    }

    private var title: String {
        guard code.image != nil else { return code.isEmpty ? "Enter text to encode" : "Not encodable" }
        if model.isEnlarged { return "Scannable at this size" }
        return model.isDense ? "Dense code · enlarge to scan" : "Scannable"
    }
    private var subtitle: String {
        var parts = ["\(code.byteCount.formatted()) / \(QRCodeGenerator.maxByteCount.formatted()) bytes"]
        if let modules = model.moduleCount { parts.append("\(modules) × \(modules) modules") }
        if code.isTooLong { parts.append(code.capacityMessage) }
        return parts.joined(separator: " · ")
    }
    private var header: some View {
        LauncherPreviewHeader(title: title, subtitle: subtitle, warning: code.isTooLong) {
            Button(model.isEnlarged ? "Compact" : "Enlarge") { model.isEnlarged.toggle() }
                .buttonStyle(LauncherChipButtonStyle(prominent: model.isDense && !model.isEnlarged)).disabled(code.image == nil)
                .help(model.isEnlarged ? "Back to the compact card" : "Show the code at \(Int(model.largeCardPoints)) points, two pixels per module on a standard display")
                .accessibilityIdentifier("launcherPreviewQREnlarge")
            Button("Paste", action: code.pasteText).buttonStyle(LauncherChipButtonStyle()).help("Use text from your clipboard")
            Button("Clear") { code.text = "" }.buttonStyle(LauncherChipButtonStyle()).disabled(code.text.isEmpty)
        }
    }

    private var statusText: String {
        if let error = code.errorMessage { return error }
        if let status = code.statusMessage { return status }
        if code.image == nil { return "Copy and save need an encodable text" }
        if model.isDense && !model.isEnlarged {
            return "Too small to scan from a standard display · Enlarge, or copy or save the \(model.exportPixelSize) px image"
        }
        return "Scan it from the screen, or copy or save the \(model.exportPixelSize) px image"
    }
    private var footer: some View {
        HStack(spacing: 8) {
            LauncherStatusLine(text: statusText, warning: code.errorMessage != nil)
            Spacer(minLength: 6)
            Button { code.save(in: hostWindow()) } label: { Label("Save…", systemImage: "square.and.arrow.down") }
                .buttonStyle(LauncherChipButtonStyle()).disabled(code.image == nil).help("Save the QR image as PNG")
                .accessibilityIdentifier("launcherPreviewQRSave")
            Button(action: code.copyImage) { Label("Copy Image", systemImage: "doc.on.doc") }
                .buttonStyle(LauncherChipButtonStyle(prominent: true)).disabled(code.image == nil).help("Copy the QR image")
                .accessibilityIdentifier("launcherPreviewQRCopy")
        }
    }

    private var card: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 8, style: .continuous).fill(.white)
            if let image = model.cardImage {
                Image(nsImage: image).resizable().interpolation(.none).scaledToFit().padding(LauncherQRPreview.cardInset)
                    .accessibilityLabel("QR code for the encoded text")
            } else {
                Image(systemName: code.isEmpty ? "qrcode" : "exclamationmark.triangle").font(.system(size: 22))
                    .foregroundStyle(Color.black.opacity(0.5))
            }
        }
        .frame(width: model.cardPoints, height: model.cardPoints)
        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(BoxTheme.border))
        .accessibilityIdentifier("launcherPreviewQRCard")
    }
}

// MARK: - Text Tools

/// Case, encode, decode, pretty-print, hashes, lines, and counts of the
/// selection, with the same engines as the popup. Enter opens Text Tools on
/// the chosen category and option.
struct LauncherTextToolsPreviewView: View {
    @ObservedObject var model: TextToolsPopupViewModel
    let height: CGFloat?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            LauncherPreviewHeader(title: title, subtitle: subtitle)
            HStack(spacing: 8) {
                LauncherChoiceBar(selection: $model.category, choices: TextToolsPopupViewModel.Category.allCases.map { ($0, $0.rawValue) }, label: "Text tool")
                options
                Spacer(minLength: 0)
            }
            output.frame(maxWidth: .infinity, maxHeight: .infinity)
            HStack(spacing: 8) {
                LauncherStatusLine(text: model.statusMessage ?? "\(model.input.count.formatted()) characters · processed on your Mac")
                Spacer(minLength: 6)
                Button { model.copy(model.copyableOutput) } label: { Label("Copy", systemImage: "doc.on.doc") }
                    .buttonStyle(LauncherChipButtonStyle()).disabled(model.copyableOutput.isEmpty)
                    .help("Copy the complete result").accessibilityIdentifier("launcherPreviewCopy")
            }
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: height, alignment: .top)
        .clipped()
    }

    private var title: String {
        switch model.category {
        case .caseConvert: return model.caseStyle.rawValue
        case .encode: return "Encoded as \(model.encodeMethod.rawValue)"
        case .decode: return model.decodeResult.map { "Decoded \($0.format)" } ?? "Nothing to decode"
        case .pretty: return model.prettyResult.map { "Pretty-printed \($0.language)" } ?? "Not JSON, XML, HTML, or code"
        case .hash: return "Hashes"
        case .lines: return model.lineOp.rawValue
        case .count: return "Counts"
        }
    }
    private var subtitle: String? {
        switch model.category {
        case .decode: return model.decodeResult == nil ? "Base64, URL, HTML entities, or hex" : nil
        case .count: return "Token estimate for \(model.modelLabel)"
        default: return nil
        }
    }

    @ViewBuilder private var options: some View {
        switch model.category {
        case .caseConvert:
            LauncherOptionMenu(selection: $model.caseStyle, choices: CaseConverter.Style.allCases.map { ($0, $0.rawValue) }, label: "Case style", symbol: "textformat")
        case .encode:
            LauncherChoiceBar(selection: $model.encodeMethod, choices: TextEncoder.Method.allCases.map { ($0, $0.rawValue) }, label: "Encoding")
        case .decode:
            LauncherChoiceBar(selection: $model.decodeFormat, choices: TextDecoder.Format.allCases.map { ($0, $0 == .auto ? "Auto" : $0.rawValue) }, label: "Decoding")
        case .lines:
            LauncherOptionMenu(selection: $model.lineOp, choices: LineTool.Operation.allCases.map { ($0, $0.rawValue) }, label: "Line operation", symbol: "list.bullet")
        case .pretty:
            Text("JSON, XML, HTML, or brace code, detected automatically").previewCaption()
        case .hash:
            Text("MD5, SHA-1, SHA-256, SHA-512 of the UTF-8 text").previewCaption()
        case .count:
            EmptyView()
        }
    }

    @ViewBuilder private var output: some View {
        switch model.category {
        case .hash:
            LauncherOutputWell {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(model.hashes, id: \.0) { algorithm, value in
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Text(algorithm.rawValue).previewCaption().frame(width: 52, alignment: .leading)
                            Text(value).font(.system(size: 10, design: .monospaced)).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                            Spacer(minLength: 2)
                            Button { model.copy(value, label: algorithm.rawValue) } label: { Image(systemName: "doc.on.doc").font(.system(size: 9)) }
                                .buttonStyle(.plain).foregroundStyle(.secondary).help("Copy \(algorithm.rawValue)")
                        }
                    }
                }.padding(8).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
        case .count:
            HStack(spacing: 8) {
                ForEach(model.stats, id: \.0) { label, value in statistic(label, value) }
                statistic("Tokens ≈", model.tokenEstimate.formatted())
            }
        default:
            LauncherOutputWell {
                if let text = model.primaryOutput, !text.isEmpty {
                    LauncherOutputText(text: text, label: "\(model.category.rawValue) result", monospaced: model.category != .caseConvert)
                } else {
                    Text(model.input.isEmpty ? "No text selected." : "No result for this text.")
                        .font(.system(size: 11)).foregroundStyle(.secondary).padding(8)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                }
            }
        }
    }
    private func statistic(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label).font(.system(size: 9)).foregroundStyle(.secondary).lineLimit(1)
            Text(value).font(.system(size: 20, weight: .medium, design: .rounded)).monospacedDigit().lineLimit(1).minimumScaleFactor(0.7)
        }.padding(10).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(BoxTheme.border))
    }
}

// MARK: - Ask AI

/// A prompt and the quick actions. Nothing is sent from here: an action or
/// Send opens Ask AI, which runs that one request with the selection.
struct LauncherAIPreviewView: View {
    @ObservedObject var model: LauncherAIPreview
    let height: CGFloat?
    var onEscape: () -> Void
    var onOpenSettings: () -> Void
    var onRun: (AIHandoff) -> Void

    private let columns = Array(repeating: GridItem(.flexible(), spacing: 6), count: 4)

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            LauncherPreviewHeader(title: model.hasSelection ? "Ask about the selection" : "Ask a question",
                                  subtitle: model.hasSelection ? "\(model.characterCount.formatted()) characters · \(model.providerSummary)" : "No text selected · \(model.providerSummary)",
                                  warning: !model.isConfigured) {
                if !model.isConfigured {
                    Button("Open Settings", action: onOpenSettings).buttonStyle(LauncherChipButtonStyle())
                        .accessibilityIdentifier("launcherPreviewAISettings")
                }
            }
            HStack(spacing: 8) {
                Image(systemName: "sparkles").font(.system(size: 11)).foregroundStyle(BoxTheme.accent).accessibilityHidden(true)
                LauncherPreviewField(text: $model.draft, placeholder: model.hasSelection ? "Ask Bello Box to…" : "Ask Bello Box a question…",
                                     label: "Ask AI instruction", monospaced: false, onSubmit: send, onEscape: onEscape)
                Button(action: send) { Label("Send", systemImage: "arrow.up") }
                    .buttonStyle(LauncherChipButtonStyle(prominent: true)).disabled(!model.canSend)
                    .help("Open Ask AI and send this instruction now")
                    .accessibilityIdentifier("launcherPreviewAISend")
            }
            LazyVGrid(columns: columns, spacing: 6) {
                ForEach(model.quickActions) { action in
                    Button { if let handoff = model.handoff(for: action) { onRun(handoff) } } label: {
                        Label(action.title, systemImage: action.symbol).font(.system(size: 10, weight: .medium)).lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 8).frame(height: 24)
                            .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 6))
                            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(BoxTheme.border))
                            .contentShape(RoundedRectangle(cornerRadius: 6))
                    }
                    .buttonStyle(.plain).disabled(!model.canRunQuickActions)
                    .help(model.hasSelection ? "Open Ask AI and run: \(action.title)" : "\(action.title) needs selected text")
                    .accessibilityIdentifier("launcherPreviewAI_\(action.id)")
                }
            }
            LauncherStatusLine(text: model.isConfigured
                ? "Nothing is sent until you choose an action or Send · ↵ opens Ask AI with your prompt, unsent"
                : "Your prompt is kept · ↵ opens Ask AI with it · connect a provider in Settings to send")
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: height, alignment: .top)
        .clipped()
    }
    /// Send, or Return inside the prompt field: the one explicit request.
    private func send() {
        guard let handoff = model.sendHandoff else { return }
        onRun(handoff)
    }
}

// MARK: - Screenshot and Scrolling Screenshot

struct LauncherCapturePreviewView: View {
    let command: LauncherCommand
    @ObservedObject var model: LauncherCapturePreview
    let height: CGFloat?
    var onCapture: (ScreenshotCaptureMode) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            LauncherPreviewHeader(title: model.hasScreenRecordingPermission ? (command == .scrollCapture ? "Ready to capture a scrolling page" : "Ready to capture")
                                                                           : "Screen Recording permission needed",
                                  subtitle: model.hasScreenRecordingPermission
                                    ? "Stays on this Mac · cursor \(model.includesCursor ? "shown" : "hidden") · default: \(model.defaultMode.label)"
                                    : "Grant it once in System Settings, then choose what to capture",
                                  warning: !model.hasScreenRecordingPermission) {
                if !model.hasScreenRecordingPermission {
                    Button("Grant Screen Recording…", action: model.requestPermission).buttonStyle(LauncherChipButtonStyle(prominent: true))
                        .accessibilityIdentifier("launcherPreviewGrantScreenRecording")
                }
            }
            if command == .scrollCapture {
                HStack(spacing: 8) {
                    modeButton(.scrolling, title: "Select Area & Scroll", detail: "Scroll yourself or auto-scroll; frames are stitched into one tall image")
                }
            } else {
                HStack(spacing: 8) {
                    ForEach(model.modes) { mode in modeButton(mode, title: mode.label, detail: help(for: mode)) }
                }
            }
            LauncherStatusLine(text: command == .scrollCapture
                ? "Review at fit, fit width, or 100% · annotate, mask, and run local OCR afterwards"
                : "↵ starts the default mode · annotate, mask, erase, crop, then copy or save · local OCR")
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: height, alignment: .top)
        .clipped()
        .onAppear(perform: model.refreshPermission)
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in model.refreshPermission() }
    }

    private func modeButton(_ mode: ScreenshotCaptureMode, title: String, detail: String) -> some View {
        Button { onCapture(mode) } label: {
            HStack(spacing: 8) {
                ToolBadge(symbol: mode.symbol, size: 24)
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 4) {
                        Text(title).font(.system(size: 11, weight: .semibold)).lineLimit(1)
                        if mode == model.defaultMode, command != .scrollCapture {
                            Text("↵").font(.system(size: 9, weight: .medium)).foregroundStyle(BoxTheme.accent)
                                .padding(.horizontal, 4).frame(height: 14).background(BoxTheme.accentSoft, in: RoundedRectangle(cornerRadius: 3))
                        }
                    }
                    if command == .scrollCapture {
                        Text(detail).font(.system(size: 10)).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(8).frame(maxWidth: .infinity, alignment: .leading)
        }
        .buttonStyle(ToolCardButtonStyle()).help(detail)
        .disabled(!model.hasScreenRecordingPermission)
        .accessibilityIdentifier("launcherPreviewCapture_\(mode.id)")
    }
    private func help(for mode: ScreenshotCaptureMode) -> String {
        switch mode {
        case .area: return "Drag a region to capture."
        case .window: return "Choose a window to capture."
        case .screen: return "Capture the display under the pointer."
        case .scrolling: return "Select an area, then scroll or auto-scroll to capture more and stitch it."
        }
    }
}

// MARK: - Screen Recording

struct LauncherRecordingPreviewView: View {
    @ObservedObject var model: LauncherRecordingPreview
    let height: CGFloat?
    var onStart: (RecordingOptions) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            LauncherPreviewHeader(title: model.canRecordVideo ? "Ready to record" : "Screen Recording permission needed",
                                  subtitle: model.canRecordVideo ? model.summary : "Grant it once in System Settings, then choose what to record",
                                  warning: !model.canRecordVideo) {
                if !model.canRecordVideo {
                    Button("Grant Screen Recording…", action: model.requestScreenRecording).buttonStyle(LauncherChipButtonStyle(prominent: true))
                        .accessibilityIdentifier("launcherPreviewGrantScreenRecording")
                }
            }
            HStack(spacing: 8) {
                Text("Output").previewCaption()
                LauncherChoiceBar(selection: $model.options.outputFormat, choices: RecordingOutputFormat.allCases.map { ($0, $0.label) }, label: "Output format")
                Text("Quality").previewCaption()
                LauncherChoiceBar(selection: $model.options.quality, choices: RecordingQualityPreset.allCases.map { ($0, $0.label) }, label: "Quality")
                Spacer(minLength: 0)
                countdown
            }
            HStack(spacing: 8) {
                Text("Audio").previewCaption()
                Menu {
                    ForEach(RecordingAudioSource.allCases) { source in Button(source.label) { model.options.audioSource = source } }
                } label: { Text(model.options.audioSource.label).font(.system(size: 10)) }
                    .menuStyle(.borderlessButton).fixedSize().accessibilityLabel("Audio source").accessibilityIdentifier("launcherPreviewAudio")
                Toggle("Cursor", isOn: $model.options.includeCursor).toggleStyle(.checkbox).controlSize(.mini).previewCaption().fixedSize()
                Toggle("Click rings", isOn: Binding(get: { model.options.clickOverlayMode != .off },
                                                    set: { model.options.clickOverlayMode = $0 ? .ringsAndLabels : .off }))
                    .toggleStyle(.checkbox).controlSize(.mini).previewCaption().fixedSize()
                Text("Keys").previewCaption()
                Menu {
                    ForEach(KeystrokeCaptureMode.allCases) { mode in Button(mode.label) { model.options.keystrokeMode = mode } }
                } label: { Text(model.options.keystrokeMode.label).font(.system(size: 10)) }
                    .menuStyle(.borderlessButton).fixedSize().accessibilityLabel("Keystroke overlay")
                Spacer(minLength: 0)
                Button { onStart(model.options) } label: {
                    Label(model.options.outputFormat == .gif ? "Record GIF…" : "Start Recording…", systemImage: "record.circle")
                }
                .buttonStyle(LauncherChipButtonStyle(prominent: true)).disabled(!model.canRecordVideo)
                .help("Next: choose an area, window, or screen and confirm the options, then recording starts")
                .accessibilityIdentifier("launcherPreviewRecordingStart")
            }
            LauncherStatusLine(text: model.deliverable + " · you still choose the target first · " + model.privacyNote + " · these options are not saved")
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: height, alignment: .top)
        .clipped()
        .onAppear(perform: model.refreshPermissions)
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in model.refreshPermissions() }
    }

    private var countdown: some View {
        HStack(spacing: 3) {
            Button { model.options.countdownSeconds = max(0, model.options.countdownSeconds - 1) } label: { Image(systemName: "minus") }
                .buttonStyle(LauncherChipButtonStyle()).disabled(model.options.countdownSeconds <= 0).accessibilityLabel("Shorter countdown")
            Text(model.options.countdownSeconds == 0 ? "No countdown" : "\(model.options.countdownSeconds) s countdown")
                .font(.system(size: 10, weight: .medium)).monospacedDigit().frame(minWidth: 80)
            Button { model.options.countdownSeconds = min(10, model.options.countdownSeconds + 1) } label: { Image(systemName: "plus") }
                .buttonStyle(LauncherChipButtonStyle()).disabled(model.options.countdownSeconds >= 10).accessibilityLabel("Longer countdown")
        }
    }
}

// MARK: - Video to GIF

struct LauncherGIFPreviewView: View {
    @ObservedObject var model: LauncherGIFPreview
    let height: CGFloat?
    var onChoose: (GIFExportOptions) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            LauncherPreviewHeader(title: "Converts locally", subtitle: model.summary)
            HStack(spacing: 8) {
                Text("Frame rate").previewCaption()
                LauncherChoiceBar(selection: $model.options.framesPerSecond, choices: GIFExportOptions.frameRateChoices.map { ($0, "\($0) fps") }, label: "Frame rate")
                Text("Longest edge").previewCaption()
                LauncherChoiceBar(selection: $model.options.maxWidth, choices: GIFExportOptions.widthChoices.map { ($0, "\($0)") }, label: "Longest edge")
                Toggle("Loop", isOn: $model.options.loops).toggleStyle(.checkbox).controlSize(.mini).previewCaption().fixedSize()
                    .help("Play again from the start when the GIF ends; off plays it once")
                Spacer(minLength: 0)
                Button { onChoose(model.options) } label: { Label("Choose Movie…", systemImage: "film") }
                    .buttonStyle(LauncherChipButtonStyle(prominent: true))
                    .help("Pick a movie on this Mac, then trim and convert it with these options")
                    .accessibilityIdentifier("launcherPreviewGIFChoose")
            }
            LauncherStatusLine(text: "Trim after choosing · the movie is scaled down, never up · ↵ opens the converter with these options")
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: height, alignment: .top)
        .clipped()
    }
}

// MARK: - Settings and Home

struct LauncherAppStatusPreviewView: View {
    let command: LauncherCommand
    @ObservedObject var model: LauncherAppStatusPreview
    let height: CGFloat?
    var onOpenSettings: (SettingsCategory?) -> Void
    var onOpenHome: (HomeHandoff?) -> Void
    private let timer = Timer.publish(every: 1.5, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            header
            statusRow
            destinations
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: height, alignment: .top)
        .clipped()
        .onAppear(perform: model.refresh)
        .onReceive(timer) { _ in model.refresh() }
    }

    private var header: some View {
        let title = command == .settings ? "Bello Box Settings" : "Bello Box \(model.version)"
        let subtitle = command == .settings ? model.shortcutSummary : "Status, setup guide, and updates"
        return LauncherPreviewHeader(title: title, subtitle: subtitle)
    }

    private var statusRow: some View {
        HStack(spacing: 8) {
            StatusChip(text: model.accessibilityTrusted ? "Selection access on" : "Selection access needed", ok: model.accessibilityTrusted,
                       symbol: model.accessibilityTrusted ? "checkmark.shield" : "lock",
                       action: model.accessibilityTrusted ? nil : { model.openAccessibilitySettings() })
            StatusChip(text: model.screenRecordingTrusted ? "Screen Recording on" : "Screen Recording needed", ok: model.screenRecordingTrusted,
                       symbol: model.screenRecordingTrusted ? "checkmark.shield" : "lock",
                       action: model.screenRecordingTrusted ? nil : { model.openScreenRecordingSettings() })
            StatusChip(text: model.aiSummary, ok: true, symbol: "sparkles", action: nil)
        }
    }

    @ViewBuilder private var destinations: some View {
        if command == .settings {
            HStack(spacing: 6) {
                Text("Open").previewCaption()
                ForEach(SettingsCategory.allCases) { category in
                    Button(category.title) { onOpenSettings(category) }
                        .buttonStyle(LauncherChipButtonStyle()).help(category.explanation)
                        .accessibilityIdentifier("launcherPreviewSettings_\(category.id)")
                }
            }
        } else {
            HStack(spacing: 6) {
                Text("Open").previewCaption()
                ForEach(HomeCategory.allCases) { category in
                    Button(category.rawValue) { onOpenHome(.category(category)) }
                        .buttonStyle(LauncherChipButtonStyle()).help(category.subtitle)
                        .accessibilityIdentifier("launcherPreviewHome_\(category.rawValue)")
                }
                Spacer(minLength: 0)
                Button { onOpenHome(.setupGuide) } label: { Label("Setup Guide", systemImage: "questionmark.circle") }
                    .buttonStyle(LauncherChipButtonStyle())
                Button { onOpenHome(.checkForUpdates) } label: { Label("Check for Updates…", systemImage: "arrow.triangle.2.circlepath") }
                    .buttonStyle(LauncherChipButtonStyle())
                    .accessibilityIdentifier("launcherPreviewCheckForUpdates")
            }
        }
    }

    private struct StatusChip: View {
        let text: String
        let ok: Bool
        let symbol: String
        let action: (() -> Void)?
        var body: some View {
            HStack(spacing: 6) {
                Image(systemName: symbol).font(.system(size: 10)).foregroundStyle(ok ? BoxTheme.success : BoxTheme.warning)
                Text(text).font(.system(size: 10, weight: .medium)).lineLimit(1)
                Spacer(minLength: 0)
                if let action {
                    Button("Open…", action: action).buttonStyle(.plain).font(.system(size: 10, weight: .semibold)).foregroundStyle(BoxTheme.accent)
                        .help("Open System Settings")
                }
            }
            .padding(.horizontal, 8).frame(height: 26).frame(maxWidth: .infinity)
            .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(BoxTheme.border))
        }
    }

}
