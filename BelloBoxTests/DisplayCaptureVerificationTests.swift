import AppKit
import CoreGraphics
import XCTest
@testable import BelloBox

final class DisplayCaptureVerificationTests: XCTestCase {
    func testFingerprintComparatorMatchesIdenticalImages() throws {
        let image = makePatternImage()

        let comparison = try XCTUnwrap(ImageFingerprintComparator.compare(image, image))

        XCTAssertTrue(comparison.matches)
        XCTAssertLessThan(comparison.rawMeanAbsoluteDifference, 0.001)
        XCTAssertLessThan(comparison.normalizedMeanAbsoluteDifference, 0.001)
    }

    func testFingerprintComparatorToleratesSmallShift() throws {
        let image = makePatternImage()
        let shifted = makePatternImage(shiftX: 3, shiftY: -2)

        let comparison = try XCTUnwrap(ImageFingerprintComparator.compare(image, shifted))

        XCTAssertTrue(comparison.matches)
    }

    func testFingerprintComparatorRejectsWrongImage() throws {
        let image = makePatternImage()
        let wrong = solidImage(color: .white)

        let comparison = try XCTUnwrap(ImageFingerprintComparator.compare(image, wrong))

        XCTAssertFalse(comparison.matches)
    }

    func testTrustCacheMismatchChoosesLegacyAndTopologyChangeReverifies() {
        let cache = DisplayCaptureTrustCache()
        let firstTopology = topology(displayID: 100, x: 0)
        let changedTopology = topology(displayID: 100, x: 500)

        cache.record(
            displayID: 100,
            topology: firstTopology,
            sckAvailability: .available,
            verificationVerdict: .mismatch,
            chosenEngine: .legacy,
            reason: "test"
        )

        let cached = cache.cachedVerdict(displayID: 100, topology: firstTopology)
        let cachedDecision = DisplayCaptureEnginePolicy.decision(
            setting: .auto,
            cachedVerdict: cached,
            legacyAvailable: true
        )
        XCTAssertEqual(cached, .mismatch)
        XCTAssertEqual(cachedDecision.engine, .legacy)
        XCTAssertEqual(cachedDecision.verify, .mismatch)

        let changedCached = cache.cachedVerdict(displayID: 100, topology: changedTopology)
        let changedDecision = DisplayCaptureEnginePolicy.decision(
            setting: .auto,
            cachedVerdict: changedCached,
            legacyAvailable: true
        )
        XCTAssertNil(changedCached)
        XCTAssertEqual(changedDecision.engine, .sck)
        XCTAssertEqual(changedDecision.verify, .skipped)
    }

    func testTrustCacheStoresUnverifiedVerdictPerTopology() {
        let cache = DisplayCaptureTrustCache()
        let topology = topology(displayID: 200, x: 0)

        cache.record(
            displayID: 200,
            topology: topology,
            sckAvailability: .available,
            verificationVerdict: .unverified,
            chosenEngine: .sck,
            reason: "legacyUnavailable"
        )

        XCTAssertEqual(cache.cachedVerdict(displayID: 200, topology: topology), .unverified)
    }

    func testEngineSettingOverridePaths() {
        XCTAssertEqual(
            DisplayCaptureEnginePolicy.decision(setting: .legacy, cachedVerdict: .match, legacyAvailable: true),
            DisplayCaptureEnginePolicy.Decision(engine: .legacy, verify: .skipped, usesCachedVerdict: false)
        )
        XCTAssertEqual(
            DisplayCaptureEnginePolicy.decision(setting: .screenCaptureKit, cachedVerdict: .mismatch, legacyAvailable: true),
            DisplayCaptureEnginePolicy.Decision(engine: .sck, verify: .skipped, usesCachedVerdict: false)
        )
        XCTAssertEqual(
            DisplayCaptureEnginePolicy.decision(setting: .auto, cachedVerdict: .match, legacyAvailable: true),
            DisplayCaptureEnginePolicy.Decision(engine: .sck, verify: .match, usesCachedVerdict: true)
        )
        XCTAssertEqual(
            DisplayCaptureEnginePolicy.decision(setting: .auto, cachedVerdict: .mismatch, legacyAvailable: false),
            DisplayCaptureEnginePolicy.Decision(engine: .sck, verify: .mismatch, usesCachedVerdict: true)
        )
        XCTAssertEqual(
            DisplayCaptureEnginePolicy.decision(setting: .auto, cachedVerdict: .unverified, legacyAvailable: true),
            DisplayCaptureEnginePolicy.Decision(engine: .sck, verify: .skipped, usesCachedVerdict: true)
        )
    }

    private func topology(displayID: CGDirectDisplayID, x: Int) -> DisplayCaptureTopology {
        DisplayCaptureTopology(entries: [
            DisplayCaptureTopology.Entry(
                displayID: displayID,
                x: x,
                y: 0,
                width: 300,
                height: 200,
                pixelsWide: 600,
                pixelsHigh: 400,
                rotation: 0
            )
        ])
    }

    private func makePatternImage(shiftX: CGFloat = 0, shiftY: CGFloat = 0) -> CGImage {
        let context = CGContext(
            data: nil,
            width: 128,
            height: 96,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(NSColor.black.cgColor)
        context.fill(CGRect(x: 0, y: 0, width: 128, height: 96))
        context.translateBy(x: shiftX, y: shiftY)
        context.setFillColor(NSColor.white.cgColor)
        context.fill(CGRect(x: 14, y: 10, width: 44, height: 70))
        context.setFillColor(NSColor(calibratedWhite: 0.45, alpha: 1).cgColor)
        context.fill(CGRect(x: 76, y: 18, width: 34, height: 52))
        context.setFillColor(NSColor(calibratedWhite: 0.75, alpha: 1).cgColor)
        context.fill(CGRect(x: 34, y: 72, width: 78, height: 10))
        return context.makeImage()!
    }

    private func solidImage(color: NSColor) -> CGImage {
        let context = CGContext(
            data: nil,
            width: 128,
            height: 96,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(color.cgColor)
        context.fill(CGRect(x: 0, y: 0, width: 128, height: 96))
        return context.makeImage()!
    }
}
