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
        case .rectangle: return "Rectangle"
        case .highlight: return "Highlight"
        case .text: return "Text"
        case .crop: return "Crop"
        case .blur: return "Mask"
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

/// A brush pass of the eraser over one annotation: a polyline in document pixels
/// and the brush diameter. Rendering punches these out of that annotation only,
/// so the screenshot pixels and every other annotation stay untouched.
struct EraserStroke: Equatable {
    var points: [CGPoint]
    var width: CGFloat
}

struct ScreenshotAnnotation: Identifiable, Equatable {
    let id: UUID
    var kind: AnnotationKind
    var style: AnnotationStyle
    var createdAt: Date
    var erasures: [EraserStroke]

    init(id: UUID = UUID(), kind: AnnotationKind, style: AnnotationStyle = .default, createdAt: Date = Date(), erasures: [EraserStroke] = []) {
        self.id = id
        self.kind = kind
        self.style = style
        self.createdAt = createdAt
        self.erasures = erasures
    }

    var isErased: Bool { !erasures.isEmpty }

    /// The annotation moved by (dx, dy), erasures included, so holes travel with the shape.
    func offset(dx: CGFloat, dy: CGFloat) -> ScreenshotAnnotation {
        guard dx != 0 || dy != 0 else { return self }
        var copy = self
        copy.kind = kind.offset(dx: dx, dy: dy)
        copy.erasures = erasures.map { stroke in
            EraserStroke(points: stroke.points.map { CGPoint(x: $0.x + dx, y: $0.y + dy) }, width: stroke.width)
        }
        return copy
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

    func offset(dx: CGFloat, dy: CGFloat) -> AnnotationKind {
        switch self {
        case let .freehand(points):
            return .freehand(points: points.map { CGPoint(x: $0.x + dx, y: $0.y + dy) })
        case let .arrow(start, end):
            return .arrow(start: CGPoint(x: start.x + dx, y: start.y + dy), end: CGPoint(x: end.x + dx, y: end.y + dy))
        case let .rectangle(rect):
            return .rectangle(rect.offsetBy(dx: dx, dy: dy))
        case let .highlight(rect):
            return .highlight(rect.offsetBy(dx: dx, dy: dy))
        case let .text(text, origin, maxWidth):
            return .text(text, origin: CGPoint(x: origin.x + dx, y: origin.y + dy), maxWidth: maxWidth)
        case let .blur(rect):
            return .blur(rect.offsetBy(dx: dx, dy: dy))
        }
    }
}

/// Decoration drawn over a mask's opaque fill. Every pattern sits on the solid
/// colour, so a masked region never shows what was underneath.
enum MaskPattern: String, CaseIterable, Identifiable, Equatable, Codable {
    case solid
    case stripes
    case dots

    var id: String { rawValue }

    var label: String {
        switch self {
        case .solid: return "Solid"
        case .stripes: return "Stripes"
        case .dots: return "Dots"
        }
    }

    var symbol: String {
        switch self {
        case .solid: return "square.fill"
        case .stripes: return "line.diagonal"
        case .dots: return "circle.grid.3x3.fill"
        }
    }
}

struct AnnotationStyle: Equatable {
    var strokeColor: CodableColor
    var fillColor: CodableColor?
    var lineWidth: CGFloat
    var opacity: CGFloat
    var fontSize: CGFloat
    /// Pattern over a mask's fill; ignored by every other annotation kind.
    var maskPattern: MaskPattern = .stripes

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

    /// Solid, fully opaque fill shared by the editor preview and the exported redaction.
    static let redactionFillColor = CodableColor(red: 0.16, green: 0.16, blue: 0.16, alpha: 1)

    static let redaction = AnnotationStyle(
        strokeColor: CodableColor(red: 0.12, green: 0.12, blue: 0.12, alpha: 1),
        fillColor: redactionFillColor,
        lineWidth: 0,
        opacity: 1,
        fontSize: 18
    )

    /// A mask style with the given opaque fill and pattern. Alpha is always 1.
    static func mask(fill: CodableColor, pattern: MaskPattern) -> AnnotationStyle {
        var style = redaction
        var opaque = fill
        opaque.alpha = 1
        style.fillColor = opaque
        style.maskPattern = pattern
        return style
    }

    /// The mask fill colour, always opaque.
    var maskFill: CodableColor {
        var fill = fillColor ?? Self.redactionFillColor
        fill.alpha = 1
        return fill
    }

