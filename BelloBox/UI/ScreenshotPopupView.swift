import SwiftUI

struct LLMOCRConfirmation: Identifiable, Equatable {
    let id = UUID()
    var document: ScreenshotDocument
    var options: OCROptions
    var approvedConfig: AIConfig
    var documentRevision: Int
    var image: CGImage
    var byteCount: Int
    var provider: ProviderKind
    var model: String
    var includesLocalHint: Bool
    var dimensions: CGSize

    static func == (lhs: LLMOCRConfirmation, rhs: LLMOCRConfirmation) -> Bool {
        // CGImage is not Equatable; SwiftUI only needs confirmation identity here.
        lhs.id == rhs.id
    }
}

struct VisibleTextAnnotationFrame: Identifiable, Equatable {
    var id: UUID
    var frame: CGRect
}

@MainActor
final class ScreenshotPopupViewModel: ObservableObject {
    @Published var document: ScreenshotDocument
    @Published var activeTool: AnnotationTool = .select
    @Published var style: AnnotationStyle = .default
    @Published var ocrPanel = OCRPanelViewModel()
    @Published var errorMessage: String?
    @Published var llmConfirmation: LLMOCRConfirmation?
    @Published var editingTextAnnotationID: UUID?
    @Published var showDiscardCloseConfirmation = false

    private let settings: AppSettings
    private let macOCRService: OCRService
    private let makeLLMOCRService: (AIConfig) -> OCRService
    private let originalAnnotations: [ScreenshotAnnotation]
    private let originalCropRect: CGRect?
    private var undoStack: [ScreenshotDocument] = []
    private var redoStack: [ScreenshotDocument] = []
    private var ocrTask: Task<Void, Never>?
    private var ocrRunID: UUID?
    private var documentRevision = 0
    private var isClosed = false
    private var movingTextAnnotationID: UUID?

    var onClose: () -> Void = {}

    init(
        document: ScreenshotDocument,
        settings: AppSettings,
        macOCRService: OCRService = MacVisionOCRService(),
        llmOCRService: OCRService? = nil,
        llmOCRServiceFactory: ((AIConfig) -> OCRService)? = nil
    ) {
        self.document = document
        self.settings = settings
        self.macOCRService = macOCRService
        self.originalAnnotations = document.annotations
        self.originalCropRect = document.cropRect
        if let llmOCRServiceFactory {
            self.makeLLMOCRService = llmOCRServiceFactory
        } else if let llmOCRService {
            self.makeLLMOCRService = { _ in llmOCRService }
        } else {
            self.makeLLMOCRService = { LLMOCRService(config: $0) }
        }
        ocrPanel.showTextRegions = settings.ocrShowTextRegions
        wireOCRPanel()
        syncOCRPanel()
        if settings.screenshotAutoCopy {
            copyRenderedImage()
        }
    }

    deinit {
        ocrTask?.cancel()
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
    var hasImageEdits: Bool {
        document.annotations != originalAnnotations || document.cropRect != originalCropRect
    }

    var visibleTextAnnotationFrames: [VisibleTextAnnotationFrame] {
        document.annotations.compactMap { annotation in
            guard annotation.id != editingTextAnnotationID,
                  case let .text(_, origin, maxWidth) = annotation.kind
            else { return nil }
            let visibleOrigin = shiftDocumentPointToVisible(origin)
            return VisibleTextAnnotationFrame(
                id: annotation.id,
                frame: CGRect(
                    x: visibleOrigin.x,
                    y: visibleOrigin.y,
                    width: maxWidth,
                    height: max(34, annotation.style.fontSize + 16)
                )
            )
        }
    }

    @discardableResult
    func refreshBaseCapture(from replacement: ScreenshotDocument, expectedDocumentID: UUID) -> Bool {
        guard document.id == expectedDocumentID,
              document.annotations.isEmpty,
              document.cropRect == nil
        else { return false }

        document.baseImage = replacement.baseImage
        document.scale = replacement.scale
        document.source = replacement.source
        documentRevision += 1
        markOCRStale()
        return true
    }

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
            beginTextAnnotation(atVisiblePoint: visiblePoint)
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

    func beginTextAnnotation(atVisiblePoint point: CGPoint) {
        pushUndo()
        let annotation = ScreenshotAnnotation(
            kind: .text("", origin: shiftVisiblePointToDocument(point), maxWidth: 260),
            style: style
        )
        document.annotations.append(annotation)
        editingTextAnnotationID = annotation.id
        markOCRStale()
    }

    func textForEditingAnnotation() -> String {
        guard let id = editingTextAnnotationID,
              let annotation = document.annotations.first(where: { $0.id == id }),
              case let .text(text, _, _) = annotation.kind
        else { return "" }
        return text
    }

    func visibleTextFrameForEditingAnnotation() -> CGRect? {
        guard let id = editingTextAnnotationID,
              let annotation = document.annotations.first(where: { $0.id == id }),
              case let .text(_, origin, maxWidth) = annotation.kind
        else { return nil }
        let visibleOrigin = shiftDocumentPointToVisible(origin)
        return CGRect(x: visibleOrigin.x, y: visibleOrigin.y, width: maxWidth, height: max(34, style.fontSize + 16))
    }

    func updateEditingText(_ text: String) {
        guard let id = editingTextAnnotationID,
              let index = document.annotations.firstIndex(where: { $0.id == id }),
              case let .text(_, origin, maxWidth) = document.annotations[index].kind
        else { return }
        documentRevision += 1
        document.annotations[index].kind = .text(text, origin: origin, maxWidth: maxWidth)
        markOCRStale()
    }

    func moveEditingText(toVisibleOrigin origin: CGPoint) {
        guard let id = editingTextAnnotationID else { return }
        moveTextAnnotation(id: id, toVisibleOrigin: origin, recordUndoIfNeeded: false)
    }

    func endTextEditing() {
        guard let id = editingTextAnnotationID else { return }
        var removedEmptyAnnotation = false
        if let index = document.annotations.firstIndex(where: { $0.id == id }),
           case let .text(text, _, _) = document.annotations[index].kind,
           text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            documentRevision += 1
            document.annotations.remove(at: index)
            removedEmptyAnnotation = true
        }
        editingTextAnnotationID = nil
        if removedEmptyAnnotation {
            collapseUndoIfCurrentDocumentMatchesTop()
            redoStack.removeAll()
        }
        markOCRStale()
    }

