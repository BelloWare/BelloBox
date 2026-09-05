import SwiftUI

struct AnnotationCanvasView: View {
    @ObservedObject var viewModel: ScreenshotPopupViewModel
    /// When set, dragging the canvas in Select mode moves the whole capture selection.
    var selectionMoveHandler: SelectionMoveGestureHandler? = nil

    @State private var isMovingSelection = false
    @State private var dragStart: CGPoint?
    @State private var dragCurrent: CGPoint?
    @State private var freehandPoints: [CGPoint] = []
    @State private var committedTextDragID: UUID?
    @State private var committedTextDragStartOrigin: CGPoint?
    @State private var editingTextDragStartOrigin: CGPoint?

    var body: some View {
        GeometryReader { geometry in
            let image = viewModel.basePreviewImage()
            let imageSize = viewModel.visibleImageSize
            let viewport = ImageViewport(imageSize: imageSize, viewSize: geometry.size)

            ZStack(alignment: .topLeading) {
                Color.black.opacity(0.06)
                Image(nsImage: NSImage(cgImage: image, size: imageSize))
                    .resizable()
                    .interpolation(.high)
                    .frame(width: viewport.fittedImageRect.width, height: viewport.fittedImageRect.height)
                    .position(x: viewport.fittedImageRect.midX, y: viewport.fittedImageRect.midY)

                if viewModel.ocrPanel.showTextRegions, let result = viewModel.document.activeOCRResult {
                    OCRTextRegionsOverlayView(regions: result.regions, viewport: viewport)
                }

                AnnotationDrawingView(
                    annotations: viewModel.visibleAnnotations + draftAnnotations,
                    imageSize: imageSize
                )
                .frame(width: viewport.fittedImageRect.width, height: viewport.fittedImageRect.height)
                .position(x: viewport.fittedImageRect.midX, y: viewport.fittedImageRect.midY)
                .allowsHitTesting(false)
                cropPreview(viewport: viewport)
                draggableTextAnnotationLayer(viewport: viewport)
                inlineTextEditor(viewport: viewport)
            }
            .contentShape(Rectangle())
            .gesture(dragGesture(viewport: viewport, geometry: geometry))
        }
    }

    /// The in-progress shape uses the same style and renderer as its committed form.
    private var draftAnnotations: [ScreenshotAnnotation] {
        guard let start = dragStart, let end = dragCurrent else { return [] }
        let rect = CGRect(x: min(start.x, end.x), y: min(start.y, end.y), width: abs(end.x - start.x), height: abs(end.y - start.y))
        let kind: AnnotationKind
        let style: AnnotationStyle
        switch viewModel.activeTool {
        case .pen:
            kind = .freehand(points: freehandPoints)
            style = viewModel.style
        case .arrow:
            kind = .arrow(start: start, end: end)
            style = viewModel.style
        case .rectangle:
            kind = .rectangle(rect)
            style = viewModel.style
        case .highlight:
            kind = .highlight(rect)
            style = .highlight
        case .blur:
            kind = .blur(rect)
            style = .redaction
        default:
            return []
        }
        return [ScreenshotAnnotation(kind: kind, style: style)]
    }

    @ViewBuilder
    private func draggableTextAnnotationLayer(viewport: ImageViewport) -> some View {
        if viewModel.activeTool == .select || viewModel.activeTool == .text {
            ForEach(viewModel.visibleTextAnnotationFrames) { annotation in
                let viewFrame = viewport.imageRectToViewRect(annotation.frame)
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(Color.clear)
                    .overlay(
                        RoundedRectangle(cornerRadius: 7, style: .continuous)
                            .stroke(BoxTheme.accent.opacity(committedTextDragID == annotation.id ? 0.95 : 0.42), lineWidth: 1.5)
                    )
                    .frame(width: max(viewFrame.width, 44), height: max(viewFrame.height, 30))
                    .position(x: viewFrame.midX, y: viewFrame.midY)
                    .contentShape(Rectangle())
                    .gesture(committedTextDragGesture(annotation: annotation, viewport: viewport))
            }
        }
    }

    private func committedTextDragGesture(annotation: VisibleTextAnnotationFrame, viewport: ImageViewport) -> some Gesture {
        DragGesture(minimumDistance: 2)
            .onChanged { value in
                if committedTextDragID != annotation.id {
                    committedTextDragID = annotation.id
                    committedTextDragStartOrigin = annotation.frame.origin
                    viewModel.beginMovingTextAnnotation(id: annotation.id)
                }
                let start = committedTextDragStartOrigin ?? annotation.frame.origin
                let delta = viewport.viewTranslationToImageTranslation(value.translation)
                viewModel.moveTextAnnotation(
                    id: annotation.id,
                    toVisibleOrigin: CGPoint(x: start.x + delta.width, y: start.y + delta.height)
                )
            }
            .onEnded { _ in
                viewModel.endMovingTextAnnotation(id: annotation.id)
                committedTextDragID = nil
                committedTextDragStartOrigin = nil
            }
    }

