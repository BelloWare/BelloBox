import AppKit
import CoreGraphics

enum AnnotationRenderError: LocalizedError, Equatable {
    case cannotCreateContext
    case cannotCropImage
    case cannotCreateImage

    var errorDescription: String? {
        switch self {
        case .cannotCreateContext:
            return "Could not create an image rendering context."
        case .cannotCropImage:
            return "Could not crop the screenshot."
        case .cannotCreateImage:
            return "Could not render the annotated screenshot."
        }
    }
}

enum AnnotationRenderer {
    static func render(_ document: ScreenshotDocument, outputScale: CGFloat? = nil) throws -> CGImage {
        try render(document, includeDecorativeAnnotations: true, target: .fullImage, outputScale: outputScale)
    }

    static func renderForOCR(_ document: ScreenshotDocument, target: OCRTarget, includeDecorativeAnnotations: Bool) throws -> CGImage {
        try render(document, includeDecorativeAnnotations: includeDecorativeAnnotations, target: target, outputScale: nil)
    }

    static func renderForExternalOCRUpload(_ document: ScreenshotDocument, target: OCRTarget) throws -> CGImage {
        try render(document, includeDecorativeAnnotations: false, target: target, outputScale: nil)
    }

    private static func render(
        _ document: ScreenshotDocument,
        includeDecorativeAnnotations: Bool,
        target: OCRTarget,
        outputScale: CGFloat?
    ) throws -> CGImage {
        let cropRect = effectiveCropRect(document: document, target: target)
        guard let croppedBase = cropImage(document.baseImage, to: cropRect) else {
            throw AnnotationRenderError.cannotCropImage
        }

        let width = croppedBase.width
        let height = croppedBase.height
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            throw AnnotationRenderError.cannotCreateContext
        }

        context.interpolationQuality = .high
        context.draw(croppedBase, in: CGRect(x: 0, y: 0, width: width, height: height))

        let annotations = shiftedAnnotations(document.annotations, cropRect: cropRect)
        drawAnnotations(annotations, in: context, imageHeight: CGFloat(height), includeDecorativeAnnotations: includeDecorativeAnnotations)

