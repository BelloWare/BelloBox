import AppKit
import CoreGraphics

enum StitchError: LocalizedError, Equatable {
    case noFrames
    case cannotRender
    case outputTooTall(Int)

    var errorDescription: String? {
        switch self {
        case .noFrames:
            return "No frames were captured for scrolling screenshot."
        case .cannotRender:
            return "Could not stitch the captured frames."
        case let .outputTooTall(height):
            return "The stitched screenshot would be too tall (\(height) px)."
        }
    }
}

enum ImageStitcher {
    static func stitch(_ frames: [CGImage], config: StitchConfig = .default) throws -> StitchResult {
        if Task.isCancelled { throw CancellationError() }
        let orderedFrames = orderedFramesForStitching(frames, direction: config.direction)
        guard let firstEntry = orderedFrames.first else { throw StitchError.noFrames }
        let first = firstEntry.image
        var normalized: [(frameIndex: Int, image: CGImage)] = []
        normalized.reserveCapacity(frames.count)
        for entry in orderedFrames {
            if Task.isCancelled { throw CancellationError() }
            let image = entry.image.width == first.width ? entry.image : try resize(entry.image, width: first.width)
            normalized.append((frameIndex: entry.frameIndex, image: image))
        }

        var placements = [FramePlacement(frameIndex: firstEntry.frameIndex, y: 0, overlapWithPrevious: 0, confidence: 1, croppedTop: 0, croppedBottom: 0)]
        var y = first.height
        var warnings: [String] = []

        for index in 1..<normalized.count {
            if Task.isCancelled { throw CancellationError() }
            let previous = normalized[index - 1].image
            let currentEntry = normalized[index]
            let current = currentEntry.image
            let match = bestOverlap(previous: previous, current: current, config: config)
            let overlap = match?.overlap ?? 0
            let confidence = match.map { 1 - $0.score } ?? 0
            if match == nil {
                warnings.append("Frame \(currentEntry.frameIndex + 1) did not have a confident overlap; it was appended without compaction.")
            } else if appearsUnchanged(previous: previous, current: current) {
                warnings.append("Frame \(currentEntry.frameIndex + 1) appears nearly unchanged from the previous frame.")
            } else if overlap > Int(CGFloat(current.height) * 0.88) {
                warnings.append("Frame \(currentEntry.frameIndex + 1) appears nearly unchanged from the previous frame.")
            }

            var croppedTop = overlap
            if config.removeRepeatedHeaderFooter, let header = repeatedHeaderHeight(first: normalized[0].image, current: current) {
                croppedTop = max(croppedTop, header)
            }
            placements.append(FramePlacement(
                frameIndex: currentEntry.frameIndex,
                y: y - croppedTop,
                overlapWithPrevious: overlap,
                confidence: confidence,
                croppedTop: croppedTop,
                croppedBottom: 0
            ))
            y += current.height - croppedTop
        }

        guard y <= config.maxOutputHeightPx else { throw StitchError.outputTooTall(y) }
        guard let context = CGContext(
            data: nil,
            width: first.width,
            height: y,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            throw StitchError.cannotRender
        }

        for (placementOrder, placement) in placements.enumerated() {
            if Task.isCancelled { throw CancellationError() }
            let image = normalized[placementOrder].image
            let cropTop = placement.croppedTop
            let cropBottom = placement.croppedBottom
            let cropHeight = image.height - cropTop - cropBottom
            guard cropHeight > 0,
                  let cropped = image.cropping(to: CGRect(x: 0, y: cropTop, width: image.width, height: cropHeight))
            else { continue }
            let drawY = y - placement.y - cropTop - cropHeight
            context.draw(cropped, in: CGRect(x: 0, y: drawY, width: image.width, height: cropHeight))
        }

        guard let image = context.makeImage() else { throw StitchError.cannotRender }
        return StitchResult(image: image, placements: placements, warnings: warnings)
    }

    private static func orderedFramesForStitching(
        _ frames: [CGImage],
        direction: ScrollDirection
    ) -> [(frameIndex: Int, image: CGImage)] {
        let indexed = frames.enumerated().map { (frameIndex: $0.offset, image: $0.element) }
        switch direction {
        case .down:
            return indexed
        case .up:
            return Array(indexed.reversed())
        }
    }

    static func appearsUnchanged(previous: CGImage, current: CGImage, downsampleWidth: Int = 420, threshold: Double = 0.015) -> Bool {
        guard previous.width == current.width, previous.height == current.height,
              let previousGray = GrayImage(image: previous, targetWidth: downsampleWidth),
              let currentGray = GrayImage(image: current, targetWidth: downsampleWidth)
        else { return false }
        return previousGray.meanAbsoluteDifference(fullImageOf: currentGray, sideInset: 8) <= threshold
    }

