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
        let topToBottom = sorted { lhs, rhs in
            let left = lhs.boundingBox?.rect ?? .zero
            let right = rhs.boundingBox?.rect ?? .zero
            if left.minY == right.minY {
                return left.minX < right.minX
            }
            return left.minY < right.minY
        }

        var lines: [OCRReadingLine] = []
        for region in topToBottom {
            let rect = region.boundingBox?.rect ?? .zero
            if let index = lines.firstIndex(where: { $0.contains(rect) }) {
                lines[index].append(region, rect: rect)
            } else {
                lines.append(OCRReadingLine(region: region, rect: rect))
            }
        }

        return lines
            .sorted { $0.minY < $1.minY }
            .flatMap { line in
                line.regions.sorted { lhs, rhs in
                    let left = lhs.boundingBox?.rect ?? .zero
                    let right = rhs.boundingBox?.rect ?? .zero
                    if left.minX == right.minX {
                        return left.minY < right.minY
                    }
                    return left.minX < right.minX
                }
            }
    }
}

private struct OCRReadingLine {
    private(set) var regions: [OCRTextRegion]
    private var midYTotal: CGFloat
    private var heightTotal: CGFloat

    init(region: OCRTextRegion, rect: CGRect) {
        self.regions = [region]
        self.midYTotal = rect.midY
        self.heightTotal = rect.height
    }

    var minY: CGFloat {
        regions.map { $0.boundingBox?.rect.minY ?? 0 }.min() ?? 0
    }

    private var midY: CGFloat {
        guard !regions.isEmpty else { return 0 }
        return midYTotal / CGFloat(regions.count)
    }

    private var averageHeight: CGFloat {
        guard !regions.isEmpty else { return 0 }
        return heightTotal / CGFloat(regions.count)
    }

    func contains(_ rect: CGRect) -> Bool {
        let tolerance = Swift.max(6, Swift.min(averageHeight, rect.height) * 0.45)
        return abs(midY - rect.midY) <= tolerance
    }

    mutating func append(_ region: OCRTextRegion, rect: CGRect) {
        regions.append(region)
        midYTotal += rect.midY
        heightTotal += rect.height
    }
}
