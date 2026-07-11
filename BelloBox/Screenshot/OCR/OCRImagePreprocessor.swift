import AppKit
import CryptoKit
import Foundation

struct PreparedOCRImage {
    var image: CGImage
    var encodedData: Data?
    var mimeType: String?
    var pixelSize: CGSize
    var digest: String
    var appliedCrop: CGRect?
    var appliedRedactions: [CGRect]
    var warnings: [String]

    static func == (lhs: PreparedOCRImage, rhs: PreparedOCRImage) -> Bool {
        lhs.image.width == rhs.image.width
            && lhs.image.height == rhs.image.height
            && lhs.encodedData == rhs.encodedData
            && lhs.mimeType == rhs.mimeType
            && lhs.pixelSize == rhs.pixelSize
            && lhs.digest == rhs.digest
            && lhs.appliedCrop == rhs.appliedCrop
            && lhs.appliedRedactions == rhs.appliedRedactions
            && lhs.warnings == rhs.warnings
    }
}

enum OCRImagePreprocessor {
    static func prepare(document: ScreenshotDocument, options: OCROptions, forExternalUpload: Bool) throws -> PreparedOCRImage {
        let cropRect = effectiveCropRect(for: document, target: options.target)
        let image = try AnnotationRenderer.renderForOCR(
            document,
            target: options.target,
            includeDecorativeAnnotations: false
        )
        var warnings: [String] = []
        var outputImage = image

        if forExternalUpload {
            let longEdge = max(outputImage.width, outputImage.height)
            if longEdge > options.maxUploadLongEdge, options.maxUploadLongEdge > 0 {
                outputImage = try downscale(outputImage, maxLongEdge: options.maxUploadLongEdge)
                warnings.append("Image was downscaled before LLM OCR upload.")
            }
        }
        let redactions = appliedRedactions(
            from: document.annotations,
            cropRect: cropRect,
            renderedSize: CGSize(width: image.width, height: image.height),
            outputSize: CGSize(width: outputImage.width, height: outputImage.height)
        )

        let encoded = forExternalUpload ? try ImageExportService.pngData(from: outputImage) : nil
        let digest = try encoded.map(sha256) ?? imageDigest(outputImage)
        return PreparedOCRImage(
            image: outputImage,
            encodedData: encoded,
            mimeType: encoded == nil ? nil : "image/png",
            pixelSize: CGSize(width: outputImage.width, height: outputImage.height),
            digest: digest,
            appliedCrop: cropRect == fullImageRect(for: document) ? nil : cropRect,
            appliedRedactions: redactions,
            warnings: warnings
        )
    }

    static func sha256(_ data: Data) -> String {
        hexEncode(SHA256.hash(data: data))
    }

    private static let hexDigits = Array("0123456789abcdef".utf8)

    private static func hexEncode<S: Sequence>(_ bytes: S) -> String where S.Element == UInt8 {
        var output: [UInt8] = []
        output.reserveCapacity(bytes.underestimatedCount * 2)
        for byte in bytes {
            output.append(hexDigits[Int(byte >> 4)])
            output.append(hexDigits[Int(byte & 0x0f)])
        }
        return String(decoding: output, as: UTF8.self)
    }

    private static func imageDigest(_ image: CGImage) throws -> String {
        let bytesPerPixel = 4
        let bytesPerRow = max(1, image.width) * bytesPerPixel
        var pixels = Data(count: bytesPerRow * max(1, image.height))
        let rendered = pixels.withUnsafeMutableBytes { pointer -> Bool in
            guard let baseAddress = pointer.baseAddress,
                  let context = CGContext(
                    data: baseAddress,
                    width: image.width,
                    height: image.height,
                    bitsPerComponent: 8,
                    bytesPerRow: bytesPerRow,
                    space: CGColorSpaceCreateDeviceRGB(),
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue
                  )
            else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
            return true
        }
        guard rendered else { throw AnnotationRenderError.cannotCreateContext }
        var digestInput = Data("\(image.width)x\(image.height)\n".utf8)
        digestInput.append(pixels)
        return sha256(digestInput)
    }

    private static func effectiveCropRect(for document: ScreenshotDocument, target: OCRTarget) -> CGRect {
        let full = fullImageRect(for: document)
        switch target {
        case .fullImage:
            return (document.cropRect ?? full).intersection(full).integral
        case let .crop(rect):
            return rect.rect.intersection(full).integral
        case let .visibleAfterRedactions(crop):
            return (crop?.rect ?? document.cropRect ?? full).intersection(full).integral
        }
    }

    private static func fullImageRect(for document: ScreenshotDocument) -> CGRect {
        CGRect(origin: .zero, size: document.imageSize).integral
    }

    private static func appliedRedactions(
        from annotations: [ScreenshotAnnotation],
        cropRect: CGRect,
        renderedSize: CGSize,
        outputSize: CGSize
    ) -> [CGRect] {
        // The redaction pixels are enforced by AnnotationRenderer; these rects
        // report the same regions in the final prepared image coordinate space.
        annotations.compactMap { annotation -> CGRect? in
            guard case let .blur(rect) = annotation.kind else { return nil }
            let clipped = rect.intersection(cropRect)
            guard clipped.width > 0, clipped.height > 0 else { return nil }
            let shifted = clipped.offsetBy(dx: -cropRect.minX, dy: -cropRect.minY)
            return scaledPixelRect(shifted, from: renderedSize, to: outputSize)
        }
    }

    private static func scaledPixelRect(_ rect: CGRect, from sourceSize: CGSize, to outputSize: CGSize) -> CGRect {
        guard sourceSize.width > 0, sourceSize.height > 0 else { return .zero }
        let scaleX = outputSize.width / sourceSize.width
        let scaleY = outputSize.height / sourceSize.height
        let minX = floor(rect.minX * scaleX)
        let minY = floor(rect.minY * scaleY)
        let maxX = ceil(rect.maxX * scaleX)
        let maxY = ceil(rect.maxY * scaleY)
        return CGRect(
            x: minX,
            y: minY,
            width: max(0, min(outputSize.width, maxX) - max(0, minX)),
            height: max(0, min(outputSize.height, maxY) - max(0, minY))
        )
    }

    private static func downscale(_ image: CGImage, maxLongEdge: Int) throws -> CGImage {
        let longEdge = max(image.width, image.height)
        guard longEdge > maxLongEdge else { return image }
        let scale = CGFloat(maxLongEdge) / CGFloat(longEdge)
        let width = max(1, Int(CGFloat(image.width) * scale))
        let height = max(1, Int(CGFloat(image.height) * scale))
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
