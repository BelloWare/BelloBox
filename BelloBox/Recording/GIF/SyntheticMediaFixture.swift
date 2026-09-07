#if DEBUG
import AppKit
import AVFoundation
import CoreGraphics
import Foundation

/// Deterministic media for review fixtures and tests: a page-like tall image and a
/// short synthetic movie. Nothing here reads the screen or a real recording.
enum SyntheticMediaFixture {
    /// A "web page": a header bar, a sequence of coloured section bands with
    /// numbers, and text-like lines, so stitching, zoom and masks have something
    /// recognisable to work on.
    static func tallPage(width: Int, height: Int) -> CGImage? {
        guard width > 0, height > 0,
              let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                                      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        let w = CGFloat(width), h = CGFloat(height)
        context.setFillColor(CGColor(red: 0.985, green: 0.972, blue: 0.955, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: w, height: h))
        let bandHeight: CGFloat = 260
        var index = 0
        var y = h
        let nsContext = NSGraphicsContext(cgContext: context, flipped: false)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = nsContext
        while y > 0 {
            let band = CGRect(x: 24, y: y - bandHeight + 16, width: w - 48, height: bandHeight - 32)
            let hue = CGFloat((index * 5) % 12) / 12
            context.setFillColor(NSColor(calibratedHue: hue, saturation: 0.35, brightness: 0.97, alpha: 1).cgColor)
            let path = CGPath(roundedRect: band, cornerWidth: 14, cornerHeight: 14, transform: nil)
            context.addPath(path)
            context.fillPath()
            context.setFillColor(NSColor(calibratedHue: hue, saturation: 0.7, brightness: 0.6, alpha: 1).cgColor)
            context.fill(CGRect(x: band.minX + 20, y: band.maxY - 44, width: min(260, band.width * 0.4), height: 18))
            var line = band.maxY - 76
            var row = 0
            while line > band.minY + 20 {
                let width = band.width * (0.55 + CGFloat((row * 7) % 5) * 0.08)
                context.setFillColor(CGColor(gray: 0.35, alpha: 0.35))
                context.fill(CGRect(x: band.minX + 20, y: line, width: min(width, band.width - 40), height: 8))
                line -= 22
                row += 1
            }
            let label = "Section \(index + 1)" as NSString
            let attributes: [NSAttributedString.Key: Any] = [
                .font: NSFont.systemFont(ofSize: 30, weight: .bold),
                .foregroundColor: NSColor.black.withAlphaComponent(0.55),
            ]
            let size = label.size(withAttributes: attributes)
            label.draw(at: CGPoint(x: band.maxX - size.width - 24, y: band.maxY - size.height - 12), withAttributes: attributes)
            y -= bandHeight
            index += 1
        }
        // A sticky-looking header at the top.
        context.setFillColor(CGColor(red: 0.16, green: 0.16, blue: 0.18, alpha: 1))
        context.fill(CGRect(x: 0, y: h - 64, width: w, height: 64))
        context.setFillColor(CGColor(red: 0.89, green: 0.46, blue: 0.15, alpha: 1))
        context.fill(CGRect(x: 24, y: h - 44, width: 120, height: 24))
        NSGraphicsContext.restoreGraphicsState()
        return context.makeImage()
    }

    /// One frame of the synthetic movie: a moving orange square over a blue-to-cream
    /// gradient with the frame number, so timing and orientation are checkable.
    /// Solid magenta, used for frames after a sentinel time so a trim test can prove
    /// nothing past the cut was taken.
    static let sentinelColor = CGColor(red: 1, green: 0, blue: 1, alpha: 1)