        guard let image = context.makeImage() else { throw AnnotationRenderError.cannotCreateImage }
        if let outputScale, outputScale > 0, outputScale != 1 {
            return try scale(image, by: outputScale)
        }
        return image
    }

    /// Shared by the live canvas and export. Geometry and style are in image pixels;
    /// the canvas scales the context, so text, strokes and arrowheads scale together.
    /// An annotation with erasures is drawn into a transparency layer bounded to its
    /// own area and the erasure brush is punched out of that layer alone, so the
    /// screenshot pixels and neighbouring annotations are never affected.
    static func drawAnnotations(
        _ annotations: [ScreenshotAnnotation],
        in context: CGContext,
        imageHeight: CGFloat,
        includeDecorativeAnnotations: Bool = true
    ) {
        if includeDecorativeAnnotations {
            drawHighlights(annotations, in: context, imageHeight: imageHeight)
            drawVectorAnnotations(annotations, in: context, imageHeight: imageHeight)
            drawTextAnnotations(annotations, in: context, imageHeight: imageHeight)
        }
        // A redaction must cover both the captured pixels and any annotations.
        applyRedactions(annotations, in: context, imageHeight: imageHeight)
    }

    /// The rectangle an annotation paints, in document pixels (top-left origin),
    /// including stroke width, arrowheads and the wrapped height of text.
    static func paintedRect(for annotation: ScreenshotAnnotation) -> CGRect {
        let lineWidth = max(annotation.style.lineWidth, 1)
        switch annotation.kind {
        case .freehand:
            return annotation.kind.bounds.insetBy(dx: -lineWidth, dy: -lineWidth)
        case .arrow:
            let head = max(10, lineWidth * 3)
            return annotation.kind.bounds.insetBy(dx: -(head + lineWidth), dy: -(head + lineWidth))
        case let .rectangle(rect):
            return rect.standardized.insetBy(dx: -lineWidth, dy: -lineWidth)
        case let .highlight(rect), let .blur(rect):
            return rect.standardized
        case let .text(text, origin, maxWidth):
            let attributed = attributedText(text, style: annotation.style)
            let height = textDrawingHeight(for: attributed, maxWidth: maxWidth, minimumHeight: annotation.style.fontSize * 1.4)
            return CGRect(x: origin.x, y: origin.y, width: maxWidth, height: height)
        }
    }

    private static func effectiveCropRect(document: ScreenshotDocument, target: OCRTarget) -> CGRect {
        let full = CGRect(origin: .zero, size: document.imageSize)
        switch target {
        case .fullImage:
            return (document.cropRect ?? full).intersection(full).integral
        case let .crop(rect):
            return rect.rect.intersection(full).integral
        case let .visibleAfterRedactions(crop):
            return (crop?.rect ?? document.cropRect ?? full).intersection(full).integral
        }
    }

    private static func cropImage(_ image: CGImage, to rect: CGRect) -> CGImage? {
        let full = CGRect(x: 0, y: 0, width: image.width, height: image.height)
        let crop = rect.intersection(full).integral
        guard crop.width > 0, crop.height > 0 else { return nil }
        return image.cropping(to: crop)
    }

    private static func shiftedAnnotations(_ annotations: [ScreenshotAnnotation], cropRect: CGRect) -> [ScreenshotAnnotation] {
        guard cropRect.origin != .zero else { return annotations }
        return annotations.map { $0.offset(dx: -cropRect.minX, dy: -cropRect.minY) }
    }

    /// Runs `body` directly for an intact annotation. For an erased one, the drawing
    /// goes into a transparency layer clipped to the annotation's painted area (so
    /// the layer stays small even on a tall capture) and the erasure strokes are
    /// removed from it with destination-out before it is composited.
    private static func drawing(_ annotation: ScreenshotAnnotation, in context: CGContext, imageHeight: CGFloat, body: () -> Void) {
        guard annotation.isErased else { body(); return }
        let brush = annotation.erasures.map(\.width).max() ?? 0
        let painted = paintedRect(for: annotation).insetBy(dx: -brush, dy: -brush)
        context.saveGState()
        context.clip(to: coreGraphicsRect(painted, imageHeight: imageHeight))
        context.beginTransparencyLayer(auxiliaryInfo: nil)
        body()
        context.setBlendMode(.destinationOut)
        context.setLineCap(.round)
        context.setLineJoin(.round)
        context.setStrokeColor(CGColor(gray: 0, alpha: 1))
        context.setFillColor(CGColor(gray: 0, alpha: 1))
        for stroke in annotation.erasures {
            guard let first = stroke.points.first else { continue }
            let width = max(stroke.width, 1)
            if stroke.points.count == 1 {
                let point = flip(first, imageHeight: imageHeight)
                context.fillEllipse(in: CGRect(x: point.x - width / 2, y: point.y - width / 2, width: width, height: width))
                continue
            }
            context.setLineWidth(width)
            context.move(to: flip(first, imageHeight: imageHeight))
            for point in stroke.points.dropFirst() {
                context.addLine(to: flip(point, imageHeight: imageHeight))
            }
            context.strokePath()
        }
        context.endTransparencyLayer()
        context.restoreGState()
    }

    private static func applyRedactions(_ annotations: [ScreenshotAnnotation], in context: CGContext, imageHeight: CGFloat) {
        for annotation in annotations {
            guard case let .blur(rect) = annotation.kind else { continue }
            drawing(annotation, in: context, imageHeight: imageHeight) {
                drawMask(rect, style: annotation.style, in: context, imageHeight: imageHeight)
            }
        }
    }

    /// The opaque fill first; any pattern is painted over it in a contrasting ink,
    /// so the region hides what was underneath whichever style is chosen.
    private static func drawMask(_ rect: CGRect, style: AnnotationStyle, in context: CGContext, imageHeight: CGFloat) {
        let cgRect = coreGraphicsRect(rect, imageHeight: imageHeight)
        context.saveGState()
        context.clip(to: cgRect)
        let fill = style.maskFill
        context.setFillColor(fill.cgColor)
        context.fill(cgRect)
        let ink = maskPatternInk(over: fill)
        switch style.maskPattern {
        case .solid:
            break
        case .stripes:
            context.setStrokeColor(ink)
            context.setLineWidth(1)
            let step: CGFloat = 8
            var x = cgRect.minX
            while x < cgRect.maxX {
                context.move(to: CGPoint(x: x, y: cgRect.minY))
                context.addLine(to: CGPoint(x: x + cgRect.height, y: cgRect.maxY))
                x += step
            }
            context.strokePath()
        case .dots:
            context.setFillColor(ink)
            let step: CGFloat = 8
            var y = cgRect.minY + step / 2
            while y < cgRect.maxY {
                var x = cgRect.minX + step / 2
                while x < cgRect.maxX {
                    context.fillEllipse(in: CGRect(x: x - 1.25, y: y - 1.25, width: 2.5, height: 2.5))
                    x += step
                }
                y += step
            }
        }
        context.restoreGState()
    }

    /// White over dark fills, black over light ones; both at the low alpha the
    /// classic charcoal stripes used, so the default mask renders exactly as before.
    private static func maskPatternInk(over fill: CodableColor) -> CGColor {
        let luminance = 0.2126 * fill.red + 0.7152 * fill.green + 0.0722 * fill.blue
        return luminance > 0.55 ? CGColor(gray: 0, alpha: 0.18) : NSColor.white.withAlphaComponent(0.16).cgColor
    }

    private static func drawHighlights(_ annotations: [ScreenshotAnnotation], in context: CGContext, imageHeight: CGFloat) {
        for annotation in annotations {
            guard case let .highlight(rect) = annotation.kind else { continue }
            drawing(annotation, in: context, imageHeight: imageHeight) {
                context.saveGState()
                context.setAlpha(annotation.style.opacity)
                context.setFillColor(annotation.style.fillColor?.cgColor ?? AnnotationStyle.highlight.strokeColor.cgColor)
                context.fill(coreGraphicsRect(rect, imageHeight: imageHeight))
                context.restoreGState()
            }
        }
    }

    private static func drawVectorAnnotations(_ annotations: [ScreenshotAnnotation], in context: CGContext, imageHeight: CGFloat) {
        for annotation in annotations {
            switch annotation.kind {
            case .freehand, .arrow, .rectangle:
                break
            default:
                continue
            }
            drawing(annotation, in: context, imageHeight: imageHeight) {
                drawVector(annotation, in: context, imageHeight: imageHeight)
            }
        }
    }

    private static func drawVector(_ annotation: ScreenshotAnnotation, in context: CGContext, imageHeight: CGFloat) {
        context.saveGState()
        context.setAlpha(annotation.style.opacity)
        context.setStrokeColor(annotation.style.strokeColor.cgColor)
        context.setLineWidth(max(annotation.style.lineWidth, 1))
        context.setLineCap(.round)
        context.setLineJoin(.round)

        switch annotation.kind {
        case let .freehand(points):
            guard let first = points.first else { break }
            if points.count == 1 {
                let point = flip(first, imageHeight: imageHeight)
                let diameter = max(annotation.style.lineWidth, 1)
                context.setFillColor(annotation.style.strokeColor.cgColor)
                context.fillEllipse(in: CGRect(x: point.x - diameter / 2, y: point.y - diameter / 2, width: diameter, height: diameter))
                break
            }
            context.move(to: flip(first, imageHeight: imageHeight))
            for point in points.dropFirst() {
                context.addLine(to: flip(point, imageHeight: imageHeight))
            }
            context.strokePath()
        case let .arrow(start, end):
            drawArrow(from: flip(start, imageHeight: imageHeight), to: flip(end, imageHeight: imageHeight), lineWidth: annotation.style.lineWidth, in: context)
        case let .rectangle(rect):
            if let fill = annotation.style.fillColor {
                context.setFillColor(fill.cgColor)
                context.fill(coreGraphicsRect(rect, imageHeight: imageHeight))
            }
            context.stroke(coreGraphicsRect(rect, imageHeight: imageHeight))
        default:
            break
        }

        context.restoreGState()
    }

    private static func drawTextAnnotations(_ annotations: [ScreenshotAnnotation], in context: CGContext, imageHeight: CGFloat) {
        for annotation in annotations {
            guard case let .text(text, origin, maxWidth) = annotation.kind else { continue }
            drawing(annotation, in: context, imageHeight: imageHeight) {
                let nsContext = NSGraphicsContext(cgContext: context, flipped: false)
                NSGraphicsContext.saveGraphicsState()
                NSGraphicsContext.current = nsContext
                let attr = attributedText(text, style: annotation.style)
                let textHeight = textDrawingHeight(for: attr, maxWidth: maxWidth, minimumHeight: annotation.style.fontSize * 1.4)
                let rect = CGRect(x: origin.x, y: imageHeight - origin.y - textHeight, width: maxWidth, height: textHeight)
                attr.draw(in: rect)
                NSGraphicsContext.restoreGraphicsState()
            }
        }
    }

    private static func attributedText(_ text: String, style: AnnotationStyle) -> NSAttributedString {
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byWordWrapping
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: style.fontSize, weight: .semibold),
            .foregroundColor: style.strokeColor.nsColor.withAlphaComponent(style.opacity),
            .paragraphStyle: paragraph,
        ]
        return NSAttributedString(string: text, attributes: attributes)
    }

    private static func textDrawingHeight(
        for attributedString: NSAttributedString,
        maxWidth: CGFloat,
        minimumHeight: CGFloat
    ) -> CGFloat {
        let measured = attributedString.boundingRect(
            with: CGSize(width: max(1, maxWidth), height: .greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading]
        )
        return max(ceil(measured.height) + 4, ceil(minimumHeight))
    }

    private static func drawArrow(from start: CGPoint, to end: CGPoint, lineWidth: CGFloat, in context: CGContext) {
        context.move(to: start)
        context.addLine(to: end)
        context.strokePath()

        let angle = atan2(end.y - start.y, end.x - start.x)
        let length = max(10, max(lineWidth, 1) * 3)
        let spread: CGFloat = .pi / 7
        let p1 = CGPoint(x: end.x - length * cos(angle - spread), y: end.y - length * sin(angle - spread))
        let p2 = CGPoint(x: end.x - length * cos(angle + spread), y: end.y - length * sin(angle + spread))
        context.move(to: end)
        context.addLine(to: p1)
        context.move(to: end)
        context.addLine(to: p2)
        context.strokePath()
    }

    private static func flip(_ point: CGPoint, imageHeight: CGFloat) -> CGPoint {
        CGPoint(x: point.x, y: imageHeight - point.y)
    }

    private static func coreGraphicsRect(_ rect: CGRect, imageHeight: CGFloat) -> CGRect {
        CGRect(x: rect.minX, y: imageHeight - rect.maxY, width: rect.width, height: rect.height).standardized
    }

    private static func scale(_ image: CGImage, by outputScale: CGFloat) throws -> CGImage {
        let width = max(1, Int(CGFloat(image.width) * outputScale))
        let height = max(1, Int(CGFloat(image.height) * outputScale))
        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            throw AnnotationRenderError.cannotCreateContext
        }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        guard let scaled = context.makeImage() else { throw AnnotationRenderError.cannotCreateImage }
        return scaled
    }
}
