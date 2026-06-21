import AppKit
import CoreGraphics
import Foundation

enum AnnotationTool: String, CaseIterable, Identifiable {
    case select
    case pen
    case arrow
    case rectangle
    case highlight
    case text
    case crop
    case blur
    case eraser

    var id: String { rawValue }

    var label: String {
        switch self {
        case .select: return "Select"
        case .pen: return "Pen"
        case .arrow: return "Arrow"
        case .rectangle: return "Rect"
        case .highlight: return "Highlight"
        case .text: return "Text"
        case .crop: return "Crop"
        case .blur: return "Blur"
        case .eraser: return "Eraser"
        }
    }

    var symbol: String {
        switch self {
        case .select: return "cursorarrow"
        case .pen: return "pencil.tip"
        case .arrow: return "arrow.up.right"
        case .rectangle: return "rectangle"
        case .highlight: return "highlighter"
        case .text: return "textformat"
        case .crop: return "crop"
        case .blur: return "checkerboard.rectangle"
        case .eraser: return "eraser"
        }
    }
}

struct ScreenshotAnnotation: Identifiable, Equatable {
    let id: UUID
    var kind: AnnotationKind
    var style: AnnotationStyle
    var createdAt: Date

    init(id: UUID = UUID(), kind: AnnotationKind, style: AnnotationStyle = .default, createdAt: Date = Date()) {
        self.id = id
        self.kind = kind
        self.style = style
        self.createdAt = createdAt
    }
}

enum AnnotationKind: Equatable {
    case freehand(points: [CGPoint])
    case arrow(start: CGPoint, end: CGPoint)
    case rectangle(CGRect)
    case highlight(CGRect)
    case text(String, origin: CGPoint, maxWidth: CGFloat)
    case blur(CGRect)

    var bounds: CGRect {
        switch self {
        case let .freehand(points):
            guard let first = points.first else { return .zero }
            return points.dropFirst().reduce(CGRect(origin: first, size: .zero)) { rect, point in
                rect.union(CGRect(origin: point, size: .zero))
            }
        case let .arrow(start, end):
            return CGRect(
                x: min(start.x, end.x),
                y: min(start.y, end.y),
                width: abs(start.x - end.x),
                height: abs(start.y - end.y)
            )
        case let .rectangle(rect), let .highlight(rect), let .blur(rect):
            return rect
        case let .text(_, origin, maxWidth):
            return CGRect(x: origin.x, y: origin.y, width: maxWidth, height: 48)
        }
    }
}

struct AnnotationStyle: Equatable {
    var strokeColor: CodableColor
    var fillColor: CodableColor?
    var lineWidth: CGFloat
    var opacity: CGFloat
    var fontSize: CGFloat

    static let `default` = AnnotationStyle(
        strokeColor: CodableColor(red: 0.95, green: 0.42, blue: 0.08, alpha: 1),
        fillColor: nil,
        lineWidth: 4,
        opacity: 1,
        fontSize: 18
    )

    static let highlight = AnnotationStyle(
        strokeColor: CodableColor(red: 1.0, green: 0.78, blue: 0.12, alpha: 1),
        fillColor: CodableColor(red: 1.0, green: 0.85, blue: 0.12, alpha: 0.32),
        lineWidth: 0,
        opacity: 0.45,
        fontSize: 18
    )

    static let redaction = AnnotationStyle(
        strokeColor: CodableColor(red: 0.12, green: 0.12, blue: 0.12, alpha: 1),
        fillColor: CodableColor(red: 0.16, green: 0.16, blue: 0.16, alpha: 1),
        lineWidth: 0,
        opacity: 1,
        fontSize: 18
    )
}

struct ImageViewport: Equatable {
    var imageSize: CGSize
    var viewSize: CGSize
    var fittedImageRect: CGRect

    init(imageSize: CGSize, viewSize: CGSize) {
        self.imageSize = imageSize
        self.viewSize = viewSize
        guard imageSize.width > 0, imageSize.height > 0, viewSize.width > 0, viewSize.height > 0 else {
            fittedImageRect = .zero
            return
        }
        let scale = min(viewSize.width / imageSize.width, viewSize.height / imageSize.height)
        let size = CGSize(width: imageSize.width * scale, height: imageSize.height * scale)
        fittedImageRect = CGRect(
            x: (viewSize.width - size.width) / 2,
            y: (viewSize.height - size.height) / 2,
            width: size.width,
            height: size.height
        )
    }

    func viewPointToImagePoint(_ point: CGPoint) -> CGPoint {
        guard fittedImageRect.width > 0, fittedImageRect.height > 0 else { return .zero }
        let x = (point.x - fittedImageRect.minX) / fittedImageRect.width * imageSize.width
        let y = (point.y - fittedImageRect.minY) / fittedImageRect.height * imageSize.height
        return CGPoint(x: min(max(x, 0), imageSize.width), y: min(max(y, 0), imageSize.height))
    }

    func imagePointToViewPoint(_ point: CGPoint) -> CGPoint {
        guard imageSize.width > 0, imageSize.height > 0 else { return .zero }
        let x = fittedImageRect.minX + point.x / imageSize.width * fittedImageRect.width
        let y = fittedImageRect.minY + point.y / imageSize.height * fittedImageRect.height
        return CGPoint(x: x, y: y)
    }

    func imageRectToViewRect(_ rect: CGRect) -> CGRect {
        let a = imagePointToViewPoint(rect.origin)
        let b = imagePointToViewPoint(CGPoint(x: rect.maxX, y: rect.maxY))
        return CGRect(x: a.x, y: a.y, width: b.x - a.x, height: b.y - a.y).standardized
    }

    func viewTranslationToImageTranslation(_ translation: CGSize) -> CGSize {
        guard fittedImageRect.width > 0, fittedImageRect.height > 0 else { return .zero }
        return CGSize(
            width: translation.width / fittedImageRect.width * imageSize.width,
            height: translation.height / fittedImageRect.height * imageSize.height
        )
    }
}
