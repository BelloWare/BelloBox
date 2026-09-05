import AppKit
import XCTest
@testable import BelloBox

enum ScreenshotTestHelpers {
    @MainActor
    static func annotationPreview(annotations: [ScreenshotAnnotation], imageSize: CGSize, viewSize: CGSize) throws -> CGImage {
        let view = AnnotationDrawingNSView(frame: CGRect(origin: .zero, size: viewSize))
        view.imageSize = imageSize
        view.annotations = annotations
        let bitmap = try XCTUnwrap(NSBitmapImageRep(
            bitmapDataPlanes: nil, pixelsWide: Int(viewSize.width), pixelsHigh: Int(viewSize.height),
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
            colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 32
        ))
        bitmap.size = viewSize
        view.cacheDisplay(in: view.bounds, to: bitmap)
        return try XCTUnwrap(bitmap.cgImage)
    }

    static func image(width: Int, height: Int, draw: ((CGContext) -> Void)? = nil) -> CGImage {
        let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        if let draw {
            draw(context)
        } else {
            context.setFillColor(NSColor.white.cgColor)
            context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        }
        return context.makeImage()!
    }

    static func stripedImage(width: Int, height: Int) -> CGImage {
        image(width: width, height: height) { context in
            for y in 0..<height {
                let value = CGFloat((y * 7) % 255) / 255
                context.setFillColor(NSColor(calibratedRed: value, green: 0.25, blue: 1 - value, alpha: 1).cgColor)
                context.fill(CGRect(x: 0, y: y, width: width, height: 1))
            }
        }
    }

    static func rgbaPixels(_ image: CGImage) -> [UInt8] {
        var bytes = [UInt8](repeating: 0, count: image.width * image.height * 4)
        bytes.withUnsafeMutableBytes { buffer in
            let context = CGContext(data: buffer.baseAddress, width: image.width, height: image.height,
                bitsPerComponent: 8, bytesPerRow: image.width * 4, space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        }
        return bytes
    }

    static func pixel(_ image: CGImage, x: Int, y: Int) -> [UInt8] {
        var data = [UInt8](repeating: 0, count: 4)
        let context = CGContext(
            data: &data,
            width: 1,
            height: 1,
            bitsPerComponent: 8,
            bytesPerRow: 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.translateBy(x: CGFloat(-x), y: CGFloat(y - image.height + 1))
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return data
    }
}
