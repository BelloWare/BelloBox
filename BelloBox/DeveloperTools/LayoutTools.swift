import Foundation

struct BezierCurve {
    let x1: Double, y1: Double, x2: Double, y2: Double
    var points: [Double] { [x1, y1, x2, y2] }
    static func parse(_ input: String) throws -> Self {
        var text = input.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let presets = ["linear": "0,0,1,1", "ease": "0.25,0.1,0.25,1", "ease-in": "0.42,0,1,1", "ease-out": "0,0,0.58,1", "ease-in-out": "0.42,0,0.58,1"]
        if let preset = presets[text] { text = preset }
        if text.hasPrefix("cubic-bezier("), text.hasSuffix(")") { text = String(text.dropFirst(13).dropLast()) }
        let parts = text.split(separator: ",", omittingEmptySubsequences: false).map { $0.trimmingCharacters(in: .whitespaces) }
        let values = parts.compactMap(Double.init)
        guard parts.count == 4, values.count == 4, values.allSatisfy(\.isFinite), (0...1).contains(values[0]), (0...1).contains(values[2]), (-10...10).contains(values[1]), (-10...10).contains(values[3]) else { throw UtilityError("Enter x1, y1, x2, y2: X must be 0–1; this editor supports Y from −10 to 10. Presets: ease, ease-in, ease-out, ease-in-out, linear.") }
        return .init(x1: values[0], y1: values[1], x2: values[2], y2: values[3])
    }
    static func coordinate(_ t: Double, _ a: Double, _ b: Double) -> Double { 3 * (1 - t) * (1 - t) * t * a + 3 * (1 - t) * t * t * b + t * t * t }
    func value(at progress: Double) -> Double {
        if progress <= 0 { return 0 }; if progress >= 1 { return 1 }
        var low = 0.0, high = 1.0
        for _ in 0..<60 { let t = (low + high) / 2; if Self.coordinate(t, x1, x2) < progress { low = t } else { high = t } }
        return Self.coordinate((low + high) / 2, y1, y2)
    }
}
struct BoxShadowSpec {
    let color: UtilityColor
    let x: Double, y: Double, blur: Double, spread: Double
}
enum LayoutTools {
    static func run(_ kind: AdditionalUtilityKind, input: String, options: [String: String]) throws -> WorkbenchResult {
        if kind == .bezier {
            let curve = try BezierCurve.parse(input), css = "cubic-bezier(" + curve.points.map(MathTool.display).joined(separator: ", ") + ")"
            let samples = [0.25,0.5,0.75].map { "At \(Int($0 * 100))% time: \(MathTool.display(curve.value(at: $0) * 100))% progress" }.joined(separator: "\n")
            return .init(text: "transition-timing-function: \(css);\n\n/*\n\(samples)\n*/", status: "Cubic Bézier · drag the handles or edit coordinates", visual: .bezier(curve))
        }
        if kind == .boxShadow {
            let color = try UtilityColor.parse(input)
            func number(_ key: String, _ range: ClosedRange<Double>) throws -> Double {
                guard let value = Double(options[key] ?? "0"), value.isFinite, range.contains(value) else { throw UtilityError("\(key.capitalized) must be \(MathTool.display(range.lowerBound))–\(MathTool.display(range.upperBound)) pixels.") }; return value
            }
            let spec = try BoxShadowSpec(color: color, x: number("x", -100...100), y: number("y", -100...100), blur: number("blur", 0...200), spread: number("spread", -50...100))
            return .init(text: "box-shadow: \(MathTool.display(spec.x))px \(MathTool.display(spec.y))px \(MathTool.display(spec.blur))px \(MathTool.display(spec.spread))px \(color.hex);", status: "Outer shadow · scaled live preview · CSS pixels", visual: .shadow(spec))
        }
        guard kind == .aspectRatio else { throw UtilityError("Choose a layout tool.") }
        let parts = input.lowercased().split { $0 == "x" || $0 == "×" || $0 == ":" || $0.isWhitespace }
        guard parts.count == 2, let width = Int(parts[0]), let height = Int(parts[1]), (1...10_000_000).contains(width), (1...10_000_000).contains(height), let target = Int(options["size"] ?? "1280"), (1...10_000_000).contains(target) else { throw UtilityError("Enter width × height and a target size, each from 1 to 10,000,000 pixels.") }
        func gcd(_ a: Int, _ b: Int) -> Int { var a = a, b = b; while b != 0 { let next = a % b; a = b; b = next }; return a }
        let divisor = gcd(width, height), targetWidth = options["dimension"] != "Height"
        let proportional = Double(target) * Double(targetWidth ? height : width) / Double(targetWidth ? width : height)
        guard proportional >= 0.5, proportional <= 100_000_000 else { throw UtilityError("The proportional dimension would round below 1 or exceed 100 million pixels. Choose a different target size.") }
        let rounded = Int(proportional.rounded()), w = targetWidth ? target : rounded, h = targetWidth ? rounded : target
        let text = "Ratio       \(width / divisor):\(height / divisor)\nSource      \(width) × \(height) px\nTarget      \(w) × \(h) px\nExact \(targetWidth ? "height" : "width")  \(MathTool.display(proportional)) px\n\naspect-ratio: \(width / divisor) / \(height / divisor);"
        return .init(text: text, status: "\(width / divisor):\(height / divisor) · target rounded to the nearest pixel", visual: .aspect(width, height, w, h))
    }
}