    @ViewBuilder
    private func cropPreview(viewport: ImageViewport) -> some View {
        if viewModel.activeTool == .crop, let rect = previewRect(viewport: viewport) {
            Rectangle()
                .stroke(BoxTheme.accent, style: StrokeStyle(lineWidth: 2, dash: [5, 3]))
                .frame(width: rect.width, height: rect.height)
                .position(x: rect.midX, y: rect.midY)
                .allowsHitTesting(false)
        }
    }

    private func dragGesture(viewport: ImageViewport, geometry: GeometryProxy) -> some Gesture {
        // Global coordinates keep `translation` stable while the canvas itself moves
        // during a selection drag; local points are derived from the canvas origin.
        DragGesture(minimumDistance: 0, coordinateSpace: .global)
            .onChanged { value in
                if viewModel.editingTextAnnotationID != nil { return }
                if viewModel.activeTool == .select, let handler = selectionMoveHandler {
                    if !isMovingSelection {
                        isMovingSelection = true
                        handler.onBegin()
                    }
                    handler.onChange(value.translation)
                    return
                }
                let origin = geometry.frame(in: .global).origin
                let point = viewport.viewPointToImagePoint(Self.localPoint(value.location, origin: origin))
                if dragStart == nil {
                    dragStart = viewport.viewPointToImagePoint(Self.localPoint(value.startLocation, origin: origin))
                    freehandPoints = [dragStart ?? point]
                }
                dragCurrent = point
                if viewModel.activeTool == .pen {
                    if freehandPoints.last != point { freehandPoints.append(point) }
                }
            }
            .onEnded { value in
                if isMovingSelection {
                    isMovingSelection = false
                    selectionMoveHandler?.onEnd()
                    resetDrag()
                    return
                }
                if viewModel.editingTextAnnotationID != nil { return }
                let origin = geometry.frame(in: .global).origin
                let end = viewport.viewPointToImagePoint(Self.localPoint(value.location, origin: origin))
                guard let start = dragStart else { resetDrag(); return }
                if viewModel.activeTool == .pen, freehandPoints.last != end { freehandPoints.append(end) }
                commit(start: start, end: end)
                resetDrag()
            }
    }

    private static func localPoint(_ point: CGPoint, origin: CGPoint) -> CGPoint {
        CGPoint(x: point.x - origin.x, y: point.y - origin.y)
    }

    private func commit(start: CGPoint, end: CGPoint) {
        if viewModel.activeTool == .text {
            viewModel.handleCanvasTap(visiblePoint: end)
            return
        }
        if viewModel.activeTool == .eraser {
            viewModel.eraseAnnotation(atVisiblePoint: end)
            return
        }
        let rect = CGRect(
            x: min(start.x, end.x),
            y: min(start.y, end.y),
            width: abs(start.x - end.x),
            height: abs(start.y - end.y)
        )
        guard viewModel.activeTool == .pen || rect.width >= 3 || rect.height >= 3 else { return }

        switch viewModel.activeTool {
        case .pen:
            guard !freehandPoints.isEmpty else { return }
            viewModel.addVisibleAnnotation(.freehand(points: freehandPoints))
        case .arrow:
            viewModel.addVisibleAnnotation(.arrow(start: start, end: end))
        case .rectangle:
            viewModel.addVisibleAnnotation(.rectangle(rect))
        case .highlight:
            viewModel.addVisibleAnnotation(.highlight(rect))
        case .crop:
            viewModel.applyVisibleCrop(rect)
        case .blur:
            viewModel.addVisibleAnnotation(.blur(rect))
        default:
            break
        }
    }

    private func previewRect(viewport: ImageViewport) -> CGRect? {
        guard let dragStart, let dragCurrent else { return nil }
        let a = viewport.imagePointToViewPoint(dragStart)
        let b = viewport.imagePointToViewPoint(dragCurrent)
        return CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(a.x - b.x), height: abs(a.y - b.y))
    }

    private func resetDrag() {
        dragStart = nil
        dragCurrent = nil
        freehandPoints = []
    }

    @ViewBuilder
    private func inlineTextEditor(viewport: ImageViewport) -> some View {
        if let frame = viewModel.visibleTextFrameForEditingAnnotation() {
            let viewFrame = viewport.imageRectToViewRect(frame)
            let fieldWidth = max(viewFrame.width, 120)
            let fieldHeight = max(viewFrame.height, 30)
            ZStack(alignment: .topLeading) {
                InlineAnnotationTextField(
                    text: Binding(
                        get: { viewModel.textForEditingAnnotation() },
                        set: { viewModel.updateEditingText($0) }
                    ),
                    fontSize: viewModel.style.fontSize * viewport.fittedImageRect.width / max(viewport.imageSize.width, 1),
                    onCommit: { viewModel.endTextEditing() },
                    onCancel: { viewModel.cancelTextEditing() }
                )
                .frame(width: fieldWidth, height: fieldHeight)

                Image(systemName: "arrow.up.and.down.and.arrow.left.and.right")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(.white)
                    .frame(width: 22, height: 22)
                    .background(Circle().fill(BoxTheme.accentGradient))
                    .overlay(Circle().strokeBorder(.white.opacity(0.45), lineWidth: 1))
                    .shadow(color: .black.opacity(0.28), radius: 5, y: 2)
                    .offset(x: -10, y: -10)
                    .contentShape(Circle())
                    .gesture(editingTextDragGesture(frame: frame, viewport: viewport))
            }
            .frame(width: fieldWidth, height: fieldHeight)
            .position(x: viewFrame.minX + fieldWidth / 2, y: viewFrame.minY + fieldHeight / 2)
        }
    }

    private func editingTextDragGesture(frame: CGRect, viewport: ImageViewport) -> some Gesture {
        DragGesture(minimumDistance: 2)
            .onChanged { value in
                if editingTextDragStartOrigin == nil {
                    editingTextDragStartOrigin = frame.origin
                }
                let start = editingTextDragStartOrigin ?? frame.origin
                let delta = viewport.viewTranslationToImageTranslation(value.translation)
                viewModel.moveEditingText(
                    toVisibleOrigin: CGPoint(x: start.x + delta.width, y: start.y + delta.height)
                )
            }
            .onEnded { _ in
                editingTextDragStartOrigin = nil
            }
    }
}

