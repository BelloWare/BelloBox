import AppKit
import CoreGraphics
import Foundation

enum ScreenshotCaptureEngine: String, CaseIterable, Identifiable, Codable {
    case auto
    case screenCaptureKit
    case legacy

    var id: String { rawValue }

    var label: String {
        switch self {
        case .auto: return "Auto"
        case .screenCaptureKit: return "ScreenCaptureKit"
        case .legacy: return "Legacy"
        }
    }
}

enum DisplayCaptureSCKAvailability: String, Equatable {
    case available
    case unavailable
    case skipped
    case unverified
}

enum DisplayCaptureVerificationVerdict: String, Equatable {
    case match
    case mismatch
    case skipped
    case unverified

    var logValue: String {
        switch self {
        case .match: return "match"
        case .mismatch: return "mismatch"
        case .skipped, .unverified: return "skipped"
        }
    }
}

enum DisplayCaptureChosenEngine: String, Equatable {
    case sck
    case legacy
}

enum DisplayCaptureEnginePolicy {
    struct Decision: Equatable {
        var engine: DisplayCaptureChosenEngine
        var verify: DisplayCaptureVerificationVerdict
        var usesCachedVerdict: Bool
    }

    static func decision(
        setting: ScreenshotCaptureEngine,
        cachedVerdict: DisplayCaptureVerificationVerdict?,
        legacyAvailable: Bool
    ) -> Decision {
        switch setting {
        case .legacy:
            return Decision(engine: .legacy, verify: .skipped, usesCachedVerdict: false)
        case .screenCaptureKit:
            return Decision(engine: .sck, verify: .skipped, usesCachedVerdict: false)
        case .auto:
            switch cachedVerdict {
            case .match:
                return Decision(engine: .sck, verify: .match, usesCachedVerdict: true)
            case .mismatch:
                return Decision(
                    engine: legacyAvailable ? .legacy : .sck,
                    verify: .mismatch,
                    usesCachedVerdict: true
                )
            case .unverified:
                return Decision(engine: .sck, verify: .skipped, usesCachedVerdict: true)
            case .skipped, .none:
                return Decision(engine: .sck, verify: .skipped, usesCachedVerdict: false)
            }
        }
    }
}

struct DisplayCaptureTopology: Hashable, CustomStringConvertible {
    struct Entry: Hashable {
        var displayID: CGDirectDisplayID
        var x: Int
        var y: Int
        var width: Int
        var height: Int
        var pixelsWide: Int
        var pixelsHigh: Int
        var rotation: Int

        var description: String {
            "\(displayID)@\(x),\(y),\(width),\(height)#\(pixelsWide)x\(pixelsHigh)r\(rotation)"
        }
    }

    var entries: [Entry]

    var description: String {
        entries.map(\.description).joined(separator: ";")
    }

    static func current() -> DisplayCaptureTopology {
        var count: UInt32 = 0
        guard CGGetOnlineDisplayList(0, nil, &count) == .success, count > 0 else {
            return DisplayCaptureTopology(entries: [])
        }
        var displays = Array(repeating: CGDirectDisplayID(), count: Int(count))
        let status = displays.withUnsafeMutableBufferPointer { buffer in
            CGGetOnlineDisplayList(count, buffer.baseAddress, &count)
        }
        guard status == .success else {
            return DisplayCaptureTopology(entries: [])
        }
        let entries = displays.prefix(Int(count)).map { displayID in
            let bounds = CGDisplayBounds(displayID)
            return Entry(
                displayID: displayID,
                x: Int(bounds.origin.x.rounded()),
                y: Int(bounds.origin.y.rounded()),
                width: Int(bounds.size.width.rounded()),
                height: Int(bounds.size.height.rounded()),
                pixelsWide: CGDisplayPixelsWide(displayID),
                pixelsHigh: CGDisplayPixelsHigh(displayID),
                rotation: Int(CGDisplayRotation(displayID).rounded())
            )
        }
        return DisplayCaptureTopology(entries: entries.sorted { $0.description < $1.description })
    }
}

struct DisplayCaptureTrustReport {
    var displayID: CGDirectDisplayID
    var topology: DisplayCaptureTopology
    var sckAvailability: DisplayCaptureSCKAvailability
    var verificationVerdict: DisplayCaptureVerificationVerdict
    var chosenEngine: DisplayCaptureChosenEngine
    var reason: String
    var updatedAt: Date
}

final class DisplayCaptureTrustCache {
    static let shared = DisplayCaptureTrustCache()

    private struct Key: Hashable {
        var displayID: CGDirectDisplayID
        var topology: DisplayCaptureTopology
    }

    private let lock = NSLock()
    private var verdicts: [Key: DisplayCaptureVerificationVerdict] = [:]
    private var reports: [Key: DisplayCaptureTrustReport] = [:]
    private var latestReports: [CGDirectDisplayID: DisplayCaptureTrustReport] = [:]

    func cachedVerdict(displayID: CGDirectDisplayID, topology: DisplayCaptureTopology) -> DisplayCaptureVerificationVerdict? {
        lock.lock()
        defer { lock.unlock() }
        let verdict = verdicts[Key(displayID: displayID, topology: topology)]
        switch verdict {
        case .match, .mismatch, .unverified:
            return verdict
        case .skipped, .none:
            return nil
        }
    }

