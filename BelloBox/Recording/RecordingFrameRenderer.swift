import AppKit
import CoreImage
import CoreMedia
import CoreText
import Foundation

struct RecordingFrameRenderContext: Equatable {
    let sourceScreenRect: CGRect
    let outputSize: CGSize
    let clickOverlayMode: ClickOverlayMode
    let keystrokeMode: KeystrokeCaptureMode
    let secureFieldRedactionMode: SecureFieldRedactionMode
}

final class RecordingFrameRenderer {
    private let ciContext = CIContext()
    private let colorSpace = CGColorSpaceCreateDeviceRGB()

    func render(
        sourcePixelBuffer: CVPixelBuffer,
        into outputPixelBuffer: CVPixelBuffer,
        context: RecordingFrameRenderContext,
        overlayEvents: [TimedOverlayEvent],
        sensitiveState: SensitiveInputState
    ) {
        let width = CVPixelBufferGetWidth(outputPixelBuffer)
        let height = CVPixelBufferGetHeight(outputPixelBuffer)
        let source = CIImage(cvPixelBuffer: sourcePixelBuffer)
        let scaleX = CGFloat(width) / max(source.extent.width, 1)
        let scaleY = CGFloat(height) / max(source.extent.height, 1)
        let scaled = source.transformed(by: CGAffineTransform(scaleX: scaleX, y: scaleY))
        ciContext.render(
            scaled,
            to: outputPixelBuffer,
            bounds: CGRect(x: 0, y: 0, width: width, height: height),
            colorSpace: colorSpace
        )

        CVPixelBufferLockBaseAddress(outputPixelBuffer, [])
        defer { CVPixelBufferUnlockBaseAddress(outputPixelBuffer, []) }

        guard let baseAddress = CVPixelBufferGetBaseAddress(outputPixelBuffer),
              let cgContext = CGContext(
                data: baseAddress,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: CVPixelBufferGetBytesPerRow(outputPixelBuffer),
                space: colorSpace,
                bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
              )
        else { return }

        drawPrivacyRedaction(sensitiveState, in: cgContext, context: context, outputSize: CGSize(width: width, height: height))
        drawOverlays(
            overlayEvents,
            sensitiveState: sensitiveState,
            in: cgContext,
            context: context,
            outputSize: CGSize(width: width, height: height)
        )
    }

    private func drawPrivacyRedaction(
        _ state: SensitiveInputState,
        in cgContext: CGContext,
        context: RecordingFrameRenderContext,
        outputSize: CGSize
    ) {
        let layout = redactionLayout(for: state, context: context, outputSize: outputSize)
        if layout.fullFrame {
            fillFullFrame(in: cgContext, outputSize: outputSize)
            return
        }

        for rect in layout.rects {
            cgContext.setFillColor(NSColor.black.withAlphaComponent(0.94).cgColor)
            cgContext.fill(rect)
        }
    }

    private func drawOverlays(
        _ events: [TimedOverlayEvent],
        sensitiveState: SensitiveInputState,
        in cgContext: CGContext,
        context: RecordingFrameRenderContext,
        outputSize: CGSize
    ) {
        let layout = redactionLayout(for: sensitiveState, context: context, outputSize: outputSize)
        for event in events {
            switch event.kind {
            case let .click(click):
                guard !layout.fullFrame else { continue }
                drawClick(click, in: cgContext, context: context, outputSize: outputSize, redactedRects: layout.rects)
            case let .keystroke(key):
                guard !layout.fullFrame else { continue }
                guard shouldDraw(key: key, sensitiveState: sensitiveState, mode: context.keystrokeMode) else { continue }
                drawKeystroke(key.displayLabel, in: cgContext, outputSize: outputSize)
            case .secureTypingHidden:
                drawKeystroke("Secure typing hidden", in: cgContext, outputSize: outputSize)
            }
        }
    }

    private func shouldDraw(
        key: KeystrokeOverlayEvent,
        sensitiveState: SensitiveInputState,
        mode: KeystrokeCaptureMode
    ) -> Bool {
        guard mode != .off else { return false }
        if sensitiveState.isSensitive, key.isPrintable { return false }
        if mode == .shortcutsOnly { return key.isShortcut }
        return true
    }