    /// Fills offered in the toolbar: neutral charcoal, black and white, plus the
    /// warm brand tones so a mask can match the annotation palette.
    static let maskFillPresets: [(name: String, color: CodableColor)] = [
        ("Charcoal", redactionFillColor),
        ("Black", CodableColor(red: 0.02, green: 0.02, blue: 0.02, alpha: 1)),
        ("White", CodableColor(red: 0.98, green: 0.97, blue: 0.95, alpha: 1)),
        ("Orange", CodableColor(red: 0.89, green: 0.46, blue: 0.15, alpha: 1)),
        ("Teal", CodableColor(red: 0.06, green: 0.40, blue: 0.43, alpha: 1)),
        ("Plum", CodableColor(red: 0.46, green: 0.26, blue: 0.70, alpha: 1)),
    ]
}

/// Distance tests shared by the eraser (which annotations a brush pass touches)
/// and by cleanup (an annotation whose every visible sample is erased is dropped).
enum AnnotationGeometry {
    static func distance(from point: CGPoint, toSegment a: CGPoint, _ b: CGPoint) -> CGFloat {
        let dx = b.x - a.x, dy = b.y - a.y
        let lengthSquared = dx * dx + dy * dy
        guard lengthSquared > 0 else { return hypot(point.x - a.x, point.y - a.y) }
        let t = max(0, min(1, ((point.x - a.x) * dx + (point.y - a.y) * dy) / lengthSquared))
        return hypot(point.x - (a.x + t * dx), point.y - (a.y + t * dy))
    }

    static func distance(fromSegment a: CGPoint, _ b: CGPoint, toSegment c: CGPoint, _ d: CGPoint) -> CGFloat {
        if segmentsIntersect(a, b, c, d) { return 0 }
        return min(distance(from: a, toSegment: c, d), distance(from: b, toSegment: c, d),
                   distance(from: c, toSegment: a, b), distance(from: d, toSegment: a, b))
    }

    static func distance(fromSegment a: CGPoint, _ b: CGPoint, toRect rect: CGRect) -> CGFloat {
        let rect = rect.standardized
        if rect.contains(a) || rect.contains(b) { return 0 }
        let corners = [CGPoint(x: rect.minX, y: rect.minY), CGPoint(x: rect.maxX, y: rect.minY),
                       CGPoint(x: rect.maxX, y: rect.maxY), CGPoint(x: rect.minX, y: rect.maxY)]
        var best = CGFloat.greatestFiniteMagnitude
        for index in 0..<4 {
            best = min(best, distance(fromSegment: a, b, toSegment: corners[index], corners[(index + 1) % 4]))
        }
        return best
    }

    /// Whether a brush pass along `a`–`b` with `radius` touches the painted part of `annotation`.
    static func brush(from a: CGPoint, to b: CGPoint, radius: CGFloat, touches annotation: ScreenshotAnnotation) -> Bool {
        let lineWidth = max(annotation.style.lineWidth, 1)
        switch annotation.kind {
        case let .freehand(points):
            guard let first = points.first else { return false }
            if points.count == 1 { return distance(from: first, toSegment: a, b) <= radius + lineWidth / 2 }
            for index in 1..<points.count where distance(fromSegment: points[index - 1], points[index], toSegment: a, b) <= radius + lineWidth / 2 {
                return true
            }
            return false
        case let .arrow(start, end):
            let head = max(10, lineWidth * 3)
            return distance(fromSegment: start, end, toSegment: a, b) <= radius + lineWidth / 2
                || distance(from: end, toSegment: a, b) <= radius + head
        case let .rectangle(rect):
            // The stroke straddles the edge, so a filled rectangle is hit within lineWidth/2 outside it too.
            if annotation.style.fillColor != nil { return distance(fromSegment: a, b, toRect: rect) <= radius + lineWidth / 2 }
            let rect = rect.standardized
            let corners = [CGPoint(x: rect.minX, y: rect.minY), CGPoint(x: rect.maxX, y: rect.minY),
                           CGPoint(x: rect.maxX, y: rect.maxY), CGPoint(x: rect.minX, y: rect.maxY)]
            for index in 0..<4 where distance(fromSegment: corners[index], corners[(index + 1) % 4], toSegment: a, b) <= radius + lineWidth / 2 {
                return true
            }
            return false
        case let .highlight(rect), let .blur(rect):
            return distance(fromSegment: a, b, toRect: rect) <= radius
        case .text:
            return distance(fromSegment: a, b, toRect: AnnotationRenderer.paintedRect(for: annotation)) <= radius
        }
    }