    static func movieFrame(index: Int, count: Int, size: CGSize, sentinel: Bool = false) -> CGImage? {
        let width = Int(size.width), height = Int(size.height)
        guard width > 0, height > 0,
              let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                                      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        if sentinel {
            context.setFillColor(sentinelColor)
            context.fill(CGRect(x: 0, y: 0, width: width, height: height))
            return context.makeImage()
        }
        let progress = count > 1 ? CGFloat(index) / CGFloat(count - 1) : 0
        context.setFillColor(CGColor(red: 0.12, green: 0.30, blue: 0.62, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        context.setFillColor(CGColor(red: 0.985, green: 0.972, blue: 0.955, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(height) / 2))
        let square = min(size.width, size.height) * 0.28
        let x = (size.width - square) * progress
        context.setFillColor(CGColor(red: 0.89, green: 0.46, blue: 0.15, alpha: 1))
        context.fill(CGRect(x: x, y: (size.height - square) / 2, width: square, height: square))
        let nsContext = NSGraphicsContext(cgContext: context, flipped: false)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = nsContext
        let label = "\(index + 1)" as NSString
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedDigitSystemFont(ofSize: min(size.width, size.height) * 0.12, weight: .bold),
            .foregroundColor: NSColor.white,
        ]
        label.draw(at: CGPoint(x: 16, y: 16), withAttributes: attributes)
        NSGraphicsContext.restoreGraphicsState()
        return context.makeImage()
    }

    /// Writes an H.264 movie of `duration` seconds at `framesPerSecond`. A
    /// `transform` becomes the track's preferred transform (rotation tests).
    @discardableResult
    static func writeMovie(to url: URL, size: CGSize, duration: TimeInterval, framesPerSecond: Int,
                           transform: CGAffineTransform = .identity, sentinelAfter: TimeInterval? = nil) async throws -> URL {
        try? FileManager.default.removeItem(at: url)
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        let width = Int(size.width), height = Int(size.height)
        let writer = try AVAssetWriter(outputURL: url, fileType: .mov)
        let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: width,
            AVVideoHeightKey: height,
        ])
        input.expectsMediaDataInRealTime = false
        input.transform = transform
        let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: width,
            kCVPixelBufferHeightKey as String: height,
        ])
        guard writer.canAdd(input) else { throw GIFTranscoderError.cannotWrite }
        writer.add(input)
        guard writer.startWriting() else { throw writer.error ?? GIFTranscoderError.cannotWrite }
        writer.startSession(atSourceTime: .zero)
        let count = max(1, Int((duration * Double(framesPerSecond)).rounded()))
        for index in 0..<count {
            while !input.isReadyForMoreMediaData { try await Task.sleep(nanoseconds: 5_000_000) }
            let sentinel = sentinelAfter.map { Double(index) / Double(framesPerSecond) >= $0 } ?? false
            guard let pool = adaptor.pixelBufferPool, let frame = movieFrame(index: index, count: count, size: size, sentinel: sentinel) else {
                throw GIFTranscoderError.cannotWrite
            }
            var buffer: CVPixelBuffer?
            guard CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault, pool, &buffer) == kCVReturnSuccess, let buffer else {
                throw GIFTranscoderError.cannotWrite
            }
            CVPixelBufferLockBaseAddress(buffer, [])
            if let base = CVPixelBufferGetBaseAddress(buffer),
               let context = CGContext(data: base, width: width, height: height, bitsPerComponent: 8,
                                       bytesPerRow: CVPixelBufferGetBytesPerRow(buffer), space: CGColorSpaceCreateDeviceRGB(),
                                       bitmapInfo: CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.premultipliedFirst.rawValue) {
                context.draw(frame, in: CGRect(x: 0, y: 0, width: width, height: height))
            }
            CVPixelBufferUnlockBaseAddress(buffer, [])
            let time = CMTime(value: CMTimeValue(index), timescale: CMTimeScale(framesPerSecond))
            guard adaptor.append(buffer, withPresentationTime: time) else { throw writer.error ?? GIFTranscoderError.cannotWrite }
        }
        input.markAsFinished()
        await writer.finishWriting()
        guard writer.status == .completed else { throw writer.error ?? GIFTranscoderError.cannotWrite }
        return url
    }
}
#endif