/// A transparent AppKit drawing surface shares Core Graphics rendering with PNG export.
/// Only the annotation layer is redrawn during a gesture, even for a tall scroll capture.
struct AnnotationDrawingView: NSViewRepresentable {
    var annotations: [ScreenshotAnnotation]
    var imageSize: CGSize

    func makeNSView(context: Context) -> AnnotationDrawingNSView {
        let view = AnnotationDrawingNSView()
        view.wantsLayer = true
        return view
    }

    func updateNSView(_ view: AnnotationDrawingNSView, context: Context) {
        view.annotations = annotations
        view.imageSize = imageSize
        view.needsDisplay = true
    }
}

final class AnnotationDrawingNSView: NSView {
    var annotations: [ScreenshotAnnotation] = []
    var imageSize: CGSize = .zero

    override var isOpaque: Bool { false }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func draw(_ dirtyRect: NSRect) {
        guard imageSize.width > 0, imageSize.height > 0,
              let context = NSGraphicsContext.current?.cgContext else { return }
        context.saveGState()
        defer { context.restoreGState() }
        context.clear(bounds)
        context.clip(to: bounds)
        context.scaleBy(x: bounds.width / imageSize.width, y: bounds.height / imageSize.height)
        AnnotationRenderer.drawAnnotations(annotations, in: context, imageHeight: imageSize.height)
    }
}

/// Lets the capture overlay move the whole selection when the user drags the canvas in
/// Select mode. Translations are reported in the canvas's point coordinate space.
struct SelectionMoveGestureHandler {
    var onBegin: () -> Void
    var onChange: (CGSize) -> Void
    var onEnd: () -> Void
}

private struct InlineAnnotationTextField: NSViewRepresentable {
    @Binding var text: String
    var fontSize: CGFloat
    var onCommit: () -> Void
    var onCancel: () -> Void

    func makeNSView(context: Context) -> NSTextField {
        let field = NSTextField(string: text)
        field.delegate = context.coordinator
        field.isBordered = true
        field.isBezeled = true
        field.drawsBackground = true
        field.backgroundColor = NSColor.textBackgroundColor.withAlphaComponent(0.92)
        field.font = .systemFont(ofSize: max(1, fontSize), weight: .semibold)
        field.focusRingType = .exterior
        DispatchQueue.main.async {
            field.window?.makeFirstResponder(field)
            field.currentEditor()?.selectAll(nil)
        }
        return field
    }

    func updateNSView(_ nsView: NSTextField, context: Context) {
        nsView.font = .systemFont(ofSize: max(1, fontSize), weight: .semibold)
        if nsView.stringValue != text {
            nsView.stringValue = text
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(text: $text, onCommit: onCommit, onCancel: onCancel)
    }

    final class Coordinator: NSObject, NSTextFieldDelegate {
        @Binding var text: String
        var onCommit: () -> Void
        var onCancel: () -> Void
        var didCancel = false

        init(text: Binding<String>, onCommit: @escaping () -> Void, onCancel: @escaping () -> Void) {
            _text = text
            self.onCommit = onCommit
            self.onCancel = onCancel
        }

        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSTextField else { return }
            text = field.stringValue
        }

        func controlTextDidEndEditing(_ notification: Notification) {
            guard let info = notification.userInfo,
                  let movement = info["NSTextMovement"] as? Int,
                  movement == NSCancelTextMovement
            else {
                onCommit()
                return
            }
            if !didCancel {
                onCancel()
            }
            didCancel = false
        }

        func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                text = control.stringValue
                onCommit()
                return true
            }
            if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
                didCancel = true
                onCancel()
                return true
            }
            return false
        }
    }
}
