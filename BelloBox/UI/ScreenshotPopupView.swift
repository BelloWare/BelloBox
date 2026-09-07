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

private struct BasePreviewKey: Equatable {
    var imageWidth: Int
    var imageHeight: Int
    var cropRect: CGRect?
    var revision: Int
}

@MainActor
final class ScreenshotPopupViewModel: ObservableObject {
    @Published var document: ScreenshotDocument {
        didSet { statusMessage = nil }
    }
    @Published var activeTool: AnnotationTool = .select
    @Published var style: AnnotationStyle = .default {
        didSet { updateEditingTextStyle() }
    }
    /// Style for new masks: an opaque fill plus a pattern. Never translucent.
    /// A `@Published` observer re-enters on every write, so it only writes back
    /// when normalization actually changes the value.
    @Published var maskStyle: AnnotationStyle = .redaction {
        didSet {
            let normalized = AnnotationStyle.mask(fill: maskStyle.maskFill, pattern: maskStyle.maskPattern)
            if maskStyle != normalized { maskStyle = normalized }
        }
    }
    /// Eraser brush diameter in image pixels, clamped to `eraserWidthRange`.
    @Published var eraserWidth: CGFloat = 24 {
        didSet {
            let clamped = Self.clampedEraserWidth(eraserWidth)
            if eraserWidth != clamped { eraserWidth = clamped }
        }
    }
    static let eraserWidthRange: ClosedRange<CGFloat> = 6...96
    static func clampedEraserWidth(_ width: CGFloat) -> CGFloat {
        guard width.isFinite else { return 24 }
        return min(max(width, eraserWidthRange.lowerBound), eraserWidthRange.upperBound)
    }
    /// How the popup editor shows the image. Tall scrolling captures open fitted
    /// to the width so they read like the page they came from.
    @Published var zoom: CanvasZoom
    /// View points per image pixel as actually laid out by the canvas host, so
    /// zoom steps start from what is on screen and the Fit label stays current.
    @Published private(set) var displayedScale: CGFloat = 1
    @Published private(set) var showsCaptureNotes: Bool
    @Published var showsAllCaptureNotes = false
    @Published var ocrPanel = OCRPanelViewModel()
    @Published var errorMessage: String?
    @Published private(set) var statusMessage: String?
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
    private var basePreviewRevision = 0
    private var cachedBasePreviewKey: BasePreviewKey?
    private var cachedBasePreview: CGImage?
    private var isClosed = false
    private var movingTextAnnotationID: UUID?
    private var movingTextUndoPushed = false
    private let allowsSelectionAdjustment: Bool
    private var isAdjustingSelection = false
    private var selectionAdjustmentUndoPushed = false
    private var isErasing = false
    private var eraserUndoPushed = false
    private var eraserLastPoint: CGPoint?
    /// Index of the erasure stroke the current gesture is extending, per annotation.
    private var eraserStrokeIndices: [UUID: Int] = [:]

    var onClose: () -> Void = {}