    func cancelTextEditing() {
        guard let id = editingTextAnnotationID else { return }
        if let index = document.annotations.firstIndex(where: { $0.id == id }) {
            documentRevision += 1
            document.annotations.remove(at: index)
        }
        editingTextAnnotationID = nil
        collapseUndoIfCurrentDocumentMatchesTop()
        redoStack.removeAll()
        markOCRStale()
    }

    func beginMovingTextAnnotation(id: UUID) {
        guard movingTextAnnotationID != id,
              editingTextAnnotationID != id,
              document.annotations.contains(where: { $0.id == id })
        else { return }
        pushUndo()
        movingTextAnnotationID = id
    }

    func moveTextAnnotation(id: UUID, toVisibleOrigin origin: CGPoint) {
        moveTextAnnotation(id: id, toVisibleOrigin: origin, recordUndoIfNeeded: true)
    }

    func endMovingTextAnnotation(id: UUID) {
        if movingTextAnnotationID == id {
            movingTextAnnotationID = nil
        }
    }

    func undo() {
        guard let previous = undoStack.popLast() else { return }
        redoStack.append(document)
        document = previous
        documentRevision += 1
        syncOCRPanel()
    }

    func redo() {
        guard let next = redoStack.popLast() else { return }
        undoStack.append(document)
        document = next
        documentRevision += 1
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
        let snapshot = document
        let options = makeOCROptions(engine: .appleVision)
        startOCRTask(service: macOCRService, document: snapshot, options: options)
    }

    func requestLLMOCR() {
        llmConfirmation = nil
        ocrPanel.errorMessage = nil
        do {
            let config = settings.currentConfig
            guard config.kind != .codexCLI else {
                throw OCRError.unsupportedProvider("Codex app-server does not support image OCR yet. Use Mac OCR or an image-capable HTTP provider.")
            }
            let options = makeOCROptions(engine: .hybrid)
            let snapshot = document
            let revision = documentRevision
            let prepared = try OCRImagePreprocessor.prepare(document: snapshot, options: options, forExternalUpload: true)
            guard let data = prepared.encodedData else { throw OCRError.imageEncodingFailed }
            guard data.count <= LLMOCRService.maxUploadBytes else {
                throw OCRError.requestTooLarge(maxBytes: LLMOCRService.maxUploadBytes)
            }
            llmConfirmation = LLMOCRConfirmation(
                document: snapshot,
                options: options,
                approvedConfig: config,
                documentRevision: revision,
                image: prepared.image,
                byteCount: data.count,
                provider: config.kind,
                model: config.model,
                includesLocalHint: options.includeLocalOCRHintForLLM,
                dimensions: prepared.pixelSize
            )
        } catch {
            ocrPanel.errorMessage = error.localizedDescription
        }
    }

    func confirmLLMOCR() {
        guard let confirmation = llmConfirmation else { return }
        llmConfirmation = nil
        guard documentRevision == confirmation.documentRevision else {
            ocrPanel.errorMessage = OCRError.staleResult.localizedDescription
            return
        }
        startOCRTask(
            service: makeLLMOCRService(confirmation.approvedConfig),
            document: confirmation.document,
            options: confirmation.options,
            expectedRevision: confirmation.documentRevision
        )
    }

    func cancelLLMOCR() {
        llmConfirmation = nil
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
        guard !isClosed else { return }
        isClosed = true
        cancelOCRTask()
        onClose()
    }

    func requestClose() {
        guard !isClosed else { return }
        if editingTextAnnotationID != nil {
            cancelTextEditing()
            return
        }
        if hasImageEdits {
            showDiscardCloseConfirmation = true
        } else {
            close()
        }
    }

    func handleEscape() {
        requestClose()
    }

    func cancelDiscardClose() {
        showDiscardCloseConfirmation = false
    }

