import AppKit
import ImageIO
import UniformTypeIdentifiers

enum ImageExportError: LocalizedError, Equatable {
    case encodingFailed(String)
    case pasteboardFailed
    case saveCancelled

    var errorDescription: String? {
        switch self {
        case let .encodingFailed(type):
            return "Could not encode image as \(type)."
        case .pasteboardFailed:
            return "Could not copy the image to the pasteboard."
        case .saveCancelled:
            return "Save was cancelled."
        }
    }
}

enum ImageExportService {
    static func pngData(from image: CGImage) throws -> Data {
        try encodedData(from: image, type: UTType.png.identifier, properties: nil)
    }

    static func jpegData(from image: CGImage, quality: CGFloat) throws -> Data {
        try encodedData(
            from: image,
            type: UTType.jpeg.identifier,
            properties: [kCGImageDestinationLossyCompressionQuality as String: quality]
        )
    }

    static func copyToPasteboard(_ image: CGImage) throws {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()

        let nsImage = NSImage(cgImage: image, size: NSSize(width: image.width, height: image.height))
        var wrote = pasteboard.writeObjects([nsImage])
        if let png = try? pngData(from: image) {
            wrote = pasteboard.setData(png, forType: .png) || wrote
        }
        if !wrote { throw ImageExportError.pasteboardFailed }
    }

    @MainActor
    static func savePNG(_ image: CGImage, suggestedName: String) throws {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.png]
        panel.canCreateDirectories = true
        panel.nameFieldStringValue = suggestedName.hasSuffix(".png") ? suggestedName : "\(suggestedName).png"

        guard panel.runModal() == .OK, let url = panel.url else {
            throw ImageExportError.saveCancelled
        }
        let data = try pngData(from: image)
        try data.write(to: url, options: .atomic)
    }

    private static func encodedData(from image: CGImage, type: String, properties: [String: Any]?) throws -> Data {
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data, type as CFString, 1, nil) else {
            throw ImageExportError.encodingFailed(type)
        }
        CGImageDestinationAddImage(destination, image, properties as CFDictionary?)
        guard CGImageDestinationFinalize(destination) else {
            throw ImageExportError.encodingFailed(type)
        }
        return data as Data
    }
}

