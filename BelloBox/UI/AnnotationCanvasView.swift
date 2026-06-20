import SwiftUI

struct AnnotationCanvasView: View {
    @ObservedObject var viewModel: ScreenshotPopupViewModel

    @State private var dragStart: CGPoint?
    @State private var dragCurrent: CGPoint?
    @State private var freehandPoints: [CGPoint] = []

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
            }
            .contentShape(Rectangle())
            .gesture(dragGesture(viewport: viewport))
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
}
