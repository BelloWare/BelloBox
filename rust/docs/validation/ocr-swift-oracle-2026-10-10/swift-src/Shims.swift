// Shims for types the extracted Swift Box OCR sources reference (e43b1c4).
import CoreGraphics

// BelloBox/Screenshot/ScreenshotModels.swift, verbatim.
struct CGRectCodable: Equatable, Codable {
    var x: CGFloat
    var y: CGFloat
    var width: CGFloat
    var height: CGFloat

    init(_ rect: CGRect) {
        x = rect.origin.x
        y = rect.origin.y
        width = rect.size.width
        height = rect.size.height
    }

    var rect: CGRect { CGRect(x: x, y: y, width: width, height: height) }
}

// Only named by OCREngine.llm; never constructed by the oracle.
enum ProviderKind: String, Codable, Equatable { case unused }
