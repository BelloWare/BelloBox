import SwiftUI

struct LLMOCRConfirmation: Identifiable, Equatable {
    let id = UUID()
    var image: CGImage
    var byteCount: Int
    var provider: ProviderKind
    var model: String
    var includesLocalHint: Bool
    var dimensions: CGSize

    static func == (lhs: LLMOCRConfirmation, rhs: LLMOCRConfirmation) -> Bool {
        lhs.id == rhs.id
    }
}

@MainActor
final class ScreenshotPopupViewModel: ObservableObject {
    @Published var document: ScreenshotDocument
    @Published var activeTool: AnnotationTool = .select
    @Published var style: AnnotationStyle = .default
    @Published var ocrPanel = OCRPanelViewModel()
    @Published var errorMessage: String?
    @Published var pendingTextLabel = "Label"
    @Published var llmConfirmation: LLMOCRConfirmation?

    private let settings: AppSettings
    private let macOCRService: MacVisionOCRService
    private let llmOCRService: LLMOCRService
    private var undoStack: [ScreenshotDocument] = []
    private var redoStack: [ScreenshotDocument] = []

    var onClose: () -> Void = {}

    init(
        document: ScreenshotDocument,
        settings: AppSettings,
        macOCRService: MacVisionOCRService = MacVisionOCRService(),
        llmOCRService: LLMOCRService? = nil
    ) {
        self.document = document
        self.settings = settings
        self.macOCRService = macOCRService
        self.llmOCRService = llmOCRService ?? LLMOCRService(settings: settings)
        ocrPanel.showTextRegions = settings.ocrShowTextRegions
        wireOCRPanel()
        if settings.screenshotAutoCopy {
            copyRenderedImage()
        }
    }

    var previewImage: CGImage {
        (try? AnnotationRenderer.render(document)) ?? document.baseImage
    }

    var visibleImageSize: CGSize {
        if let crop = document.cropRect {
            return CGSize(width: max(1, crop.width), height: max(1, crop.height))
        }
        return document.imageSize
    }

    var canUndo: Bool { !undoStack.isEmpty }
    var canRedo: Bool { !redoStack.isEmpty }

    func addVisibleAnnotation(_ kind: AnnotationKind) {
        let shifted = shiftVisibleKindToDocument(kind)
        var annotationStyle = style
        if case .highlight = shifted { annotationStyle = .highlight }
        if case .blur = shifted { annotationStyle = .redaction }
        addAnnotation(ScreenshotAnnotation(kind: shifted, style: annotationStyle))
    }

    func handleCanvasTap(visiblePoint: CGPoint) {
        switch activeTool {
        case .text:
            let text = pendingTextLabel.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !text.isEmpty else { return }
            addVisibleAnnotation(.text(text, origin: visiblePoint, maxWidth: 240))
        case .eraser:
            eraseAnnotation(atVisiblePoint: visiblePoint)
        default:
            break
        }
    }

    func eraseAnnotation(atVisiblePoint point: CGPoint) {
        let docPoint = shiftVisiblePointToDocument(point)
        guard let index = document.annotations.lastIndex(where: { $0.kind.bounds.insetBy(dx: -8, dy: -8).contains(docPoint) }) else { return }
        pushUndo()
        document.annotations.remove(at: index)
        markOCRStale()
    }

    func applyVisibleCrop(_ rect: CGRect) {
        let docRect = shiftVisibleRectToDocument(rect).intersection(CGRect(origin: .zero, size: document.imageSize)).integral
        guard docRect.width >= 4, docRect.height >= 4 else { return }
        pushUndo()
        document.cropRect = docRect
        markOCRStale()
    }

    func addAnnotation(_ annotation: ScreenshotAnnotation) {
        pushUndo()
        document.annotations.append(annotation)
        markOCRStale()
    }

    func undo() {
        guard let previous = undoStack.popLast() else { return }
        redoStack.append(document)
        document = previous
        syncOCRPanel()
    }

    func redo() {
        guard let next = redoStack.popLast() else { return }
        undoStack.append(document)
        document = next
        syncOCRPanel()
    }

