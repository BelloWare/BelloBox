import CoreGraphics

/// Pixel comparisons have no UI state and can run on a worker task. Keep the decision
/// inputs together so a sample is always compared with one consistent frame snapshot.
enum ScrollSampleAnalyzer {
    enum Result {
        case firstFrame
        case sizeChanged
        case unchanged
        case content(Comparison)
    }

    struct Comparison {
        let settled: Bool
        let header: Int
        let footer: Int
        let previousFooter: Int
        let match: OverlapMatch?
        let reversed: Bool
        let difference: Double
    }

    static func analyze(
        _ sample: CGImage, first: CGImage?, previous: CGImage?, lastSample: CGImage?,
        frameCount: Int, lastAppendedFooter: Int, configuration: ScrollCaptureEngine.Configuration
    ) throws -> Result {
        try Task.checkCancellation()
        guard let previous, let first else { return .firstFrame }
        guard sample.width == previous.width, sample.height == previous.height else { return .sizeChanged }
        if ImageStitcher.appearsUnchanged(previous: previous, current: sample, threshold: configuration.changeThreshold) {
            return .unchanged
        }
        let settled = lastSample.map {
            ImageStitcher.appearsUnchanged(previous: $0, current: sample, threshold: configuration.settleThreshold)
        } ?? false
        try Task.checkCancellation()
        let header = configuration.stitch.removeRepeatedHeaderFooter
            ? ImageStitcher.stickyBandHeight(first: first, previous: previous, current: sample, detect: ImageStitcher.repeatedHeaderHeight) : 0
        let footer = configuration.stitch.removeRepeatedHeaderFooter
            ? ImageStitcher.stickyBandHeight(first: first, previous: previous, current: sample, detect: ImageStitcher.repeatedFooterHeight) : 0
        try Task.checkCancellation()
        let previousFooter = frameCount == 1 ? footer : lastAppendedFooter
        let match = ImageStitcher.bestOverlap(previous: previous, current: sample, config: configuration.stitch,
            skippingTopRows: header, skippingBottomRows: previousFooter)
        try Task.checkCancellation()
        let reversed = match == nil && ImageStitcher.bestOverlap(previous: sample, current: previous,
            config: configuration.stitch, skippingTopRows: header, skippingBottomRows: footer) != nil
        let difference = match == nil && !reversed
            ? ImageStitcher.meanAbsoluteDifference(previous: previous, current: sample) ?? 0 : 0
        try Task.checkCancellation()
        return .content(Comparison(settled: settled, header: header, footer: footer,
            previousFooter: previousFooter, match: match, reversed: reversed, difference: difference))
    }
}
