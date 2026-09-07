import AppKit
import CoreImage
import CoreImage.CIFilterBuiltins
import ImageIO
import UniformTypeIdentifiers

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

    /// Modules per side, including the generator's quiet zone, or nil when the
    /// string cannot be encoded. Dense codes need more pixels per module.
    static func moduleCount(for string: String) -> Int? {
        guard isEncodable(string) else { return nil }
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(string.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage, output.extent.width > 0 else { return nil }
        return Int(output.extent.width.rounded())
    }

    /// The smallest side, in points, at which every module still covers two
    /// pixels on a standard-resolution display; scanners read that reliably.
    static func readablePointSize(for string: String) -> CGFloat? {
        moduleCount(for: string).map { CGFloat($0) * 2 }
    }

    /// Export and clipboard size: at least 512 px and four pixels per module,
    /// so a dense code stays readable after any scaling.
    static func exportPixelSize(for string: String) -> CGFloat {
        max(512, CGFloat((moduleCount(for: string) ?? 0) * 4))
    }

    /// Returns a crisp, square QR image for `string`, or nil when the string is
    /// empty or too long to encode.
    static func image(for string: String, pixelSize: CGFloat = 512) -> NSImage? {
        guard let cgImage = cgImage(for: string, pixelSize: pixelSize) else { return nil }
        return NSImage(cgImage: cgImage, size: NSSize(width: pixelSize, height: pixelSize))
    }

    static func pngData(for string: String, pixelSize: CGFloat = 512) -> Data? {
        guard let cgImage = cgImage(for: string, pixelSize: pixelSize) else { return nil }
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(
            data,
            UTType.png.identifier as CFString,
            1,
            nil
        ) else { return nil }
        CGImageDestinationAddImage(destination, cgImage, nil)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return data as Data
    }

    private static func cgImage(for string: String, pixelSize: CGFloat) -> CGImage? {
        guard isEncodable(string), pixelSize > 0 else { return nil }

        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(string.utf8)
        filter.correctionLevel = "M"

        guard let output = filter.outputImage, output.extent.width > 0 else { return nil }
        let scale = pixelSize / output.extent.width
        let scaled = output.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
        return context.createCGImage(scaled, from: scaled.extent)
    }
}
