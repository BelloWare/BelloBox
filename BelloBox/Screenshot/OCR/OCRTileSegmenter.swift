import CoreGraphics

struct OCRImageTile: Equatable {
    var image: CGImage
    var yOffset: Int
    var index: Int

    static func == (lhs: OCRImageTile, rhs: OCRImageTile) -> Bool {
        lhs.image.width == rhs.image.width
            && lhs.image.height == rhs.image.height
            && lhs.yOffset == rhs.yOffset
            && lhs.index == rhs.index
    }
}

enum OCRTileSegmenter {
    static func tiles(from image: CGImage, maxTileHeight: Int = 3200, overlap: Int = 160) -> [OCRImageTile] {
        guard image.height > maxTileHeight, maxTileHeight > overlap else {
            return [OCRImageTile(image: image, yOffset: 0, index: 0)]
        }
        var result: [OCRImageTile] = []
        var y = 0
        var index = 0
        while y < image.height {
            let height = min(maxTileHeight, image.height - y)
            if let crop = image.cropping(to: CGRect(x: 0, y: y, width: image.width, height: height)) {
                result.append(OCRImageTile(image: crop, yOffset: y, index: index))
            }
            if y + height >= image.height { break }
            y += maxTileHeight - overlap
            index += 1
        }
        return result
    }

    static func offset(_ regions: [OCRTextRegion], by tile: OCRImageTile) -> [OCRTextRegion] {
        regions.map { region in
            var copy = region
            if let rect = region.boundingBox?.rect {
                copy.boundingBox = CGRectCodable(rect.offsetBy(dx: 0, dy: CGFloat(tile.yOffset)))
            }
            copy.children = offset(region.children, by: tile)
            return copy
        }
    }

    static func deduplicateOverlapRegions(_ regions: [OCRTextRegion], intersectionThreshold: CGFloat = 0.72) -> [OCRTextRegion] {
        var kept: [OCRTextRegion] = []
        for region in regions.sortedByReadingOrder() {
            guard let rect = region.boundingBox?.rect else {
                kept.append(region)
                continue
            }
            let duplicate = kept.contains { existing in
                guard existing.text == region.text, let other = existing.boundingBox?.rect else { return false }
                let intersection = rect.intersection(other)
                guard !intersection.isNull, rect.area > 0 else { return false }
                return intersection.area / rect.area >= intersectionThreshold
            }
            if !duplicate { kept.append(region) }
        }
        return kept
    }
}

private extension CGRect {
    var area: CGFloat {
        guard !isNull, !isEmpty else { return 0 }
        return width * height
    }
}

