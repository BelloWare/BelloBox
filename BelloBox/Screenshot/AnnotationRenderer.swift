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
        applyRedactions(annotations, in: context, imageHeight: CGFloat(height))

        if includeDecorativeAnnotations {
            drawHighlights(annotations, in: context, imageHeight: CGFloat(height))
            drawVectorAnnotations(annotations, in: context, imageHeight: CGFloat(height))
            drawTextAnnotations(annotations, in: context, imageHeight: CGFloat(height))
        }

        guard let image = context.makeImage() else { throw AnnotationRenderError.cannotCreateImage }
        if let outputScale, outputScale > 0, outputScale != 1 {
            return try scale(image, by: outputScale)
        }
        return image
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
        return annotations.compactMap { annotation in
            var copy = annotation
            switch annotation.kind {
            case let .freehand(points):
                copy.kind = .freehand(points: points.map { CGPoint(x: $0.x - cropRect.minX, y: $0.y - cropRect.minY) })
            case let .arrow(start, end):
                copy.kind = .arrow(
                    start: CGPoint(x: start.x - cropRect.minX, y: start.y - cropRect.minY),
                    end: CGPoint(x: end.x - cropRect.minX, y: end.y - cropRect.minY)
                )
            case let .rectangle(rect):
                copy.kind = .rectangle(rect.offsetBy(dx: -cropRect.minX, dy: -cropRect.minY))
            case let .highlight(rect):
                copy.kind = .highlight(rect.offsetBy(dx: -cropRect.minX, dy: -cropRect.minY))
            case let .text(text, origin, maxWidth):
                copy.kind = .text(text, origin: CGPoint(x: origin.x - cropRect.minX, y: origin.y - cropRect.minY), maxWidth: maxWidth)
            case let .blur(rect):
                copy.kind = .blur(rect.offsetBy(dx: -cropRect.minX, dy: -cropRect.minY))
            }
            return copy
        }
    }

    private static func applyRedactions(_ annotations: [ScreenshotAnnotation], in context: CGContext, imageHeight: CGFloat) {
        for annotation in annotations {
            guard case let .blur(rect) = annotation.kind else { continue }
            let cgRect = coreGraphicsRect(rect, imageHeight: imageHeight)
            context.saveGState()
            context.setFillColor(annotation.style.fillColor?.cgColor ?? AnnotationStyle.redaction.fillColor!.cgColor)
            context.fill(cgRect)
            context.setStrokeColor(NSColor.white.withAlphaComponent(0.16).cgColor)
            context.setLineWidth(1)
            let step: CGFloat = 8
            var x = cgRect.minX
            while x < cgRect.maxX {
                context.move(to: CGPoint(x: x, y: cgRect.minY))
                context.addLine(to: CGPoint(x: x + cgRect.height, y: cgRect.maxY))
                x += step
            }
            context.strokePath()
            context.restoreGState()
        }
    }

    private static func drawHighlights(_ annotations: [ScreenshotAnnotation], in context: CGContext, imageHeight: CGFloat) {
        for annotation in annotations {
            guard case let .highlight(rect) = annotation.kind else { continue }
            context.saveGState()
            context.setAlpha(annotation.style.opacity)
            context.setFillColor(annotation.style.fillColor?.cgColor ?? AnnotationStyle.highlight.strokeColor.cgColor)
            context.fill(coreGraphicsRect(rect, imageHeight: imageHeight))
            context.restoreGState()
        }
    }

    private static func drawVectorAnnotations(_ annotations: [ScreenshotAnnotation], in context: CGContext, imageHeight: CGFloat) {
        for annotation in annotations {
            context.saveGState()
            context.setAlpha(annotation.style.opacity)
            context.setStrokeColor(annotation.style.strokeColor.cgColor)
            context.setLineWidth(max(annotation.style.lineWidth, 1))
            context.setLineCap(.round)
            context.setLineJoin(.round)

            switch annotation.kind {
            case let .freehand(points):
                guard let first = points.first else { break }
                context.move(to: flip(first, imageHeight: imageHeight))
                for point in points.dropFirst() {
                    context.addLine(to: flip(point, imageHeight: imageHeight))
                }
                context.strokePath()
            case let .arrow(start, end):
                drawArrow(from: flip(start, imageHeight: imageHeight), to: flip(end, imageHeight: imageHeight), in: context)
            case let .rectangle(rect):
                context.stroke(coreGraphicsRect(rect, imageHeight: imageHeight))
            default:
                break
            }

            context.restoreGState()
        }
    }

    private static func drawTextAnnotations(_ annotations: [ScreenshotAnnotation], in context: CGContext, imageHeight: CGFloat) {
        let nsContext = NSGraphicsContext(cgContext: context, flipped: false)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = nsContext
        for annotation in annotations {
            guard case let .text(text, origin, maxWidth) = annotation.kind else { continue }
            let paragraph = NSMutableParagraphStyle()
            paragraph.lineBreakMode = .byWordWrapping
            let attributes: [NSAttributedString.Key: Any] = [
                .font: NSFont.systemFont(ofSize: annotation.style.fontSize, weight: .semibold),
                .foregroundColor: annotation.style.strokeColor.nsColor.withAlphaComponent(annotation.style.opacity),
                .paragraphStyle: paragraph,
            ]
            let attr = NSAttributedString(string: text, attributes: attributes)
            let rect = CGRect(
                x: origin.x,
                y: imageHeight - origin.y - annotation.style.fontSize * 1.3,
                width: maxWidth,
                height: max(annotation.style.fontSize * 2.4, 44)
            )
            attr.draw(in: rect)
        }
        NSGraphicsContext.restoreGraphicsState()
    }

    private static func drawArrow(from start: CGPoint, to end: CGPoint, in context: CGContext) {
        context.move(to: start)
        context.addLine(to: end)
        context.strokePath()

        let angle = atan2(end.y - start.y, end.x - start.x)
        let length: CGFloat = 14
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

