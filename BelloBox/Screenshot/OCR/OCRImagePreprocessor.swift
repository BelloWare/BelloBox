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
        let image = try AnnotationRenderer.renderForOCR(
            document,
            target: options.target,
            includeDecorativeAnnotations: false
        )
        let redactions = document.annotations.compactMap { annotation -> CGRect? in
            if case let .blur(rect) = annotation.kind { return rect }
            return nil
        }
        var warnings: [String] = []
        var outputImage = image

        if forExternalUpload {
            let longEdge = max(outputImage.width, outputImage.height)
            if longEdge > options.maxUploadLongEdge, options.maxUploadLongEdge > 0 {
                outputImage = try downscale(outputImage, maxLongEdge: options.maxUploadLongEdge)
                warnings.append("Image was downscaled before LLM OCR upload.")
            }
        }

        let encoded = try ImageExportService.pngData(from: outputImage)
        let digest = sha256(encoded)
        return PreparedOCRImage(
            image: outputImage,
            encodedData: forExternalUpload ? encoded : nil,
            mimeType: forExternalUpload ? "image/png" : nil,
            pixelSize: CGSize(width: outputImage.width, height: outputImage.height),
            digest: digest,
            appliedCrop: cropRect(for: document, target: options.target),
            appliedRedactions: redactions,
            warnings: warnings
        )
    }

    static func sha256(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    private static func cropRect(for document: ScreenshotDocument, target: OCRTarget) -> CGRect? {
        switch target {
        case .fullImage:
            return document.cropRect
        case let .crop(rect):
            return rect.rect
        case let .visibleAfterRedactions(crop):
            return crop?.rect ?? document.cropRect
        }
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

