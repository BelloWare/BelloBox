import AppKit
import CoreMedia
import CoreVideo
@testable import BelloBox
import XCTest

final class RecordingFrameRendererTests: XCTestCase {
    func testKnownSensitiveFrameIsRedactedBeforeOutput() {
        let output = render(
            sensitiveState: .sensitiveKnownFrame(
                SensitiveFieldInfo(
                    reason: .secureTextField,
                    frameInScreenPoints: CGRect(x: 45, y: 45, width: 10, height: 10),
                    owningAppBundleID: nil,
                    confidence: 1.0
                )
            ),
            overlayEvents: []
        )

        XCTAssertTrue(isDark(pixel(at: CGPoint(x: 50, y: 50), in: output)))
        XCTAssertFalse(isDark(pixel(at: CGPoint(x: 5, y: 5), in: output)))
    }

    func testStrictUnknownSensitiveFrameBlacksOutFrameAndSuppressesClickOverlay() {
        let click = TimedOverlayEvent(
            id: UUID(),
            time: .zero,
            kind: .click(
                ClickOverlayEvent(
                    button: .left,
                    clickCount: 1,
                    locationInScreenPoints: CGPoint(x: 50, y: 50)
                )
            ),
            expiresAt: CMTime(seconds: 1, preferredTimescale: 600)
        )

        let output = render(
            sensitiveState: .sensitiveUnknownFrame(reason: .unknownFocusedTextField),
            overlayEvents: [click]
        )

        XCTAssertTrue(isDark(pixel(at: CGPoint(x: 50, y: 50), in: output)))
    }

    func testClickInsideKnownSensitiveFrameIsSuppressed() {
        let click = TimedOverlayEvent(
            id: UUID(),
            time: .zero,
            kind: .click(
                ClickOverlayEvent(
                    button: .left,
                    clickCount: 1,
                    locationInScreenPoints: CGPoint(x: 50, y: 50)
                )
            ),
            expiresAt: CMTime(seconds: 1, preferredTimescale: 600)
        )

        let output = render(
            sensitiveState: .sensitiveKnownFrame(
                SensitiveFieldInfo(
                    reason: .secureTextField,
                    frameInScreenPoints: CGRect(x: 45, y: 45, width: 10, height: 10),
                    owningAppBundleID: nil,
                    confidence: 1.0
                )
            ),
            overlayEvents: [click]
        )

        XCTAssertTrue(isDark(pixel(at: CGPoint(x: 50, y: 50), in: output)))
    }

    private func render(
        sensitiveState: SensitiveInputState,
        overlayEvents: [TimedOverlayEvent]
    ) -> CVPixelBuffer {
        let source = makePixelBuffer(width: 100, height: 100, red: 255, green: 255, blue: 255)
        let output = makePixelBuffer(width: 100, height: 100, red: 0, green: 0, blue: 0)
        let context = RecordingFrameRenderContext(
            sourceScreenRect: CGRect(x: 0, y: 0, width: 100, height: 100),
            outputSize: CGSize(width: 100, height: 100),
            clickOverlayMode: .ringsAndLabels,
            keystrokeMode: .allKeys,
            secureFieldRedactionMode: .strict
        )

        RecordingFrameRenderer().render(
            sourcePixelBuffer: source,
            into: output,
            context: context,
            overlayEvents: overlayEvents,
            sensitiveState: sensitiveState
        )
        return output
    }

    private func makePixelBuffer(width: Int, height: Int, red: UInt8, green: UInt8, blue: UInt8) -> CVPixelBuffer {
        var pixelBuffer: CVPixelBuffer?
        let attributes: [String: Any] = [
            kCVPixelBufferCGImageCompatibilityKey as String: true,
            kCVPixelBufferCGBitmapContextCompatibilityKey as String: true,
            kCVPixelBufferIOSurfacePropertiesKey as String: [:]
        ]
        let status = CVPixelBufferCreate(
            kCFAllocatorDefault,
            width,
            height,
            kCVPixelFormatType_32BGRA,
            attributes as CFDictionary,
            &pixelBuffer
        )
        XCTAssertEqual(status, kCVReturnSuccess)
        guard let pixelBuffer else {
            XCTFail("Could not create test pixel buffer.")
            fatalError("Could not create test pixel buffer.")
        }

        CVPixelBufferLockBaseAddress(pixelBuffer, [])
        let bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)
        let base = CVPixelBufferGetBaseAddress(pixelBuffer)!.assumingMemoryBound(to: UInt8.self)
        for y in 0..<height {
            for x in 0..<width {
                let offset = y * bytesPerRow + x * 4
                base[offset] = blue
                base[offset + 1] = green
                base[offset + 2] = red
                base[offset + 3] = 255
            }
        }
        CVPixelBufferUnlockBaseAddress(pixelBuffer, [])
        return pixelBuffer
    }

    private func pixel(at point: CGPoint, in pixelBuffer: CVPixelBuffer) -> (red: UInt8, green: UInt8, blue: UInt8) {
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }
        let x = max(0, min(CVPixelBufferGetWidth(pixelBuffer) - 1, Int(point.x)))
        let y = max(0, min(CVPixelBufferGetHeight(pixelBuffer) - 1, Int(point.y)))
        let offset = y * CVPixelBufferGetBytesPerRow(pixelBuffer) + x * 4
        let base = CVPixelBufferGetBaseAddress(pixelBuffer)!.assumingMemoryBound(to: UInt8.self)
        return (red: base[offset + 2], green: base[offset + 1], blue: base[offset])
    }

    private func isDark(_ pixel: (red: UInt8, green: UInt8, blue: UInt8)) -> Bool {
        Int(pixel.red) + Int(pixel.green) + Int(pixel.blue) < 90
    }
}
