import SwiftUI

struct AnnotationCanvasView: View {
    @ObservedObject var viewModel: ScreenshotPopupViewModel

    @State private var dragStart: CGPoint?
    @State private var dragCurrent: CGPoint?
    @State private var freehandPoints: [CGPoint] = []
    @State private var committedTextDragID: UUID?
    @State private var committedTextDragStartOrigin: CGPoint?
    @State private var editingTextDragStartOrigin: CGPoint?

    var body: some View {
        GeometryReader { geometry in
            let image = viewModel.previewImage
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

                previewLayer(viewport: viewport)
                draggableTextAnnotationLayer(viewport: viewport)
                inlineTextEditor(viewport: viewport)
            }
            .contentShape(Rectangle())
            .gesture(dragGesture(viewport: viewport))
        }
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
    private func previewLayer(viewport: ImageViewport) -> some View {
        if viewModel.activeTool == .pen, freehandPoints.count > 1 {
            Path { path in
                let first = viewport.imagePointToViewPoint(freehandPoints[0])
                path.move(to: first)
                for point in freehandPoints.dropFirst() {
                    path.addLine(to: viewport.imagePointToViewPoint(point))
                }
            }
            .stroke(Color(nsColor: viewModel.style.strokeColor.nsColor), style: StrokeStyle(lineWidth: viewModel.style.lineWidth, lineCap: .round, lineJoin: .round))
        } else if let rect = previewRect(viewport: viewport) {
            switch viewModel.activeTool {
            case .arrow:
                Path { path in
                    guard let dragStart, let dragCurrent else { return }
                    path.move(to: viewport.imagePointToViewPoint(dragStart))
                    path.addLine(to: viewport.imagePointToViewPoint(dragCurrent))
                }
                    .stroke(Color(nsColor: viewModel.style.strokeColor.nsColor), lineWidth: viewModel.style.lineWidth)
            case .rectangle, .crop:
                Rectangle()
                    .stroke(Color(nsColor: viewModel.style.strokeColor.nsColor), lineWidth: max(viewModel.style.lineWidth, 2))
                    .frame(width: rect.width, height: rect.height)
                    .position(x: rect.midX, y: rect.midY)
            case .highlight:
                Rectangle()
                    .fill(Color.yellow.opacity(0.32))
                    .frame(width: rect.width, height: rect.height)
                    .position(x: rect.midX, y: rect.midY)
            case .blur:
                Rectangle()
                    .fill(Color.black.opacity(0.68))
                    .frame(width: rect.width, height: rect.height)
                    .position(x: rect.midX, y: rect.midY)
            default:
                EmptyView()
            }
        }
    }

    private func dragGesture(viewport: ImageViewport) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { value in
                if viewModel.editingTextAnnotationID != nil { return }
                let point = viewport.viewPointToImagePoint(value.location)
                if dragStart == nil {
                    dragStart = viewport.viewPointToImagePoint(value.startLocation)
                    freehandPoints = []
                }
                dragCurrent = point
                if viewModel.activeTool == .pen {
                    freehandPoints.append(point)
                }
            }
            .onEnded { value in
                if viewModel.editingTextAnnotationID != nil { return }
                let end = viewport.viewPointToImagePoint(value.location)
                guard let start = dragStart else { resetDrag(); return }
                commit(start: start, end: end)
                resetDrag()
            }
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
            viewModel.addVisibleAnnotation(.freehand(points: simplify(freehandPoints)))
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

    private func simplify(_ points: [CGPoint]) -> [CGPoint] {
        guard points.count > 2 else { return points }
        var result: [CGPoint] = []
        var last: CGPoint?
        for point in points {
            if let previous = last, hypot(point.x - previous.x, point.y - previous.y) < 2 { continue }
            result.append(point)
            last = point
        }
        return result
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
            .position(x: viewFrame.midX, y: viewFrame.midY)
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

private struct InlineAnnotationTextField: NSViewRepresentable {
    @Binding var text: String
    var onCommit: () -> Void
    var onCancel: () -> Void

    func makeNSView(context: Context) -> NSTextField {
        let field = NSTextField(string: text)
        field.delegate = context.coordinator
        field.isBordered = true
        field.isBezeled = true
        field.drawsBackground = true
        field.backgroundColor = NSColor.textBackgroundColor.withAlphaComponent(0.92)
        field.font = .systemFont(ofSize: 18, weight: .semibold)
        field.focusRingType = .exterior
        DispatchQueue.main.async {
            field.window?.makeFirstResponder(field)
            field.currentEditor()?.selectAll(nil)
        }
        return field
    }

    func updateNSView(_ nsView: NSTextField, context: Context) {
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