    func confirmDiscardAndClose() {
        showDiscardCloseConfirmation = false
        close()
    }

    func finish() {
        guard !isClosed else { return }
        copyRenderedImage()
        if errorMessage == nil {
            isClosed = true
            cancelOCRTask()
            onClose()
        }
    }

    private func wireOCRPanel() {
        ocrPanel.onRunMacOCR = { [weak self] in self?.runMacOCR() }
        ocrPanel.onRunLLMOCR = { [weak self] in self?.requestLLMOCR() }
        ocrPanel.onCopyPlainText = { [weak self] in self?.copyOCRText() }
        ocrPanel.onCopyMarkdown = { [weak self] in self?.copyOCRMarkdown() }
    }

    private func startOCRTask(
        service: OCRService,
        document snapshot: ScreenshotDocument,
        options: OCROptions,
        expectedRevision: Int? = nil
    ) {
        guard !ocrPanel.isRunning else { return }
        let runID = UUID()
        let expectedRevision = expectedRevision ?? documentRevision
        ocrRunID = runID
        ocrPanel.isRunning = true
        ocrPanel.errorMessage = nil
        ocrTask?.cancel()
        ocrTask = Task { [weak self, service, snapshot, options, runID, expectedRevision] in
            do {
                let result = try await service.recognize(document: snapshot, options: options)
                guard !Task.isCancelled else { return }
                await MainActor.run {
                    self?.completeOCRRun(runID, expectedRevision: expectedRevision, result: .success(result))
                }
            } catch is CancellationError {
                await MainActor.run {
                    self?.finishOCRRun(runID)
                }
            } catch {
                guard !Task.isCancelled else { return }
                await MainActor.run {
                    self?.completeOCRRun(runID, expectedRevision: expectedRevision, result: .failure(error))
                }
            }
        }
    }

    private func completeOCRRun(_ runID: UUID, expectedRevision: Int, result: Result<OCRResult, Error>) {
        guard ocrRunID == runID else { return }
        guard documentRevision == expectedRevision else {
            ocrPanel.errorMessage = OCRError.staleResult.localizedDescription
            finishOCRRun(runID)
            return
        }
        switch result {
        case let .success(ocrResult):
            document.ocrResults.append(ocrResult)
            document.activeOCRResultID = ocrResult.id
            syncOCRPanel()
        case let .failure(error):
            ocrPanel.errorMessage = error.localizedDescription
        }
        finishOCRRun(runID)
    }

    private func finishOCRRun(_ runID: UUID) {
        guard ocrRunID == runID else { return }
        ocrPanel.isRunning = false
        ocrRunID = nil
        ocrTask = nil
    }

    private func cancelOCRTask() {
        ocrRunID = nil
        ocrTask?.cancel()
        ocrTask = nil
        ocrPanel.isRunning = false
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
        documentRevision += 1
    }

    private func collapseUndoIfCurrentDocumentMatchesTop() {
        guard let previous = undoStack.last,
              previous.annotations == document.annotations,
              previous.cropRect == document.cropRect
        else { return }
        undoStack.removeLast()
    }

    private func moveTextAnnotation(id: UUID, toVisibleOrigin origin: CGPoint, recordUndoIfNeeded: Bool) {
        guard let index = document.annotations.firstIndex(where: { $0.id == id }),
              case let .text(text, _, maxWidth) = document.annotations[index].kind
        else { return }
        if recordUndoIfNeeded, movingTextAnnotationID != id {
            beginMovingTextAnnotation(id: id)
        }
        let visibleOrigin = clampedVisibleTextOrigin(
            origin,
            maxWidth: maxWidth,
            fontSize: document.annotations[index].style.fontSize
        )
        document.annotations[index].kind = .text(
            text,
            origin: shiftVisiblePointToDocument(visibleOrigin),
            maxWidth: maxWidth
        )
        documentRevision += 1
        markOCRStale()
    }

    private func clampedVisibleTextOrigin(_ origin: CGPoint, maxWidth: CGFloat, fontSize: CGFloat) -> CGPoint {
        let size = visibleImageSize
        let height = max(34, fontSize + 16)
        return CGPoint(
            x: min(max(origin.x, 0), max(0, size.width - maxWidth)),
            y: min(max(origin.y, 0), max(0, size.height - height))
        )
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

    private func shiftDocumentPointToVisible(_ point: CGPoint) -> CGPoint {
        guard let crop = document.cropRect else { return point }
        return CGPoint(x: point.x - crop.minX, y: point.y - crop.minY)
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
                    onClose: viewModel.requestClose
                )

                AnnotationToolbarView(viewModel: viewModel)

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
            .onExitCommand(perform: viewModel.handleEscape)
            .alert("Discard screenshot edits?", isPresented: $viewModel.showDiscardCloseConfirmation) {
                Button("Keep Editing", role: .cancel) { viewModel.cancelDiscardClose() }
                Button("Discard", role: .destructive) { viewModel.confirmDiscardAndClose() }
            } message: {
                Text("Your screenshot annotations and crop changes will be lost.")
            }

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