    func copyRenderedImage() {
        do {
            let image = try AnnotationRenderer.render(document)
            try ImageExportService.copyToPasteboard(image)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func saveRenderedImage() {
        do {
            let image = try AnnotationRenderer.render(document)
            try ImageExportService.savePNG(image, suggestedName: "BelloBox-Screenshot-\(Int(Date().timeIntervalSince1970))")
            errorMessage = nil
        } catch ImageExportError.saveCancelled {
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func runMacOCR() {
        guard !ocrPanel.isRunning else { return }
        ocrPanel.isRunning = true
        ocrPanel.errorMessage = nil
        let snapshot = document
        let options = makeOCROptions(engine: .appleVision)
        Task {
            do {
                let result = try await macOCRService.recognize(document: snapshot, options: options)
                document.ocrResults.append(result)
                document.activeOCRResultID = result.id
                syncOCRPanel()
            } catch {
                ocrPanel.errorMessage = error.localizedDescription
            }
            ocrPanel.isRunning = false
        }
    }

    func requestLLMOCR() {
        do {
            let options = makeOCROptions(engine: .hybrid)
            let prepared = try OCRImagePreprocessor.prepare(document: document, options: options, forExternalUpload: true)
            guard let data = prepared.encodedData else { throw OCRError.imageEncodingFailed }
            guard data.count <= LLMOCRService.maxUploadBytes else {
                throw OCRError.requestTooLarge(maxBytes: LLMOCRService.maxUploadBytes)
            }
            llmConfirmation = LLMOCRConfirmation(
                image: prepared.image,
                byteCount: data.count,
                provider: settings.currentConfig.kind,
                model: settings.currentConfig.model,
                includesLocalHint: options.includeLocalOCRHintForLLM,
                dimensions: prepared.pixelSize
            )
        } catch {
            ocrPanel.errorMessage = error.localizedDescription
        }
    }

    func confirmLLMOCR() {
        llmConfirmation = nil
        runLLMOCR()
    }

    func cancelLLMOCR() {
        llmConfirmation = nil
    }

    func runLLMOCR() {
        guard !ocrPanel.isRunning else { return }
        ocrPanel.isRunning = true
        ocrPanel.errorMessage = nil
        let snapshot = document
        let options = makeOCROptions(engine: .hybrid)
        Task {
            do {
                let result = try await llmOCRService.recognize(document: snapshot, options: options)
                document.ocrResults.append(result)
                document.activeOCRResultID = result.id
                syncOCRPanel()
            } catch {
                ocrPanel.errorMessage = error.localizedDescription
            }
            ocrPanel.isRunning = false
        }
    }

    func copyOCRText() {
        guard let result = document.activeOCRResult else { return }
        do { try OCRResultFormatter.copyPlainText(result); ocrPanel.errorMessage = nil }
        catch { ocrPanel.errorMessage = error.localizedDescription }
    }

    func copyOCRMarkdown() {
        guard let result = document.activeOCRResult else { return }
        do { try OCRResultFormatter.copyMarkdown(result); ocrPanel.errorMessage = nil }
        catch { ocrPanel.errorMessage = error.localizedDescription }
    }

    func close() {
        onClose()
    }

    func finish() {
        copyRenderedImage()
        if errorMessage == nil {
            onClose()
        }
    }

    private func wireOCRPanel() {
        ocrPanel.onRunMacOCR = { [weak self] in self?.runMacOCR() }
        ocrPanel.onRunLLMOCR = { [weak self] in self?.requestLLMOCR() }
        ocrPanel.onCopyPlainText = { [weak self] in self?.copyOCRText() }
        ocrPanel.onCopyMarkdown = { [weak self] in self?.copyOCRMarkdown() }
    }

    private func makeOCROptions(engine: OCRRequestedEngine) -> OCROptions {
        OCROptions(
            engine: engine,
            recognitionLevel: settings.ocrRecognitionLevel,
            languageHints: settings.ocrLanguageHints,
            usesLanguageCorrection: settings.ocrUseLanguageCorrection,
            customWords: [],
            target: .visibleAfterRedactions(crop: document.cropRect.map(CGRectCodable.init)),
            outputFormat: .plainTextAndMarkdown,
            maxUploadLongEdge: settings.llmOCRMaxUploadLongEdge,
            includeLocalOCRHintForLLM: settings.llmOCRIncludeLocalOCRHint
        )
    }

    private func pushUndo() {
        undoStack.append(document)
        redoStack.removeAll()
    }

    private func syncOCRPanel() {
        ocrPanel.result = document.activeOCRResult
        ocrPanel.showTextRegions = settings.ocrShowTextRegions || ocrPanel.showTextRegions
    }

    private func markOCRStale() {
        guard !document.ocrResults.isEmpty else { return }
        document.ocrResults = document.ocrResults.map { result in
            var copy = result
            if !copy.warnings.contains("OCR may be out of date after crop or redaction changed.") {
                copy.warnings.append("OCR may be out of date after crop or redaction changed.")
            }
            return copy
        }
        syncOCRPanel()
    }

    private func shiftVisibleKindToDocument(_ kind: AnnotationKind) -> AnnotationKind {
        switch kind {
        case let .freehand(points):
            return .freehand(points: points.map(shiftVisiblePointToDocument))
        case let .arrow(start, end):
            return .arrow(start: shiftVisiblePointToDocument(start), end: shiftVisiblePointToDocument(end))
        case let .rectangle(rect):
            return .rectangle(shiftVisibleRectToDocument(rect))
        case let .highlight(rect):
            return .highlight(shiftVisibleRectToDocument(rect))
        case let .text(text, origin, maxWidth):
            return .text(text, origin: shiftVisiblePointToDocument(origin), maxWidth: maxWidth)
        case let .blur(rect):
            return .blur(shiftVisibleRectToDocument(rect))
        }
    }

    private func shiftVisiblePointToDocument(_ point: CGPoint) -> CGPoint {
        guard let crop = document.cropRect else { return point }
        return CGPoint(x: point.x + crop.minX, y: point.y + crop.minY)
    }

    private func shiftVisibleRectToDocument(_ rect: CGRect) -> CGRect {
        guard let crop = document.cropRect else { return rect.standardized }
        return rect.offsetBy(dx: crop.minX, dy: crop.minY).standardized
    }
}

struct ScreenshotPopupView: View {
    static let preferredSize = CGSize(width: 1040, height: 760)

    @ObservedObject var viewModel: ScreenshotPopupViewModel
    var onMinimize: () -> Void

    var body: some View {
        ZStack {
            VStack(alignment: .leading, spacing: 12) {
                PopupHeader(
                    icon: "camera.viewfinder",
                    title: "Screenshot",
                    subtitle: sourceSummary,
                    onMinimize: onMinimize,
                    onClose: viewModel.close
                )

                toolbar

                HStack(alignment: .top, spacing: 12) {
                    AnnotationCanvasView(viewModel: viewModel)
                        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(.primary.opacity(0.08), lineWidth: 1))
                        .frame(maxWidth: .infinity, maxHeight: .infinity)

                    OCRPanelView(viewModel: viewModel.ocrPanel)
                        .frame(width: 285)
                        .toolPanel()
                }

                footer
            }
            .padding(16)
            .frame(width: Self.preferredSize.width, height: Self.preferredSize.height)
            .popupCard()
            .onExitCommand(perform: viewModel.close)

            if let confirmation = viewModel.llmConfirmation {
                LLMOCRConfirmationView(
                    confirmation: confirmation,
                    onConfirm: viewModel.confirmLLMOCR,
                    onCancel: viewModel.cancelLLMOCR
                )
                .padding(32)
            }
        }
    }

    private var toolbar: some View {
        HStack(spacing: 6) {
            ForEach(AnnotationTool.allCases) { tool in
                Button {
                    viewModel.activeTool = tool
                } label: {
                    Image(systemName: tool.symbol)
                        .frame(width: 28, height: 26)
                }
                .buttonStyle(.plain)
                .background(RoundedRectangle(cornerRadius: 7).fill(viewModel.activeTool == tool ? BoxTheme.accentSoft : .clear))
                .help(tool.label)
            }

            Divider().frame(height: 24)

            ColorPicker("Color", selection: Binding(
                get: { Color(nsColor: viewModel.style.strokeColor.nsColor) },
                set: { color in
                    if let cgColor = color.cgColor, let nsColor = NSColor(cgColor: cgColor) {
                        viewModel.style.strokeColor = CodableColor(nsColor)
                    }
                }
            ))
            .labelsHidden()
            .frame(width: 34)

            Slider(value: Binding(
                get: { Double(viewModel.style.lineWidth) },
                set: { viewModel.style.lineWidth = CGFloat($0) }
            ), in: 1...12, step: 1)
            .frame(width: 90)

            TextField("Text", text: $viewModel.pendingTextLabel)
                .textFieldStyle(.roundedBorder)
                .frame(width: 120)

            Spacer()

            Button { viewModel.undo() } label: { Image(systemName: "arrow.uturn.backward") }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(!viewModel.canUndo)
                .keyboardShortcut("z", modifiers: .command)
            Button { viewModel.redo() } label: { Image(systemName: "arrow.uturn.forward") }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(!viewModel.canRedo)
                .keyboardShortcut("Z", modifiers: [.command, .shift])
            Button { viewModel.finish() } label: { Image(systemName: "checkmark") }
                .buttonStyle(PrimaryButtonStyle())
                .help("Copy image and finish")
        }
    }

    private var footer: some View {
        HStack(spacing: 10) {
            if let error = viewModel.errorMessage {
                Label(error, systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(.orange)
                    .lineLimit(2)
            }
            Spacer()
            Button("Copy Image") { viewModel.copyRenderedImage() }
                .buttonStyle(PrimaryButtonStyle())
            Button("Save PNG") { viewModel.saveRenderedImage() }
                .buttonStyle(SecondaryButtonStyle())
            Button("Done") { viewModel.finish() }
                .buttonStyle(SecondaryButtonStyle())
        }
    }

    private var sourceSummary: String {
        switch viewModel.document.source {
        case .area:
            return "Area capture"
        case let .window(title, owner, _):
            return [owner, title].compactMap { $0 }.joined(separator: " · ")
        case .display:
            return "Screen capture"
        case let .scrolling(_, frameCount):
            return "Scrolling capture · \(frameCount) frames"
        case .importedClipboard:
            return "Clipboard image"
        }
    }
}

private struct LLMOCRConfirmationView: View {
    let confirmation: LLMOCRConfirmation
    var onConfirm: () -> Void
    var onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            PopupHeader(
                icon: "sparkles",
                title: "Confirm LLM OCR Upload",
                subtitle: "\(confirmation.provider.displayName) · \(confirmation.model)",
                onClose: onCancel
            )

            Image(nsImage: NSImage(cgImage: confirmation.image, size: confirmation.dimensions))
                .resizable()
                .scaledToFit()
                .frame(height: 190)
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(.primary.opacity(0.08), lineWidth: 1))

            VStack(alignment: .leading, spacing: 4) {
                Text("Image: \(Int(confirmation.dimensions.width)) × \(Int(confirmation.dimensions.height)) px")
                Text("Upload size: \(ByteCountFormatter.string(fromByteCount: Int64(confirmation.byteCount), countStyle: .file))")
                Text(confirmation.includesLocalHint ? "Mac OCR text will be included as a hint." : "No Mac OCR hint will be included.")
            }
            .font(.caption)
            .foregroundStyle(.secondary)

            Text("Only the redaction-aware OCR image shown above will be sent. Decorative annotations are excluded.")
                .font(.caption)
                .fixedSize(horizontal: false, vertical: true)

            HStack {
                Spacer()
                Button("Cancel") { onCancel() }
                    .buttonStyle(SecondaryButtonStyle())
                Button("Upload and Improve") { onConfirm() }
                    .buttonStyle(PrimaryButtonStyle())
            }
        }
        .padding(18)
        .frame(width: 460)
        .popupCard()
        .shadow(radius: 20)
    }
}