    private func drawClick(
        _ click: ClickOverlayEvent,
        in cgContext: CGContext,
        context: RecordingFrameRenderContext,
        outputSize: CGSize,
        redactedRects: [CGRect]
    ) {
        guard context.clickOverlayMode.isEnabled else { return }
        let point = pixelPoint(for: click.locationInScreenPoints, sourceScreenRect: context.sourceScreenRect, outputSize: outputSize)
        guard CGRect(origin: .zero, size: outputSize).insetBy(dx: -60, dy: -60).contains(point) else { return }
        guard !redactedRects.contains(where: { $0.insetBy(dx: -4, dy: -4).contains(point) }) else { return }

        let radius: CGFloat = click.clickCount > 1 ? 28 : 22
        let ring = CGRect(x: point.x - radius, y: point.y - radius, width: radius * 2, height: radius * 2)
        cgContext.setStrokeColor(NSColor.systemBlue.cgColor)
        cgContext.setLineWidth(5)
        cgContext.strokeEllipse(in: ring)
        cgContext.setFillColor(NSColor.systemBlue.withAlphaComponent(0.16).cgColor)
        cgContext.fillEllipse(in: ring.insetBy(dx: 3, dy: 3))

        guard context.clickOverlayMode == .ringsAndLabels else { return }
        let label: String
        switch click.button {
        case .left: label = click.clickCount > 1 ? "Double click" : "Click"
        case .right: label = "Right click"
        case .middle: label = "Middle click"
        case .other: label = "Click"
        }
        drawBubble(label, near: CGPoint(x: point.x + radius + 8, y: point.y - 10), in: cgContext)
    }

    private func drawKeystroke(_ label: String, in cgContext: CGContext, outputSize: CGSize) {
        drawBubble(label, near: CGPoint(x: outputSize.width / 2, y: 34), in: cgContext, centered: true)
    }

    private func drawBubble(_ label: String, near point: CGPoint, in cgContext: CGContext, centered: Bool = false) {
        let font = CTFontCreateWithName("SF Pro Rounded" as CFString, 18, nil)
        let attributes: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: NSColor.white
        ]
        let attributed = NSAttributedString(string: label, attributes: attributes)
        let line = CTLineCreateWithAttributedString(attributed)
        let bounds = CTLineGetBoundsWithOptions(line, [.useGlyphPathBounds])
        let paddingX: CGFloat = 12
        let paddingY: CGFloat = 7
        let width = ceil(bounds.width) + paddingX * 2
        let height = ceil(bounds.height) + paddingY * 2 + 3
        let x = centered ? point.x - width / 2 : point.x
        let y = point.y
        let rect = CGRect(x: x, y: y, width: width, height: height)

        cgContext.saveGState()
        cgContext.setFillColor(NSColor.black.withAlphaComponent(0.72).cgColor)
        cgContext.addPath(CGPath(roundedRect: rect, cornerWidth: 10, cornerHeight: 10, transform: nil))
        cgContext.fillPath()
        cgContext.textPosition = CGPoint(x: rect.minX + paddingX, y: rect.minY + paddingY)
        CTLineDraw(line, cgContext)
        cgContext.restoreGState()
    }

    private func fillFullFrame(in cgContext: CGContext, outputSize: CGSize) {
        cgContext.setFillColor(NSColor.black.cgColor)
        cgContext.fill(CGRect(origin: .zero, size: outputSize))
    }

    private func redactionLayout(
        for state: SensitiveInputState,
        context: RecordingFrameRenderContext,
        outputSize: CGSize
    ) -> (fullFrame: Bool, rects: [CGRect]) {
        switch state {
        case .notSensitive, .detectorUnavailable:
            return (false, [])
        case let .sensitiveKnownFrame(info):
            guard let frame = info.frameInScreenPoints, !frame.isNull, !frame.isEmpty else {
                return (context.secureFieldRedactionMode == .strict, [])
            }
            let bounds = CGRect(origin: .zero, size: outputSize)
            let rect = pixelRect(for: frame, sourceScreenRect: context.sourceScreenRect, outputSize: outputSize)
                .insetBy(dx: -8, dy: -8)
                .intersection(bounds)
            return rect.isNull || rect.isEmpty ? (false, []) : (false, [rect])
        case .sensitiveUnknownFrame:
            return (context.secureFieldRedactionMode == .strict, [])
        }
    }

    private func pixelPoint(for point: CGPoint, sourceScreenRect: CGRect, outputSize: CGSize) -> CGPoint {
        CGPoint(
            x: (point.x - sourceScreenRect.minX) / max(sourceScreenRect.width, 1) * outputSize.width,
            y: (point.y - sourceScreenRect.minY) / max(sourceScreenRect.height, 1) * outputSize.height
        )
    }

    private func pixelRect(for rect: CGRect, sourceScreenRect: CGRect, outputSize: CGSize) -> CGRect {
        let origin = pixelPoint(for: rect.origin, sourceScreenRect: sourceScreenRect, outputSize: outputSize)
        let far = pixelPoint(for: CGPoint(x: rect.maxX, y: rect.maxY), sourceScreenRect: sourceScreenRect, outputSize: outputSize)
        return CGRect(
            x: min(origin.x, far.x),
            y: min(origin.y, far.y),
            width: abs(far.x - origin.x),
            height: abs(far.y - origin.y)
        )
    }
}