    static func bestOverlap(previous: CGImage, current: CGImage, config: StitchConfig) -> OverlapMatch? {
        guard let previousGray = GrayImage(image: previous, targetWidth: config.downsampleWidth),
              let currentGray = GrayImage(image: current, targetWidth: config.downsampleWidth)
        else { return nil }

        let maxOriginalOverlap = Int(CGFloat(min(previous.height, current.height)) * config.maxOverlapFraction)
        guard maxOriginalOverlap >= config.minOverlapPx else { return nil }
        let scale = CGFloat(previousGray.height) / CGFloat(previous.height)

        var best: OverlapMatch?
        for overlap in stride(from: config.minOverlapPx, through: maxOriginalOverlap, by: 4) {
            let scaledOverlap = max(1, Int(CGFloat(overlap) * scale))
            guard scaledOverlap < previousGray.height, scaledOverlap < currentGray.height else { continue }
            let score = previousGray.meanAbsoluteDifference(
                bottomRows: scaledOverlap,
                of: currentGray,
                topRows: scaledOverlap,
                sideInset: 8
            )
            if best.map({ score < $0.score }) ?? true {
                best = OverlapMatch(overlap: overlap, score: score)
            }
        }
        guard let best, best.score <= config.scoreThreshold else { return nil }
        return best
    }

    private static func repeatedHeaderHeight(first: CGImage, current: CGImage) -> Int? {
        guard first.width == current.width,
              let a = GrayImage(image: first, targetWidth: min(first.width, 420)),
              let b = GrayImage(image: current, targetWidth: min(current.width, 420))
        else { return nil }
        let maxBand = min(96, min(first.height, current.height) / 5)
        var bestHeight = 0
        for height in stride(from: 24, through: maxBand, by: 8) {
            let scaled = max(1, Int(CGFloat(height) * CGFloat(a.height) / CGFloat(first.height)))
            let score = a.meanAbsoluteDifference(topRows: scaled, of: b, topRows: scaled, sideInset: 12)
            if score < 0.012 { bestHeight = height }
        }
        return bestHeight > 0 ? bestHeight : nil
    }

    private static func resize(_ image: CGImage, width: Int) throws -> CGImage {
        let scale = CGFloat(width) / CGFloat(image.width)
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
            throw StitchError.cannotRender
        }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        guard let output = context.makeImage() else { throw StitchError.cannotRender }
        return output
    }
}

struct OverlapMatch: Equatable {
    var overlap: Int
    var score: Double
}

private struct GrayImage {
    var width: Int
    var height: Int
    var pixels: [UInt8]

    init?(image: CGImage, targetWidth: Int) {
        let width = max(1, min(targetWidth, image.width))
        let scale = CGFloat(width) / CGFloat(image.width)
        let height = max(1, Int(CGFloat(image.height) * scale))
        var data = [UInt8](repeating: 0, count: width * height * 4)
        guard let context = CGContext(
            data: &data,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return nil }
        context.interpolationQuality = .low
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))

        self.width = width
        self.height = height
        pixels = [UInt8](repeating: 0, count: width * height)
        for index in 0..<(width * height) {
            let base = index * 4
            let red = Double(data[base])
            let green = Double(data[base + 1])
            let blue = Double(data[base + 2])
            pixels[index] = UInt8(max(0, min(255, red * 0.299 + green * 0.587 + blue * 0.114)))
        }
    }

    func meanAbsoluteDifference(bottomRows rows: Int, of other: GrayImage, topRows: Int, sideInset: Int) -> Double {
        meanAbsoluteDifference(
            firstY: height - rows,
            second: other,
            secondY: 0,
            rows: min(rows, topRows),
            sideInset: sideInset
        )
    }

    func meanAbsoluteDifference(topRows rows: Int, of other: GrayImage, topRows: Int, sideInset: Int) -> Double {
        meanAbsoluteDifference(
            firstY: 0,
            second: other,
            secondY: 0,
            rows: min(rows, topRows),
            sideInset: sideInset
        )
    }

    func meanAbsoluteDifference(fullImageOf other: GrayImage, sideInset: Int) -> Double {
        meanAbsoluteDifference(
            firstY: 0,
            second: other,
            secondY: 0,
            rows: min(height, other.height),
            sideInset: sideInset
        )
    }

    private func meanAbsoluteDifference(firstY: Int, second: GrayImage, secondY: Int, rows: Int, sideInset: Int) -> Double {
        let compareWidth = min(width, second.width)
        let inset = min(sideInset, compareWidth / 4)
        let startX = inset
        let endX = max(startX + 1, compareWidth - inset)
        var total = 0.0
        var count = 0
        for row in 0..<rows {
            let y1 = firstY + row
            let y2 = secondY + row
            guard y1 >= 0, y1 < height, y2 >= 0, y2 < second.height else { continue }
            for x in startX..<endX {
                total += abs(Double(pixels[y1 * width + x]) - Double(second.pixels[y2 * second.width + x]))
                count += 1
            }
        }
        guard count > 0 else { return 1 }
        return total / Double(count) / 255.0
    }
}
