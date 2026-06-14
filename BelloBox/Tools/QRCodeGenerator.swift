import AppKit
import CoreImage
import CoreImage.CIFilterBuiltins

/// Renders text into a QR code image using CoreImage (no external dependencies).
enum QRCodeGenerator {
    /// Rough upper bound for a version-40 QR at medium correction. Beyond this
    /// the generator returns nil; we surface a friendly message instead.
    static let maxByteCount = 2000

    private static let context = CIContext()

    static func isEncodable(_ string: String) -> Bool {
        let isBlank = string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        return !isBlank && string.utf8.count <= maxByteCount
    }

    /// Returns a crisp, square QR image for `string`, or nil when the string is
    /// empty or too long to encode.
    static func image(for string: String, pixelSize: CGFloat = 512) -> NSImage? {
        guard isEncodable(string) else { return nil }

        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(string.utf8)
        filter.correctionLevel = "M"

        guard let output = filter.outputImage, output.extent.width > 0 else { return nil }
        let scale = pixelSize / output.extent.width
        let scaled = output.transformed(by: CGAffineTransform(scaleX: scale, y: scale))

        guard let cgImage = context.createCGImage(scaled, from: scaled.extent) else { return nil }
        return NSImage(cgImage: cgImage, size: NSSize(width: pixelSize, height: pixelSize))
    }

    static func pngData(for string: String, pixelSize: CGFloat = 512) -> Data? {
        guard
            let image = image(for: string, pixelSize: pixelSize),
            let tiff = image.tiffRepresentation,
            let rep = NSBitmapImageRep(data: tiff)
        else { return nil }
        return rep.representation(using: .png, properties: [:])
    }
}
