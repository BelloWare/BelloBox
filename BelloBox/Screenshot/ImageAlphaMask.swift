import CoreGraphics

enum ImageAlphaMask {
    /// Multiplies `image`'s alpha by `shape`'s alpha, so a rectangular crop from a display
    /// snapshot takes on the shape (rounded corners, transparent regions) of a live window
    /// image while keeping the snapshot's pixels. Returns nil when the sizes differ by more
    /// than one pixel or no drawing context can be created.
    static func apply(shapeOf shape: CGImage, to image: CGImage) -> CGImage? {
        guard image.width > 0, image.height > 0,
              abs(shape.width - image.width) <= 1,
              abs(shape.height - image.height) <= 1,
              let context = CGContext(
                data: nil,
                width: image.width,
                height: image.height,
                bitsPerComponent: 8,
                bytesPerRow: 0,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
              )
        else { return nil }
        let rect = CGRect(x: 0, y: 0, width: image.width, height: image.height)
        context.interpolationQuality = .none
        context.draw(image, in: rect)
        context.setBlendMode(.destinationIn)
        context.draw(shape, in: rect)
        return context.makeImage()
    }
}