    func record(
        displayID: CGDirectDisplayID,
        topology: DisplayCaptureTopology,
        sckAvailability: DisplayCaptureSCKAvailability,
        verificationVerdict: DisplayCaptureVerificationVerdict,
        chosenEngine: DisplayCaptureChosenEngine,
        reason: String,
        updatedAt: Date = Date(),
        cacheVerdict: Bool = true
    ) {
        lock.lock()
        defer { lock.unlock() }

        let key = Key(displayID: displayID, topology: topology)
        if cacheVerdict, verificationVerdict == .match || verificationVerdict == .mismatch || verificationVerdict == .unverified {
            verdicts[key] = verificationVerdict
        }
        let report = DisplayCaptureTrustReport(
            displayID: displayID,
            topology: topology,
            sckAvailability: sckAvailability,
            verificationVerdict: verificationVerdict,
            chosenEngine: chosenEngine,
            reason: reason,
            updatedAt: updatedAt
        )
        reports[key] = report
        latestReports[displayID] = report
    }

    func report(displayID: CGDirectDisplayID, topology: DisplayCaptureTopology) -> DisplayCaptureTrustReport? {
        lock.lock()
        defer { lock.unlock() }
        return reports[Key(displayID: displayID, topology: topology)] ?? latestReports[displayID]
    }

    func invalidateAll() {
        lock.lock()
        verdicts.removeAll()
        reports.removeAll()
        latestReports.removeAll()
        lock.unlock()
    }
}

struct ImageLumaFingerprint: Equatable {
    var gridSize: Int
    var luma: [Double]

    var mean: Double {
        guard !luma.isEmpty else { return 0 }
        return luma.reduce(0, +) / Double(luma.count)
    }

    var standardDeviation: Double {
        guard !luma.isEmpty else { return 0 }
        let mean = mean
        let variance = luma.reduce(0) { $0 + pow($1 - mean, 2) } / Double(luma.count)
        return sqrt(variance)
    }
}

struct ImageFingerprintComparison: Equatable {
    var matches: Bool
    var rawMeanAbsoluteDifference: Double
    var normalizedMeanAbsoluteDifference: Double
    var offsetX: Int
    var offsetY: Int
}

enum ImageFingerprintComparator {
    static func fingerprint(_ image: CGImage, gridSize: Int = 16) -> ImageLumaFingerprint? {
        guard gridSize > 1 else { return nil }
        let bytesPerPixel = 4
        let bytesPerRow = gridSize * bytesPerPixel
        var data = [UInt8](repeating: 0, count: gridSize * bytesPerRow)
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGBitmapInfo.byteOrder32Big.union(
            CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)
        )
        let didDraw = data.withUnsafeMutableBytes { pointer -> Bool in
            guard let context = CGContext(
                data: pointer.baseAddress,
                width: gridSize,
                height: gridSize,
                bitsPerComponent: 8,
                bytesPerRow: bytesPerRow,
                space: colorSpace,
                bitmapInfo: bitmapInfo.rawValue
            ) else { return false }
            context.interpolationQuality = .medium
            context.draw(image, in: CGRect(x: 0, y: 0, width: gridSize, height: gridSize))
            return true
        }
        guard didDraw else { return nil }

