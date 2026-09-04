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

        // Sticky bars repeat on every frame: a header (browser chrome, toolbars) at the
        // top and/or a footer (status bar, input bar) at the bottom. Each frame's bars
        // are measured on that frame (a bar must repeat in the first and the previous
        // frame); both are skipped when matching, the header is cropped from every later
        // frame and the footer from every frame but the last, so the content hidden under
        // it is recovered from the next frame.
        let images = normalized.map(\.image)
        let headers = images.indices.map { index -> Int in
            guard config.removeRepeatedHeaderFooter, index > 0 else { return 0 }
            return stickyBandHeight(first: images[0], previous: images[index - 1], current: images[index], detect: repeatedHeaderHeight)
        }
        var footers = images.indices.map { index -> Int in
            guard config.removeRepeatedHeaderFooter, index > 0 else { return 0 }
            return stickyBandHeight(first: images[0], previous: images[index - 1], current: images[index], detect: repeatedFooterHeight)
        }
        if footers.count > 1 {
            footers[0] = footers[1]
        }

        for index in 1..<normalized.count {
            if Task.isCancelled { throw CancellationError() }
            let previous = normalized[index - 1].image
            let currentEntry = normalized[index]
            let current = currentEntry.image
            let header = headers[index]
            let previousFooter = footers[index - 1]
            let match = bestOverlap(
                previous: previous,
                current: current,
                config: config,
                skippingTopRows: header,
                skippingBottomRows: previousFooter
            )
            let overlap = match?.overlap ?? 0
            let confidence = match.map { 1 - $0.score } ?? 0
            if match == nil {
                warnings.append("Frame \(currentEntry.frameIndex + 1) did not have a confident overlap; it was appended without compaction.")
            } else if appearsUnchanged(previous: previous, current: current) {
                warnings.append("Frame \(currentEntry.frameIndex + 1) appears nearly unchanged from the previous frame.")
            } else if overlap > Int(CGFloat(current.height - header - footers[index]) * 0.88) {
                warnings.append("Frame \(currentEntry.frameIndex + 1) appears nearly unchanged from the previous frame.")
            }

            // The seam rows are drawn from the current frame rather than the previous one:
            // a bar at the bottom of the previous frame that was measured a little short,
            // or was too thin to detect at all, then never shows, and the document rows it
            // covered come from the frame that shows them.
            let slack = match == nil ? 0 : min(seamSlackRows, overlap)
            if match != nil, previousFooter + slack > 0 {
                placements[placements.count - 1].croppedBottom = previousFooter + slack
                y -= previousFooter + slack
            }
            let croppedTop = header + overlap - slack
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

    /// Finds how many rows of `current` (below its first `header` rows) repeat the bottom
    /// of `previous`. The coarse pass scans a downsampled copy in 4 px steps; the best
    /// candidate is then refined at full resolution before the confidence threshold is
    /// applied, so detailed content (text) is not rejected for a 1-2 px misalignment.
    /// Ties prefer the larger overlap, so blank seams never duplicate content.
    static func bestOverlap(
        previous: CGImage,
        current: CGImage,
        config: StitchConfig,
        skippingTopRows header: Int = 0,
        skippingBottomRows footer: Int = 0
    ) -> OverlapMatch? {
        guard let previousGray = GrayImage(image: previous, targetWidth: config.downsampleWidth),
              let currentGray = GrayImage(image: current, targetWidth: config.downsampleWidth)
        else { return nil }

        let header = max(0, min(header, current.height - 1))
        let footer = max(0, min(footer, previous.height - 1))
        let usableHeight = min(previous.height - footer, current.height - header)
        let maxOriginalOverlap = Int(CGFloat(usableHeight) * config.maxOverlapFraction)
        guard maxOriginalOverlap >= config.minOverlapPx else { return nil }
        let scale = CGFloat(previousGray.height) / CGFloat(previous.height)
        let scaledHeader = Int((CGFloat(header) * scale).rounded())
        let scaledFooter = Int((CGFloat(footer) * scale).rounded())
        let previousEnd = previousGray.height - scaledFooter

        var best: OverlapMatch?
        var scores: [Double] = []
        for overlap in stride(from: config.minOverlapPx, through: maxOriginalOverlap, by: 4) {
            let scaledOverlap = max(1, Int(CGFloat(overlap) * scale))
            guard scaledOverlap < previousEnd, scaledHeader + scaledOverlap <= currentGray.height else { continue }
            let score = previousGray.meanAbsoluteDifference(
                rowsEndingAt: previousEnd,
                count: scaledOverlap,
                of: currentGray,
                rowsStartingAt: scaledHeader,
                sideInset: 8
            )
            scores.append(score)
            let isBetter = best.map { candidate in
                score < candidate.score - 0.0001 || (abs(score - candidate.score) <= 0.0001 && overlap > candidate.overlap)
            } ?? true
            if isBetter {
                best = OverlapMatch(overlap: overlap, score: score)
            }
        }
        guard let best else { return nil }
        let refined = refineOverlap(previous: previous, current: current, around: best, header: header, footer: footer) ?? best
        guard refined.score <= config.scoreThreshold else { return nil }
        // A real overlap stands out from the other candidates; unrelated smooth content
        // can slip under the absolute threshold but scores like everything around it.
        if refined.score > 0.01, scores.count >= 5 {
            let median = scores.sorted()[scores.count / 2]
            guard refined.score <= median * 0.5 else { return nil }
        }
        return refined
    }

    /// Mean absolute difference (0...1) between two same-size frames, or nil when they
    /// cannot be compared.
    static func meanAbsoluteDifference(previous: CGImage, current: CGImage, downsampleWidth: Int = 420) -> Double? {
        guard previous.width == current.width, previous.height == current.height,
              let previousGray = GrayImage(image: previous, targetWidth: downsampleWidth),
              let currentGray = GrayImage(image: current, targetWidth: downsampleWidth)
        else { return nil }
        return previousGray.meanAbsoluteDifference(fullImageOf: currentGray, sideInset: 8)
    }

    /// The coarse search above steps 4 px at a downsampled resolution; this pass compares
    /// the candidate band at full resolution, one row at a time, so frames are placed at
    /// their exact offset and stitched output has no duplicated or missing rows. Ties keep
    /// the coarse candidate.
    private static func refineOverlap(previous: CGImage, current: CGImage, around coarse: OverlapMatch, header: Int, footer: Int) -> OverlapMatch? {
        let radius = 4
        let limit = min(previous.height - footer, current.height - header)
        let maxOverlap = min(coarse.overlap + radius, limit)
        let minOverlap = max(1, coarse.overlap - radius)
        guard maxOverlap >= minOverlap, maxOverlap > 0,
              let previousBand = previous.cropping(to: CGRect(x: 0, y: previous.height - footer - maxOverlap, width: previous.width, height: maxOverlap)),
              let currentBand = current.cropping(to: CGRect(x: 0, y: header, width: current.width, height: maxOverlap)),
              let previousGray = GrayImage(image: previousBand, targetWidth: previous.width),
              let currentGray = GrayImage(image: currentBand, targetWidth: current.width)
        else { return nil }
        func score(_ overlap: Int) -> Double {
            previousGray.meanAbsoluteDifference(bottomRows: overlap, of: currentGray, topRows: overlap, sideInset: 8)
        }
        var best = OverlapMatch(overlap: min(max(coarse.overlap, minOverlap), maxOverlap), score: score(min(max(coarse.overlap, minOverlap), maxOverlap)))
        for overlap in minOverlap...maxOverlap where overlap != best.overlap {
            let candidate = score(overlap)
            if candidate < best.score - 0.0005 {
                best = OverlapMatch(overlap: overlap, score: candidate)
            }
        }
        return best
    }

    /// A sticky band must be present in every frame: the smaller of the band shared with
    /// the first frame and the band shared with the previous frame, or 0.
    static func stickyBandHeight(
        first: CGImage,
        previous: CGImage,
        current: CGImage,
        detect: (CGImage, CGImage) -> Int?
    ) -> Int {
        guard let withFirst = detect(first, current), withFirst > 0 else { return 0 }
        guard let withPrevious = detect(previous, current), withPrevious > 0 else { return 0 }
        return min(withFirst, withPrevious)
    }

    /// Rows at a seam that are drawn from the later frame instead of the earlier one.
    static let seamSlackRows = 32

    /// Height of a band at the bottom of `current` that is identical to the bottom of
    /// `first` (a sticky footer such as a status or input bar), or nil when there is none.
    static func repeatedFooterHeight(first: CGImage, current: CGImage) -> Int? {
        repeatedBandHeight(first: first, current: current, atTop: false)
    }

    /// Height of a band at the top of `current` that is identical to the top of `first`
    /// (a sticky header), or nil when there is none.
    static func repeatedHeaderHeight(first: CGImage, current: CGImage) -> Int? {
        repeatedBandHeight(first: first, current: current, atTop: true)
    }

    /// Finds a band at the top or bottom of `current` that repeats the same band of
    /// `first`. An 8 px scan on a downsampled copy finds the rough size; the edge is then
    /// walked row by row at full resolution so bars of any height come out exact.
    private static func repeatedBandHeight(first: CGImage, current: CGImage, atTop: Bool) -> Int? {
        guard first.width == current.width, first.height > 0, current.height > 0,
              let a = GrayImage(image: first, targetWidth: min(first.width, 420)),
              let b = GrayImage(image: current, targetWidth: min(current.width, 420))
        else { return nil }
        let cap = min(min(first.height, current.height) / 3, 240)
        guard cap >= 24 else { return nil }
        let scale = CGFloat(a.height) / CGFloat(first.height)

        func coarseScore(_ rows: Int) -> Double {
            let scaled = max(1, Int((CGFloat(rows) * scale).rounded()))
            guard scaled <= a.height, scaled <= b.height else { return 1 }
            return atTop
                ? a.meanAbsoluteDifference(topRows: scaled, of: b, topRows: scaled, sideInset: 12)
                : a.meanAbsoluteDifference(rowsEndingAt: a.height, count: scaled, of: b, rowsStartingAt: b.height - scaled, sideInset: 12)
        }
        var coarse = 0
        for rows in stride(from: 24, through: cap, by: 8) where coarseScore(rows) < 0.012 {
            coarse = rows
        }
        guard coarse > 0 else { return nil }

        // Refine the edge one row at a time at full resolution.
        let bandLimit = min(coarse + 8, cap)
        let firstBand = atTop
            ? CGRect(x: 0, y: 0, width: first.width, height: bandLimit)
            : CGRect(x: 0, y: first.height - bandLimit, width: first.width, height: bandLimit)
        let currentBand = atTop
            ? CGRect(x: 0, y: 0, width: current.width, height: bandLimit)
            : CGRect(x: 0, y: current.height - bandLimit, width: current.width, height: bandLimit)
        guard let fa = first.cropping(to: firstBand).flatMap({ GrayImage(image: $0, targetWidth: first.width) }),
              let fb = current.cropping(to: currentBand).flatMap({ GrayImage(image: $0, targetWidth: current.width) })
        else { return coarse }
        // Row `depth` counted from the band's outer edge (top edge for headers, bottom edge for footers).
        func rowsMatch(depth: Int) -> Bool {
            let ya = atTop ? depth : fa.height - 1 - depth
            let yb = atTop ? depth : fb.height - 1 - depth
            return fa.rowDifference(ya, other: fb, otherY: yb, sideInset: 12) < 0.02
        }
        var rows = min(coarse, bandLimit)
        while rows > 0, !rowsMatch(depth: rows - 1) { rows -= 1 }
        while rows < bandLimit, rowsMatch(depth: rows) { rows += 1 }
        guard rows >= 8 else { return nil }
        return rows
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

    /// Compares this image's bottom `rows` with `rows` of `other` starting at `start`.
    func meanAbsoluteDifference(bottomRows rows: Int, of other: GrayImage, rowsStartingAt start: Int, sideInset: Int) -> Double {
        meanAbsoluteDifference(
            firstY: height - rows,
            second: other,
            secondY: start,
            rows: rows,
            sideInset: sideInset
        )
    }

    /// Compares this image's `count` rows ending at `end` with `count` rows of `other`
    /// starting at `start`.
    func meanAbsoluteDifference(rowsEndingAt end: Int, count: Int, of other: GrayImage, rowsStartingAt start: Int, sideInset: Int) -> Double {
        meanAbsoluteDifference(
            firstY: end - count,
            second: other,
            secondY: start,
            rows: count,
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

    /// Difference between one row of this image and one row of `other`.
    func rowDifference(_ y: Int, other: GrayImage, otherY: Int, sideInset: Int) -> Double {
        meanAbsoluteDifference(firstY: y, second: other, secondY: otherY, rows: 1, sideInset: sideInset)
    }

    /// Mean absolute deviation from the mean over a band of rows (0 = uniform).
    func contrast(rowsFrom start: Int, count: Int, sideInset: Int) -> Double {
        let inset = min(sideInset, width / 4)
        let startX = inset
        let endX = max(startX + 1, width - inset)
        let rows = max(0, start)..<min(height, start + count)
        var total = 0.0
        var samples = 0
        for y in rows {
            for x in startX..<endX {
                total += Double(pixels[y * width + x])
                samples += 1
            }
        }
        guard samples > 0 else { return 0 }
        let mean = total / Double(samples)
        var deviation = 0.0
        for y in rows {
            for x in startX..<endX {
                deviation += abs(Double(pixels[y * width + x]) - mean)
            }
        }
        return deviation / Double(samples) / 255.0
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