    static func isCovered(_ point: CGPoint, by erasures: [EraserStroke]) -> Bool {
        for stroke in erasures {
            let radius = stroke.width / 2
            guard let first = stroke.points.first else { continue }
            if stroke.points.count == 1 {
                if hypot(point.x - first.x, point.y - first.y) <= radius { return true }
                continue
            }
            for index in 1..<stroke.points.count where distance(from: point, toSegment: stroke.points[index - 1], stroke.points[index]) <= radius {
                return true
            }
        }
        return false
    }

    private static func segmentsIntersect(_ a: CGPoint, _ b: CGPoint, _ c: CGPoint, _ d: CGPoint) -> Bool {
        func orientation(_ p: CGPoint, _ q: CGPoint, _ r: CGPoint) -> CGFloat {
            (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y)
        }
        let o1 = orientation(a, b, c), o2 = orientation(a, b, d), o3 = orientation(c, d, a), o4 = orientation(c, d, b)
        if (o1 > 0) != (o2 > 0), (o3 > 0) != (o4 > 0), o1 != 0, o2 != 0, o3 != 0, o4 != 0 { return true }
        return false
    }
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
        fittedImageRect = Self.centredRect(imageSize: imageSize, scale: scale, in: viewSize)
    }

    /// The image at an explicit magnification, centred in the view. The view may be
    /// larger than the image (padding) or smaller (the host scrolls); the scale is
    /// applied literally either way.
    init(imageSize: CGSize, viewSize: CGSize, scale: CGFloat) {
        self.imageSize = imageSize
        self.viewSize = viewSize
        guard imageSize.width > 0, imageSize.height > 0, scale > 0, scale.isFinite else {
            fittedImageRect = .zero
            return
        }
        fittedImageRect = Self.centredRect(imageSize: imageSize, scale: scale, in: viewSize)
    }

    private static func centredRect(imageSize: CGSize, scale: CGFloat, in viewSize: CGSize) -> CGRect {
        let size = CGSize(width: imageSize.width * scale, height: imageSize.height * scale)
        return CGRect(
            x: (viewSize.width - size.width) / 2,
            y: (viewSize.height - size.height) / 2,
            width: size.width,
            height: size.height
        )
    }

    /// View points per image pixel.
    var scale: CGFloat {
        guard imageSize.width > 0 else { return 1 }
        return fittedImageRect.width / imageSize.width
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

/// How the popup editor scales the image: fitted whole, fitted to the width
/// (natural for tall scrolling captures), or a fixed magnification.
enum CanvasZoom: Equatable {
    case fit
    case fitWidth
    case scale(CGFloat)

    static let steps: [CGFloat] = [0.25, 0.5, 0.75, 1, 1.5, 2, 3, 4]

    /// View points per image pixel for `imageSize` shown in `available`.
    func scale(imageSize: CGSize, available: CGSize) -> CGFloat {
        guard imageSize.width > 0, imageSize.height > 0, available.width > 0, available.height > 0 else { return 1 }
        switch self {
        case .fit:
            return min(available.width / imageSize.width, available.height / imageSize.height)
        case .fitWidth:
            return available.width / imageSize.width
        case let .scale(value):
            return max(0.05, min(value, 8))
        }
    }

    /// The canvas size for this zoom; never smaller than the viewport so the
    /// image stays centred when it does not fill it.
    func contentSize(imageSize: CGSize, available: CGSize) -> CGSize {
        let scale = scale(imageSize: imageSize, available: available)
        return CGSize(width: max(available.width, (imageSize.width * scale).rounded(.down)),
                      height: max(available.height, (imageSize.height * scale).rounded(.down)))
    }

    static func zoomedIn(from current: CGFloat) -> CanvasZoom {
        .scale(steps.first { $0 > current + 0.001 } ?? steps[steps.count - 1])
    }

    static func zoomedOut(from current: CGFloat) -> CanvasZoom {
        .scale(steps.last { $0 < current - 0.001 } ?? steps[0])
    }

    var label: String {
        switch self {
        case .fit: return "Fit"
        case .fitWidth: return "Fit Width"
        case let .scale(value): return "\(Int((value * 100).rounded()))%"
        }
    }
}
