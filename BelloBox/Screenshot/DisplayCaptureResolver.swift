import CoreGraphics

struct DisplayCaptureCandidate: Equatable {
    var displayID: CGDirectDisplayID
    var frame: CGRect
}

enum DisplayCaptureResolver {
    enum CandidateSource: Equatable {
        case initial
        case refreshed
    }

    enum MatchPath: Equatable {
        case initialDisplayID
        case refreshedDisplayID
        case refreshedBounds
        case initialBounds
    }

    enum LegacyFallbackReason: Equatable {
        case noScreenCaptureKitDisplay
    }

    enum Resolution: Equatable {
        case screenCaptureKit(candidate: DisplayCaptureCandidate, source: CandidateSource, path: MatchPath)
        case legacyFallback(reason: LegacyFallbackReason)
        case noDisplayFound
    }

    static func resolve(
        requestedDisplayID: CGDirectDisplayID,
        requestedBounds: CGRect,
        initialCandidates: [DisplayCaptureCandidate],
        refreshedCandidates: [DisplayCaptureCandidate]?,
        legacyFallbackAvailable: Bool
    ) -> Resolution {
        if let candidate = initialCandidates.first(where: { $0.displayID == requestedDisplayID }) {
            return .screenCaptureKit(candidate: candidate, source: .initial, path: .initialDisplayID)
        }

        if let refreshedCandidates,
           let candidate = refreshedCandidates.first(where: { $0.displayID == requestedDisplayID }) {
            return .screenCaptureKit(candidate: candidate, source: .refreshed, path: .refreshedDisplayID)
        }

        if let refreshedCandidates,
           let candidate = boundsMatch(for: requestedBounds, in: refreshedCandidates) {
            return .screenCaptureKit(candidate: candidate, source: .refreshed, path: .refreshedBounds)
        }

        if let candidate = boundsMatch(for: requestedBounds, in: initialCandidates) {
            return .screenCaptureKit(candidate: candidate, source: .initial, path: .initialBounds)
        }

        return legacyFallbackAvailable
            ? .legacyFallback(reason: .noScreenCaptureKitDisplay)
            : .noDisplayFound
    }

    private static func boundsMatch(
        for requestedBounds: CGRect,
        in candidates: [DisplayCaptureCandidate]
    ) -> DisplayCaptureCandidate? {
        guard !requestedBounds.isNull, !requestedBounds.isEmpty else { return nil }
        return candidates.first { nearlyEqual($0.frame, requestedBounds, tolerance: 2) }
    }

    private static func nearlyEqual(_ lhs: CGRect, _ rhs: CGRect, tolerance: CGFloat) -> Bool {
        let lhs = lhs.standardized
        let rhs = rhs.standardized
        return abs(lhs.origin.x - rhs.origin.x) <= tolerance
            && abs(lhs.origin.y - rhs.origin.y) <= tolerance
            && abs(lhs.size.width - rhs.size.width) <= tolerance
            && abs(lhs.size.height - rhs.size.height) <= tolerance
    }
}
