import CoreGraphics
import Vision

enum OCRBoundingBoxConverter {
    static func imagePixelRect(fromVisionNormalizedBox box: CGRect, imageSize: CGSize) -> CGRect {
        CGRect(
            x: box.minX * imageSize.width,
            y: (1 - box.maxY) * imageSize.height,
            width: box.width * imageSize.width,
            height: box.height * imageSize.height
        ).standardized
    }

    static func regions(from observations: [VNRecognizedTextObservation], imageSize: CGSize) -> [OCRTextRegion] {
        observations.compactMap { observation in
            guard let text = observation.topCandidates(1).first else { return nil }
            let rect = imagePixelRect(fromVisionNormalizedBox: observation.boundingBox, imageSize: imageSize)
            return OCRTextRegion(
                kind: .line,
                text: text.string,
                confidence: text.confidence,
                boundingBox: CGRectCodable(rect),
                children: []
            )
        }
        .sortedByReadingOrder()
    }
}

extension Array where Element == OCRTextRegion {
    func sortedByReadingOrder() -> [OCRTextRegion] {
        sorted { lhs, rhs in
            let l = lhs.boundingBox?.rect ?? .zero
            let r = rhs.boundingBox?.rect ?? .zero
            let tolerance = Swift.max(6, Swift.min(l.height, r.height) * 0.45)
            if abs(l.midY - r.midY) <= tolerance {
                return l.minX < r.minX
            }
            return l.minY < r.minY
        }
    }
}
