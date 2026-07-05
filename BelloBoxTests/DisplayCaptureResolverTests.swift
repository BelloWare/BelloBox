import CoreGraphics
import XCTest
@testable import BelloBox

final class DisplayCaptureResolverTests: XCTestCase {
    func testIDHitUsesInitialScreenCaptureKitDisplay() {
        let candidate = DisplayCaptureCandidate(displayID: 42, frame: CGRect(x: 0, y: 0, width: 300, height: 200))

        let resolution = DisplayCaptureResolver.resolve(
            requestedDisplayID: 42,
            requestedBounds: candidate.frame,
            initialCandidates: [candidate],
            refreshedCandidates: nil,
            legacyFallbackAvailable: true
        )

        XCTAssertEqual(resolution, .screenCaptureKit(candidate: candidate, source: .initial, path: .initialDisplayID))
    }

    func testStaleIDRefreshHitUsesRefreshedDisplayList() {
        let refreshed = DisplayCaptureCandidate(displayID: 42, frame: CGRect(x: 0, y: 0, width: 300, height: 200))

        let resolution = DisplayCaptureResolver.resolve(
            requestedDisplayID: 42,
            requestedBounds: refreshed.frame,
            initialCandidates: [DisplayCaptureCandidate(displayID: 7, frame: refreshed.frame)],
            refreshedCandidates: [refreshed],
            legacyFallbackAvailable: true
        )

        XCTAssertEqual(resolution, .screenCaptureKit(candidate: refreshed, source: .refreshed, path: .refreshedDisplayID))
    }

    func testBoundsMatchUsesRefreshedDisplayWhenIDChanged() {
        let refreshed = DisplayCaptureCandidate(displayID: 77, frame: CGRect(x: -320, y: 0, width: 320, height: 220))

        let resolution = DisplayCaptureResolver.resolve(
            requestedDisplayID: 42,
            requestedBounds: CGRect(x: -321, y: 1, width: 320, height: 220),
            initialCandidates: [DisplayCaptureCandidate(displayID: 7, frame: CGRect(x: 0, y: 0, width: 300, height: 200))],
            refreshedCandidates: [refreshed],
            legacyFallbackAvailable: true
        )

        XCTAssertEqual(resolution, .screenCaptureKit(candidate: refreshed, source: .refreshed, path: .refreshedBounds))
    }

    func testLegacyFallbackEngagesWhenScreenCaptureKitHasNoMatchingDisplay() {
        let resolution = DisplayCaptureResolver.resolve(
            requestedDisplayID: 42,
            requestedBounds: CGRect(x: 1000, y: 0, width: 300, height: 200),
            initialCandidates: [DisplayCaptureCandidate(displayID: 7, frame: CGRect(x: 0, y: 0, width: 300, height: 200))],
            refreshedCandidates: [DisplayCaptureCandidate(displayID: 8, frame: CGRect(x: 300, y: 0, width: 300, height: 200))],
            legacyFallbackAvailable: true
        )

        XCTAssertEqual(resolution, .legacyFallback(reason: .noScreenCaptureKitDisplay))
    }

    func testNoDisplayFoundOnlyWhenScreenCaptureKitAndLegacyAreUnavailable() {
        let resolution = DisplayCaptureResolver.resolve(
            requestedDisplayID: 42,
            requestedBounds: CGRect(x: 1000, y: 0, width: 300, height: 200),
            initialCandidates: [],
            refreshedCandidates: [],
            legacyFallbackAvailable: false
        )

        XCTAssertEqual(resolution, .noDisplayFound)
    }
}