    init(
        document: ScreenshotDocument,
        settings: AppSettings,
        macOCRService: OCRService = MacVisionOCRService(),
        llmOCRService: OCRService? = nil,
        llmOCRServiceFactory: ((AIConfig) -> OCRService)? = nil,
        allowsSelectionAdjustment: Bool = false
    ) {
        self.document = document
        self.settings = settings
        self.macOCRService = macOCRService
        self.allowsSelectionAdjustment = allowsSelectionAdjustment
        self.originalAnnotations = document.annotations
        self.originalCropRect = document.cropRect
        self.zoom = Self.initialZoom(for: document)
        self.showsCaptureNotes = !document.captureNotes.isEmpty
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

    func basePreviewImage() -> CGImage {
        let key = BasePreviewKey(
            imageWidth: document.baseImage.width,
            imageHeight: document.baseImage.height,
            cropRect: document.cropRect,
            revision: basePreviewRevision
        )
        if cachedBasePreviewKey == key, let cachedBasePreview {
            return cachedBasePreview
        }
        let image: CGImage
        if let crop = document.cropRect?.integral,
           crop.width > 0,
           crop.height > 0,
           let cropped = document.baseImage.cropping(to: crop.intersection(CGRect(origin: .zero, size: document.imageSize)).integral) {
            image = cropped
        } else {
            image = document.baseImage
        }
        cachedBasePreviewKey = key
        cachedBasePreview = image
        return image
    }

    var visibleImageSize: CGSize {
        if let crop = document.cropRect {
            return CGSize(width: max(1, crop.width), height: max(1, crop.height))
        }
        return document.imageSize
    }

    var canResetCrop: Bool { document.cropRect != originalCropRect }

    func resetCrop() {
        guard canResetCrop else { return }
        endTextEditing()
        pushUndo()
        replaceCropRect(with: originalCropRect)
        markOCRStale()
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

    var visibleAnnotations: [ScreenshotAnnotation] {
        let crop = document.cropRect
        return document.annotations.compactMap { annotation in
            guard annotation.id != editingTextAnnotationID else { return nil }
            guard let crop else { return annotation }
            return annotation.offset(dx: -crop.minX, dy: -crop.minY)
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
        basePreviewRevision += 1
        markOCRStale()
        return true
    }

    func addVisibleAnnotation(_ kind: AnnotationKind) {
        let shifted = shiftVisibleKindToDocument(kind)
        var annotationStyle = style
        if case .highlight = shifted { annotationStyle = .highlight }
        if case .blur = shifted { annotationStyle = maskStyle }
        addAnnotation(ScreenshotAnnotation(kind: shifted, style: annotationStyle))
    }

    func handleCanvasTap(visiblePoint: CGPoint) {
        switch activeTool {
        case .text:
            beginTextAnnotation(atVisiblePoint: visiblePoint)
        case .eraser:
            beginErasing()
            erase(toVisiblePoint: visiblePoint)
            endErasing()
        default:
            break
        }
    }

    // MARK: - Eraser

    /// Starts one eraser gesture. The first change records a single undo step, so
    /// the whole drag reverts at once; nothing is recorded until something is touched.
    func beginErasing() {
        endTextEditing()
        isErasing = true
        eraserUndoPushed = false
        eraserLastPoint = nil
        eraserStrokeIndices = [:]
    }

    /// Extends the gesture to `point` (visible-image pixels). Every annotation under the
    /// brush pass gets an erasure stroke; the screenshot pixels are never changed, and
    /// annotations drawn later over the same area are unaffected.
    func erase(toVisiblePoint point: CGPoint) {
        guard isErasing else { beginErasing(); erase(toVisiblePoint: point); endErasing(); return }
        let end = shiftVisiblePointToDocument(point)
        let start = eraserLastPoint ?? end
        eraserLastPoint = end
        let radius = eraserWidth / 2
        var touched = false
        for index in document.annotations.indices {
            let annotation = document.annotations[index]
            guard annotation.id != editingTextAnnotationID,
                  AnnotationGeometry.brush(from: start, to: end, radius: radius, touches: annotation)
            else { continue }
            if !eraserUndoPushed {
                pushUndo()
                eraserUndoPushed = true
            } else {
                documentRevision += 1
            }
            touched = true
            var erasures = document.annotations[index].erasures
            if let strokeIndex = eraserStrokeIndices[annotation.id], strokeIndex < erasures.count,
               erasures[strokeIndex].points.last == start, erasures[strokeIndex].width == eraserWidth {
                erasures[strokeIndex].points.append(end)
            } else {
                erasures.append(EraserStroke(points: start == end ? [end] : [start, end], width: eraserWidth))
                eraserStrokeIndices[annotation.id] = erasures.count - 1
            }
            document.annotations[index].erasures = erasures
        }
        if touched { markOCRStale() }
    }

    /// Ends the gesture. Annotations keep their erasure history; nothing is ever
    /// removed on the eraser's behalf, so paint outside the brush always survives.
    func endErasing() {
        guard isErasing else { return }
        isErasing = false
        eraserLastPoint = nil
        eraserStrokeIndices = [:]
    }

    /// Whole-object removal, used by tests and by Select-mode deletion.
    func removeAnnotation(id: UUID) {
        guard let index = document.annotations.firstIndex(where: { $0.id == id }) else { return }
        if editingTextAnnotationID == id { editingTextAnnotationID = nil }
        pushUndo()
        document.annotations.remove(at: index)
        markOCRStale()
    }

    // MARK: - Zoom and capture notes

    static func initialZoom(for document: ScreenshotDocument) -> CanvasZoom {
        let size = document.cropRect?.size ?? document.imageSize
        let tall = size.height > size.width * 1.6
        return document.source.scrollingFrameCount > 0 && tall ? .fitWidth : .fit
    }

    /// Called by the canvas host after it laid out, never during a SwiftUI update.
    func reportDisplayedScale(_ scale: CGFloat) {
        guard scale.isFinite, scale > 0, abs(scale - displayedScale) > 0.0005 else { return }
        displayedScale = scale
    }

    func zoomIn() { zoom = CanvasZoom.zoomedIn(from: displayedScale) }
    func zoomOut() { zoom = CanvasZoom.zoomedOut(from: displayedScale) }
    func zoomToFit() { zoom = .fit }
    func zoomToFitWidth() { zoom = .fitWidth }
    func zoomToActualSize() { zoom = .scale(1) }

    var zoomLabel: String {
        switch zoom {
        case .scale: return zoom.label
        case .fit, .fitWidth: return "\(Int((displayedScale * 100).rounded()))% · \(zoom.label)"
        }
    }

    func dismissCaptureNotes() { showsCaptureNotes = false }

    func applyVisibleCrop(_ rect: CGRect) {
        let docRect = shiftVisibleRectToDocument(rect).intersection(CGRect(origin: .zero, size: document.imageSize)).integral
        guard docRect.width >= 4, docRect.height >= 4 else { return }
        pushUndo()
        replaceCropRect(with: docRect)
        markOCRStale()
    }

    /// Whether the base image covers the whole display, so the selection can be resized
    /// or moved within it after capture.
    var supportsSelectionAdjustment: Bool { allowsSelectionAdjustment }

    /// The current selection in base-image pixels.
    var selectionCropRect: CGRect {
        document.cropRect ?? CGRect(origin: .zero, size: document.imageSize)
    }

    /// The smallest selection the handles may produce, in base-image pixels.
    var minimumSelectionPixelSize: CGSize {
        let edge = max(4, (RegionCaptureGeometry.minimumAreaSize * max(document.scale, 1)).rounded())
        return CGSize(width: edge, height: edge)
    }

    /// Starts an interactive resize or move. Only the first change inside a session
    /// records an undo step, so a whole drag reverts with a single undo.
    func beginSelectionAdjustment() {
        guard allowsSelectionAdjustment, !isAdjustingSelection else { return }
        isAdjustingSelection = true
        selectionAdjustmentUndoPushed = false
    }

    /// Sets the selection in base-image pixels, clamped to the image and the minimum
    /// size. Annotations keep their document coordinates, so they stay attached to the
    /// pixels they were drawn on.
    func setSelectionCropRect(_ rect: CGRect) {
        guard allowsSelectionAdjustment else { return }
        let imageBounds = CGRect(origin: .zero, size: document.imageSize)
        let clamped = SelectionResizeGeometry
            .clamped(rect, in: imageBounds, minimumSize: minimumSelectionPixelSize)
            .integral
            .intersection(imageBounds)
        guard !clamped.isNull, clamped.width >= 1, clamped.height >= 1 else { return }
        let newCrop: CGRect? = clamped == imageBounds ? nil : clamped
        guard newCrop != document.cropRect else { return }
        if isAdjustingSelection {
            if selectionAdjustmentUndoPushed {
                documentRevision += 1
            } else {
                pushUndo()
                selectionAdjustmentUndoPushed = true
            }
        } else {
            pushUndo()
        }
        replaceCropRect(with: newCrop)
        markOCRStale()
    }

    func endSelectionAdjustment() {
        guard isAdjustingSelection else { return }
        isAdjustingSelection = false
        selectionAdjustmentUndoPushed = false
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
              case let .text(currentText, origin, maxWidth) = document.annotations[index].kind
        else { return }
        guard currentText != text else { return }
        documentRevision += 1
        document.annotations[index].kind = .text(text, origin: origin, maxWidth: maxWidth)
        markOCRStale()
    }

    private func updateEditingTextStyle() {
        guard let id = editingTextAnnotationID,
              let index = document.annotations.firstIndex(where: { $0.id == id }),
              document.annotations[index].style != style else { return }
        document.annotations[index].style = style
        documentRevision += 1
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
        movingTextAnnotationID = id
        movingTextUndoPushed = false
    }

    func moveTextAnnotation(id: UUID, toVisibleOrigin origin: CGPoint) {
        moveTextAnnotation(id: id, toVisibleOrigin: origin, recordUndoIfNeeded: true)
    }

    func endMovingTextAnnotation(id: UUID) {
        if movingTextAnnotationID == id {
            movingTextAnnotationID = nil
            movingTextUndoPushed = false
        }
    }

    func undo() {
        guard let previous = undoStack.popLast() else { return }
        redoStack.append(document)
        document = previous
        documentRevision += 1
        basePreviewRevision += 1
        syncOCRPanel()
    }

    func redo() {
        guard let next = redoStack.popLast() else { return }
        undoStack.append(document)
        document = next
        documentRevision += 1
        basePreviewRevision += 1
        syncOCRPanel()
    }

    func copyRenderedImage() {
        statusMessage = nil
        do {
            let image = try AnnotationRenderer.render(document)
            try ImageExportService.copyToPasteboard(image)
            errorMessage = nil
            statusMessage = "Copied image."
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func saveRenderedImage() {
        statusMessage = nil
        do {
            let image = try AnnotationRenderer.render(document)
            try ImageExportService.savePNG(image, suggestedName: "BelloBox-Screenshot-\(Int(Date().timeIntervalSince1970))")
            errorMessage = nil
            statusMessage = "Saved PNG."
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
        ocrPanel.statusMessage = nil
        do {
            try OCRResultFormatter.copyPlainText(result)
            ocrPanel.errorMessage = nil
            ocrPanel.statusMessage = "Copied text."
        }
        catch { ocrPanel.errorMessage = error.localizedDescription }
    }

    func copyOCRMarkdown() {
        guard let result = document.activeOCRResult else { return }
        ocrPanel.statusMessage = nil
        do {
            try OCRResultFormatter.copyMarkdown(result)
            ocrPanel.errorMessage = nil
            ocrPanel.statusMessage = "Copied Markdown."
        }
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
        ocrPanel.onCancel = { [weak self] in self?.cancelOCR() }
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
        ocrPanel.statusMessage = nil
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

    func cancelOCR() {
        guard ocrPanel.isRunning else { return }
        cancelOCRTask()
        ocrPanel.errorMessage = nil
        ocrPanel.statusMessage = "Reading cancelled."
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
              case let .text(_, currentOrigin, maxWidth) = document.annotations[index].kind
        else { return }
        let visibleOrigin = clampedVisibleTextOrigin(
            origin,
            maxWidth: maxWidth,
            fontSize: document.annotations[index].style.fontSize
        )
        let documentOrigin = shiftVisiblePointToDocument(visibleOrigin)
        guard currentOrigin != documentOrigin else { return }
        var didAdvanceRevision = false
        if recordUndoIfNeeded {
            if movingTextAnnotationID != id {
                movingTextAnnotationID = id
                movingTextUndoPushed = false
            }
            if !movingTextUndoPushed {
                pushUndo()
                movingTextUndoPushed = true
                didAdvanceRevision = true
            }
        }
        if !didAdvanceRevision {
            documentRevision += 1
        }
        document.annotations[index] = document.annotations[index].offset(
            dx: documentOrigin.x - currentOrigin.x,
            dy: documentOrigin.y - currentOrigin.y
        )
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

    /// Changes the crop and keeps OCR region boxes, which are stored in the visible
    /// image's coordinates, over the same pixels they were recognised on.
    private func replaceCropRect(with newCrop: CGRect?) {
        let oldOrigin = document.cropRect?.origin ?? .zero
        let newOrigin = newCrop?.origin ?? .zero
        let delta = CGPoint(x: oldOrigin.x - newOrigin.x, y: oldOrigin.y - newOrigin.y)
        document.cropRect = newCrop
        basePreviewRevision += 1
        guard delta != .zero, !document.ocrResults.isEmpty else { return }
        document.ocrResults = document.ocrResults.map { result in
            var copy = result
            copy.regions = Self.shiftRegions(result.regions, by: delta)
            return copy
        }
    }

    private static func shiftRegions(_ regions: [OCRTextRegion], by delta: CGPoint) -> [OCRTextRegion] {
        regions.map { region in
            var copy = region
            if let box = region.boundingBox?.rect {
                copy.boundingBox = CGRectCodable(box.offsetBy(dx: delta.x, dy: delta.y))
            }
            copy.children = shiftRegions(region.children, by: delta)
            return copy
        }
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
    @State private var showsOCR = false

    var body: some View {
        ZStack {
            VStack(alignment: .leading, spacing: 12) {
                PopupHeader(
                    icon: "camera.viewfinder",
                    title: "Screenshot",
                    subtitle: "\(sourceSummary) · \(Int(viewModel.visibleImageSize.width)) × \(Int(viewModel.visibleImageSize.height)) px",
                    onMinimize: onMinimize,
                    onClose: viewModel.requestClose
                )

                AnnotationToolbarView(viewModel: viewModel)

                if viewModel.showsCaptureNotes, !viewModel.document.captureNotes.isEmpty {
                    captureNotes
                }

                HStack(alignment: .top, spacing: 12) {
                    ZoomableAnnotationCanvas(viewModel: viewModel)
                        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(.primary.opacity(0.08), lineWidth: 1))
                        .frame(maxWidth: .infinity, maxHeight: .infinity)

                    if showsOCR {
                        OCRPanelView(viewModel: viewModel.ocrPanel)
                            .frame(width: 285)
                            .toolPanel()
                    }
                }

                footer
            }
            .padding(16)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
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

    /// Notes from the capture itself (a scrolling frame without overlap, frames
    /// left out), where the user reviews the result rather than in the text reader.
    /// Every note stays reachable: two are shown inline, the rest behind "Show all",
    /// which lists them in a bounded scrolling area. The engine puts notes about an
    /// incomplete capture first, so they are always among the visible two.
    private var captureNotes: some View {
        CaptureNotesBanner(
            title: viewModel.document.source.scrollingFrameCount > 0
                ? "Scrolling capture · \(viewModel.document.source.scrollingFrameCount) frames"
                : "About this capture",
            notes: viewModel.document.captureNotes,
            showsAll: $viewModel.showsAllCaptureNotes,
            onDismiss: viewModel.dismissCaptureNotes
        )
    }

    private var zoomControls: some View {
        HStack(spacing: 4) {
            Button { viewModel.zoomOut() } label: { Image(systemName: "minus.magnifyingglass") }
                .buttonStyle(SecondaryButtonStyle())
                .keyboardShortcut("-", modifiers: .command)
                .accessibilityLabel("Zoom out")
                .help("Zoom out (⌘−)")
            Menu {
                Button("Fit", action: viewModel.zoomToFit).keyboardShortcut("9", modifiers: .command)
                Button("Fit Width", action: viewModel.zoomToFitWidth)
                Button("Actual Size", action: viewModel.zoomToActualSize).keyboardShortcut("0", modifiers: .command)
                Divider()
                ForEach(CanvasZoom.steps, id: \.self) { step in
                    Button("\(Int(step * 100))%") { viewModel.zoom = .scale(step) }
                }
            } label: {
                Text(viewModel.zoomLabel).font(.caption.monospacedDigit()).frame(minWidth: 88)
            }
            .menuStyle(.borderlessButton).fixedSize()
            .accessibilityLabel("Zoom")
            .accessibilityValue(viewModel.zoomLabel)
            .help("Fit (⌘9), Actual Size (⌘0), or a magnification. Scroll to pan.")
            Button { viewModel.zoomIn() } label: { Image(systemName: "plus.magnifyingglass") }
                .buttonStyle(SecondaryButtonStyle())
                .keyboardShortcut("=", modifiers: .command)
                .accessibilityLabel("Zoom in")
                .help("Zoom in (⌘+)")
        }
    }

    private var footer: some View {
        HStack(spacing: 10) {
            Button {
                showsOCR.toggle()
            } label: {
                Label(showsOCR ? "Hide Text Reader" : "Show Text Reader", systemImage: "sidebar.right")
            }
            .buttonStyle(SecondaryButtonStyle())
            .keyboardShortcut("o", modifiers: [.command, .option])
            .help("Show or hide the text reader (⌥⌘O)")
            zoomControls
            if let message = viewModel.errorMessage ?? viewModel.statusMessage {
                Label(message, systemImage: viewModel.errorMessage == nil ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(viewModel.errorMessage == nil ? Color.secondary : BoxTheme.warning)
                    .lineLimit(2)
            }
            Spacer()
            Button("Copy Image") { viewModel.copyRenderedImage() }
                .buttonStyle(PrimaryButtonStyle())
                .keyboardShortcut("c", modifiers: [.command, .shift])
                .help("Copy image (⇧⌘C)")
            Button("Save PNG…") { viewModel.saveRenderedImage() }
                .buttonStyle(SecondaryButtonStyle())
                .keyboardShortcut("s", modifiers: .command)
            Button("Copy & Finish") { viewModel.finish() }
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
            return "Scrolling capture · \(frameCount) frame\(frameCount == 1 ? "" : "s")"
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

/// The capture notes card. `CaptureNotesBanner.inlineCount` notes are always
/// visible; "Show all" reveals the rest in a scrolling list capped in height.
struct CaptureNotesBanner: View {
    static let inlineCount = 2
    static let expandedMaxHeight: CGFloat = 132

    var title: String
    var notes: [String]
    @Binding var showsAll: Bool
    var onDismiss: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "info.circle.fill").foregroundStyle(BoxTheme.warning).padding(.top, 1)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    Text(title).font(.caption.weight(.semibold))
                    if notes.count > Self.inlineCount {
                        Text("\(notes.count) notes").font(.caption2).foregroundStyle(.secondary)
                    }
                }
                if showsAll {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 5) {
                            ForEach(Array(notes.enumerated()), id: \.offset) { _, note in
                                Text(note).font(.caption).foregroundStyle(.secondary)
                                    .fixedSize(horizontal: false, vertical: true)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }
                    .frame(maxHeight: Self.expandedMaxHeight)
                    .accessibilityIdentifier("captureNotesAll")
                } else {
                    ForEach(Array(notes.prefix(Self.inlineCount).enumerated()), id: \.offset) { _, note in
                        Text(note).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                if notes.count > Self.inlineCount {
                    Button(showsAll ? "Show fewer" : "Show all \(notes.count) notes") {
                        withAnimation(reduceMotion ? nil : .easeOut(duration: 0.15)) { showsAll.toggle() }
                    }
                    .buttonStyle(.link).font(.caption)
                    .accessibilityIdentifier("captureNotesToggle")
                }
            }
            Spacer(minLength: 8)
            Button("Dismiss", action: onDismiss)
                .buttonStyle(.link).font(.caption)
                .accessibilityLabel("Dismiss capture notes")
        }
        .padding(.horizontal, 12).padding(.vertical, 8)
        .background(BoxTheme.warning.opacity(0.10), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Capture notes")
        .accessibilityValue(notes.joined(separator: ". "))
    }
}

/// Hosts the canvas in an `NSScrollView` sized for the current zoom. The render
/// scale is explicit: "Fit" and "Fit Width" are computed from the real clip size
/// (legacy scrollers deducted), magnifications are applied literally, and the
/// document is padded to the viewport only for centring, which never changes
/// the magnification. The scale actually laid out is reported back so zoom
/// steps and the Fit label follow what is on screen.
struct ZoomableAnnotationCanvas: NSViewRepresentable {
    @ObservedObject var viewModel: ScreenshotPopupViewModel

    func makeNSView(context: Context) -> ZoomableCanvasScrollView {
        ZoomableCanvasScrollView(viewModel: viewModel)
    }

    func updateNSView(_ view: ZoomableCanvasScrollView, context: Context) {
        view.apply(zoom: viewModel.zoom, imageSize: viewModel.visibleImageSize)
    }
}

final class ZoomableCanvasScrollView: NSScrollView {
    private let viewModel: ScreenshotPopupViewModel
    private let hosting: NSHostingView<AnnotationCanvasView>
    private let document = FlippedDocumentView()
    private var zoom: CanvasZoom = .fit
    private var imageSize: CGSize = .zero
    private var laidOut: (bounds: CGSize, zoom: CanvasZoom, image: CGSize)?
    /// The scale the document is currently laid out at (0 until the first layout).
    private(set) var renderScale: CGFloat = 0

    init(viewModel: ScreenshotPopupViewModel) {
        self.viewModel = viewModel
        hosting = NSHostingView(rootView: AnnotationCanvasView(viewModel: viewModel, renderScale: 1))
        super.init(frame: .zero)
        hasVerticalScroller = true
        hasHorizontalScroller = true
        autohidesScrollers = true
        borderType = .noBorder
        drawsBackground = false
        hosting.autoresizingMask = [.width, .height]
        document.addSubview(hosting)
        documentView = document
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    func apply(zoom: CanvasZoom, imageSize: CGSize) {
        self.zoom = zoom
        self.imageSize = imageSize
        relayout()
    }

    override func layout() {
        super.layout()
        relayout()
    }

    /// The viewport the image can use for `zoom`, deducting legacy scrollers when
    /// the content will overflow that axis.
    static func availableSize(bounds: CGSize, zoom: CanvasZoom, imageSize: CGSize, legacyScrollerWidth: CGFloat) -> CGSize {
        var available = bounds
        guard legacyScrollerWidth > 0, imageSize.width > 0, imageSize.height > 0 else { return available }
        let first = zoom.scale(imageSize: imageSize, available: available)
        if imageSize.height * first > available.height + 0.5 { available.width = max(1, available.width - legacyScrollerWidth) }
        let second = zoom.scale(imageSize: imageSize, available: available)
        if imageSize.width * second > available.width + 0.5 { available.height = max(1, available.height - legacyScrollerWidth) }
        return available
    }

    private func relayout() {
        let full = bounds.size
        guard full.width > 0, full.height > 0, imageSize.width > 0, imageSize.height > 0 else { return }
        if let laidOut, laidOut.bounds == full, laidOut.zoom == zoom, laidOut.image == imageSize { return }
        let legacy = scrollerStyle == .legacy ? NSScroller.scrollerWidth(for: .regular, scrollerStyle: .legacy) : 0
        let available = Self.availableSize(bounds: full, zoom: zoom, imageSize: imageSize, legacyScrollerWidth: legacy)
        let scale = zoom.scale(imageSize: imageSize, available: available)
        let content = zoom.contentSize(imageSize: imageSize, available: available)
        // Keep the point at the centre of the viewport where it was, in image terms.
        let visible = contentView.bounds
        let previous = document.frame.size
        let centre = previous.width > 0 && previous.height > 0
            ? CGPoint(x: visible.midX / previous.width, y: visible.midY / previous.height)
            : CGPoint(x: 0.5, y: 0)
        hosting.rootView = AnnotationCanvasView(viewModel: viewModel, renderScale: scale)
        document.frame = CGRect(origin: .zero, size: content)
        hosting.frame = document.bounds
        laidOut = (full, zoom, imageSize)
        renderScale = scale
        let clip = contentView.bounds.size
        let target = CGPoint(x: min(max(0, centre.x * content.width - clip.width / 2), max(0, content.width - clip.width)),
                             y: min(max(0, centre.y * content.height - clip.height / 2), max(0, content.height - clip.height)))
        contentView.scroll(to: target)
        reflectScrolledClipView(contentView)
        let viewModel = self.viewModel
        DispatchQueue.main.async { viewModel.reportDisplayedScale(scale) }
    }
}

/// A flipped document view so a tall image starts at its top, like a page.
final class FlippedDocumentView: NSView {
    override var isFlipped: Bool { true }
}