        var luma: [Double] = []
        luma.reserveCapacity(gridSize * gridSize)
        for index in stride(from: 0, to: data.count, by: bytesPerPixel) {
            let red = Double(data[index]) / 255.0
            let green = Double(data[index + 1]) / 255.0
            let blue = Double(data[index + 2]) / 255.0
            luma.append(0.2126 * red + 0.7152 * green + 0.0722 * blue)
        }
        return ImageLumaFingerprint(gridSize: gridSize, luma: luma)
    }

    static func compare(
        _ lhs: ImageLumaFingerprint,
        _ rhs: ImageLumaFingerprint,
        normalizedThreshold: Double = 0.55,
        rawThreshold: Double = 0.10
    ) -> ImageFingerprintComparison {
        guard lhs.gridSize == rhs.gridSize, lhs.luma.count == rhs.luma.count, !lhs.luma.isEmpty else {
            return ImageFingerprintComparison(
                matches: false,
                rawMeanAbsoluteDifference: 1,
                normalizedMeanAbsoluteDifference: 1,
                offsetX: 0,
                offsetY: 0
            )
        }

        let offsets = [-1, 0, 1]
        var best = difference(lhs, rhs, offsetX: 0, offsetY: 0)
        for offsetY in offsets {
            for offsetX in offsets {
                let candidate = difference(lhs, rhs, offsetX: offsetX, offsetY: offsetY)
                if candidate.normalizedMeanAbsoluteDifference < best.normalizedMeanAbsoluteDifference {
                    best = candidate
                }
            }
        }

        let lowContrast = lhs.standardDeviation < 0.02 && rhs.standardDeviation < 0.02
        let matches = lowContrast
            ? best.rawMeanAbsoluteDifference <= rawThreshold
            : (best.normalizedMeanAbsoluteDifference <= normalizedThreshold || best.rawMeanAbsoluteDifference <= rawThreshold)
        return ImageFingerprintComparison(
            matches: matches,
            rawMeanAbsoluteDifference: best.rawMeanAbsoluteDifference,
            normalizedMeanAbsoluteDifference: best.normalizedMeanAbsoluteDifference,
            offsetX: best.offsetX,
            offsetY: best.offsetY
        )
    }

    static func compare(_ lhs: CGImage, _ rhs: CGImage, gridSize: Int = 16) -> ImageFingerprintComparison? {
        guard let lhsFingerprint = fingerprint(lhs, gridSize: gridSize),
              let rhsFingerprint = fingerprint(rhs, gridSize: gridSize)
        else { return nil }
        return compare(lhsFingerprint, rhsFingerprint)
    }

    private static func difference(
        _ lhs: ImageLumaFingerprint,
        _ rhs: ImageLumaFingerprint,
        offsetX: Int,
        offsetY: Int
    ) -> ImageFingerprintComparison {
        let gridSize = lhs.gridSize
        let lhsMean = lhs.mean
        let rhsMean = rhs.mean
        let lhsStdDev = max(lhs.standardDeviation, 0.08)
        let rhsStdDev = max(rhs.standardDeviation, 0.08)
        var rawTotal = 0.0
        var normalizedTotal = 0.0
        var count = 0

        for y in 0 ..< gridSize {
            let rhsY = y + offsetY
            guard rhsY >= 0, rhsY < gridSize else { continue }
            for x in 0 ..< gridSize {
                let rhsX = x + offsetX
                guard rhsX >= 0, rhsX < gridSize else { continue }
                let lhsValue = lhs.luma[y * gridSize + x]
                let rhsValue = rhs.luma[rhsY * gridSize + rhsX]
                rawTotal += abs(lhsValue - rhsValue)
                normalizedTotal += abs(((lhsValue - lhsMean) / lhsStdDev) - ((rhsValue - rhsMean) / rhsStdDev))
                count += 1
            }
        }

        guard count > 0 else {
            return ImageFingerprintComparison(
                matches: false,
                rawMeanAbsoluteDifference: 1,
                normalizedMeanAbsoluteDifference: 1,
                offsetX: offsetX,
                offsetY: offsetY
            )
        }
        return ImageFingerprintComparison(
            matches: false,
            rawMeanAbsoluteDifference: rawTotal / Double(count),
            normalizedMeanAbsoluteDifference: normalizedTotal / Double(count),
            offsetX: offsetX,
            offsetY: offsetY
        )
    }
}

struct RGBColorSample: Equatable {
    var red: Double
    var green: Double
    var blue: Double
    var alpha: Double

    init(red: Double, green: Double, blue: Double, alpha: Double = 1) {
        self.red = red
        self.green = green
        self.blue = blue
        self.alpha = alpha
    }

    init(_ color: NSColor) {
        let converted = color.usingColorSpace(.deviceRGB) ?? color
        self.init(
            red: Double(converted.redComponent),
            green: Double(converted.greenComponent),
            blue: Double(converted.blueComponent),
            alpha: Double(converted.alphaComponent)
        )
    }

    func distance(to other: RGBColorSample) -> Double {
        sqrt(pow(red - other.red, 2) + pow(green - other.green, 2) + pow(blue - other.blue, 2))
    }

    var diagnosticString: String {
        String(format: "%.3f,%.3f,%.3f", red, green, blue)
    }
}

enum ImageColorAnalyzer {
    static func averageColor(_ image: CGImage, sampleSize: Int = 16) -> RGBColorSample? {
        guard sampleSize > 0 else { return nil }
        let bytesPerPixel = 4
        let bytesPerRow = sampleSize * bytesPerPixel
        var data = [UInt8](repeating: 0, count: sampleSize * bytesPerRow)
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGBitmapInfo.byteOrder32Big.union(
            CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)
        )
        let didDraw = data.withUnsafeMutableBytes { pointer -> Bool in
            guard let context = CGContext(
                data: pointer.baseAddress,
                width: sampleSize,
                height: sampleSize,
                bitsPerComponent: 8,
                bytesPerRow: bytesPerRow,
                space: colorSpace,
                bitmapInfo: bitmapInfo.rawValue
            ) else { return false }
            context.interpolationQuality = .medium
            context.draw(image, in: CGRect(x: 0, y: 0, width: sampleSize, height: sampleSize))
            return true
        }
        guard didDraw else { return nil }

        var red = 0.0
        var green = 0.0
        var blue = 0.0
        var alpha = 0.0
        for index in stride(from: 0, to: data.count, by: bytesPerPixel) {
            red += Double(data[index]) / 255.0
            green += Double(data[index + 1]) / 255.0
            blue += Double(data[index + 2]) / 255.0
            alpha += Double(data[index + 3]) / 255.0
        }
        let count = Double(sampleSize * sampleSize)
        return RGBColorSample(red: red / count, green: green / count, blue: blue / count, alpha: alpha / count)
    }
}
