import Foundation

struct UtilityColor: Equatable {
    var red: Double, green: Double, blue: Double, alpha: Double = 1
    var hex: String {
        let channels = [red, green, blue] + (alpha < 1 ? [alpha] : [])
        return "#" + channels.map { String(format: "%02X", Int(($0 * 255).rounded())) }.joined()
    }
    var rgb: String { "rgb(\(Int((red * 255).rounded())) \(Int((green * 255).rounded())) \(Int((blue * 255).rounded()))" + (alpha < 1 ? " / \(MathTool.display(alpha))" : "") + ")" }
    var hsl: String {
        let hi = max(red, green, blue), lo = min(red, green, blue), delta = hi - lo, light = (hi + lo) / 2
        var hue = 0.0
        if delta > 0 {
            if hi == red { hue = (green - blue) / delta }
            else if hi == green { hue = (blue - red) / delta + 2 }
            else { hue = (red - green) / delta + 4 }
            hue = (hue * 60 + 360).truncatingRemainder(dividingBy: 360)
        }
        let saturation = delta == 0 ? 0 : delta / (1 - abs(2 * light - 1))
        return String(format: "hsl(%.2f %.2f%% %.2f%%", locale: Locale(identifier: "en_US_POSIX"), hue, saturation * 100, light * 100) + (alpha < 1 ? " / \(MathTool.display(alpha))" : "") + ")"
    }
    var luminance: Double {
        func linear(_ c: Double) -> Double { c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4) }
        return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
    }
    static func parse(_ input: String) throws -> Self {
        var text = input.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard text.utf8.count <= 256 else { throw UtilityError("Enter one HEX, RGB, or HSL color.") }
        let names = ["black": "#000000", "white": "#ffffff", "red": "#ff0000", "transparent": "#00000000"]
        text = names[text] ?? text
        if text.hasPrefix("#") {
            var digits = String(text.dropFirst())
            if digits.count == 3 || digits.count == 4 { digits = digits.map { "\($0)\($0)" }.joined() }
            guard [6, 8].contains(digits.count), digits.allSatisfy({ $0.isASCII && $0.isHexDigit }), let value = UInt32(digits, radix: 16) else { throw UtilityError("HEX colors need 3, 4, 6, or 8 hexadecimal digits after #.") }
            let rgb = digits.count == 8 ? value >> 8 : value
            return Self(red: Double((rgb >> 16) & 255) / 255, green: Double((rgb >> 8) & 255) / 255, blue: Double(rgb & 255) / 255, alpha: digits.count == 8 ? Double(value & 255) / 255 : 1)
        }
        guard let open = text.firstIndex(of: "("), text.hasSuffix(")") else { throw UtilityError("Use #B84D16, rgb(184 77 22), or hsl(20 79% 40%).") }
        let name = String(text[..<open])
        guard ["rgb", "rgba", "hsl", "hsla"].contains(name) else { throw UtilityError("Supported functions are rgb(), rgba(), hsl(), and hsla().") }
        let body = text[text.index(after: open)..<text.index(before: text.endIndex)]
        let parts: [String]
        if body.contains(",") {
            guard !body.contains("/") else { throw UtilityError("Use commas or space/slash color syntax, not both.") }
            parts = body.split(separator: ",", omittingEmptySubsequences: false).map { $0.trimmingCharacters(in: .whitespaces) }
        } else {
            let components = body.split(separator: "/", omittingEmptySubsequences: false)
            guard components.count <= 2 else { throw UtilityError("A color can have only one alpha channel.") }
            let channels = components[0].split(whereSeparator: \.isWhitespace).map(String.init)
            guard channels.count == 3 else { throw UtilityError("Provide three color channels before the optional / alpha.") }
            parts = channels + (components.count == 2 ? [components[1].trimmingCharacters(in: .whitespaces)] : [])
        }
        guard parts.count == 3 || parts.count == 4 else { throw UtilityError("Provide three color channels and, optionally, alpha.") }
        func channel(_ s: String, scale: Double) throws -> Double {
            let percent = s.hasSuffix("%"), token = percent ? String(s.dropLast()) : s
            guard let value = Double(token), value.isFinite else { throw UtilityError("Color channels must be finite numbers.") }
            let normalized = value / (percent ? 100 : scale)
            guard (0...1).contains(normalized) else { throw UtilityError("Color channels must be within their range (0–255, 0–100%, or alpha 0–1).") }
            return normalized
        }
        let alpha = try parts.count == 4 ? channel(parts[3], scale: 1) : 1
        if name.hasPrefix("rgb") { return try Self(red: channel(parts[0], scale: 255), green: channel(parts[1], scale: 255), blue: channel(parts[2], scale: 255), alpha: alpha) }
        guard parts[1].hasSuffix("%"), parts[2].hasSuffix("%"), let hue = Double(parts[0].hasSuffix("deg") ? String(parts[0].dropLast(3)) : parts[0]), hue.isFinite else { throw UtilityError("HSL needs a hue in degrees and saturation/lightness percentages.") }
        let saturation = try channel(parts[1], scale: 1), light = try channel(parts[2], scale: 1)
        let h = (hue.truncatingRemainder(dividingBy: 360) + 360).truncatingRemainder(dividingBy: 360) / 60
        let c = (1 - abs(2 * light - 1)) * saturation, x = c * (1 - abs(h.truncatingRemainder(dividingBy: 2) - 1)), m = light - c / 2
        let channels: (Double, Double, Double)
        switch h { case ..<1: channels = (c,x,0); case ..<2: channels = (x,c,0); case ..<3: channels = (0,c,x); case ..<4: channels = (0,x,c); case ..<5: channels = (x,0,c); default: channels = (c,0,x) }
        return Self(red: channels.0 + m, green: channels.1 + m, blue: channels.2 + m, alpha: alpha)
    }
}

enum UtilityVisual {
    case color(UtilityColor)
    case contrast(UtilityColor, UtilityColor, Double)
    case gradient(UtilityColor, UtilityColor, Double)
    case markdown([MarkdownBlock])
    case permissions(Int)
    case table(DataTable)
    case bits(String, String)
    case statistics(StatisticsVisual)
    case aspect(Int, Int, Int, Int)
    case bezier(BezierCurve)
    case shadow(BoxShadowSpec)
}

enum ColorTool {
    static func run(_ kind: AdditionalUtilityKind, input: String, options: [String: String]) throws -> WorkbenchResult {
        let a = try UtilityColor.parse(input)
        if kind == .color {
            let format = options["format"] ?? "All formats"
            let output = format == "HEX" ? a.hex : format == "RGB" ? a.rgb : format == "HSL" ? a.hsl : "\(a.hex)\n\(a.rgb)\n\(a.hsl)"
            return WorkbenchResult(text: output, status: "Color converted · sRGB · \(format)", visual: .color(a))
        }
        let b = try UtilityColor.parse(options[kind == .contrast ? "background" : "end"] ?? "#FFFFFF")
        if kind == .gradient {
            guard let angle = Double(options["angle"] ?? "135"), angle.isFinite, abs(angle) <= 1e6 else { throw UtilityError("Enter a finite angle between −1,000,000° and 1,000,000°.") }
            let normalized = (angle.truncatingRemainder(dividingBy: 360) + 360).truncatingRemainder(dividingBy: 360)
            return WorkbenchResult(text: "background: linear-gradient(\(MathTool.display(normalized))deg, \(a.hex) 0%, \(b.hex) 100%);", status: "Linear gradient · CSS angle", visual: .gradient(a, b, normalized))
        }
        guard a.alpha == 1 && b.alpha == 1 else { throw UtilityError("Use opaque colors for contrast. Transparent colors depend on the surface behind them.") }
        let ratio = (max(a.luminance, b.luminance) + 0.05) / (min(a.luminance, b.luminance) + 0.05)
        func verdict(_ threshold: Double) -> String { ratio >= threshold ? "Pass" : "Fail" }
        return WorkbenchResult(text: String(format: "Contrast %.2f:1", locale: Locale(identifier: "en_US_POSIX"), ratio) + "\n\nAA normal text (4.5:1): \(verdict(4.5))\nAA large text (3:1): \(verdict(3))\nAAA normal text (7:1): \(verdict(7))\nAAA large text (4.5:1): \(verdict(4.5))\n\nLarge text: ≥18 pt, or ≥14 pt bold. Thresholds use the unrounded ratio.", status: ratio >= 4.5 ? "AA normal text passes" : "AA normal text fails", visual: .contrast(a, b, ratio))
    }
}

struct MarkdownBlock {
    enum Kind { case heading(Int), paragraph, bullet, quote, code, rule }
    let kind: Kind, text: String
}

enum MarkdownTool {
    static func escapeHTML(_ text: String) -> String {
        text.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;").replacingOccurrences(of: ">", with: "&gt;").replacingOccurrences(of: "\"", with: "&quot;").replacingOccurrences(of: "'", with: "&#39;")
    }
    /// Small, deliberately non-executing Markdown subset. Raw HTML is always escaped.
    static func inlineHTML(_ text: String) -> String {
        let pattern = "`([^`]+)`|\\*\\*([^*]+)\\*\\*|\\*([^*]+)\\*|\\[([^\\]]+)\\]\\(([^\\s)]+)\\)"
        let regex = try! NSRegularExpression(pattern: pattern)
        let ns = text as NSString
        var output = "", offset = 0
        for match in regex.matches(in: text, range: NSRange(location: 0, length: ns.length)) {
            output += escapeHTML(ns.substring(with: NSRange(location: offset, length: match.range.location - offset)))
            func value(_ index: Int) -> String? { let r = match.range(at: index); return r.location == NSNotFound ? nil : ns.substring(with: r) }
            if let code = value(1) { output += "<code>\(escapeHTML(code))</code>" }
            else if let bold = value(2) { output += "<strong>\(escapeHTML(bold))</strong>" }
            else if let italic = value(3) { output += "<em>\(escapeHTML(italic))</em>" }
            else if let label = value(4), let url = value(5), let scheme = URL(string: url)?.scheme?.lowercased(), ["https", "http", "mailto"].contains(scheme) {
                output += "<a href=\"\(escapeHTML(url))\" rel=\"noreferrer\">\(escapeHTML(label))</a>"
            } else { output += escapeHTML(ns.substring(with: match.range)) }
            offset = NSMaxRange(match.range)
        }
        output += escapeHTML(ns.substring(from: offset)); return output
    }
    static func render(_ input: String) throws -> WorkbenchResult {
        // CommonMark line endings: CRLF is one break, including in fenced code.
        let normalized = input.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n")
        let lines = normalized.components(separatedBy: "\n")
        guard lines.count <= 4_000 else { throw UtilityError("Preview up to 4,000 Markdown lines at a time.") }
        var blocks: [MarkdownBlock] = [], code: [String] = [], fenced = false, paragraph: [String] = []
        func flush() { if !paragraph.isEmpty { blocks.append(.init(kind: .paragraph, text: paragraph.joined(separator: "\n"))); paragraph.removeAll() } }
        for line in lines {
            try Task.checkCancellation()
            if line.hasPrefix("```") {
                flush()
                if fenced { blocks.append(.init(kind: .code, text: code.joined(separator: "\n"))); code.removeAll() }
                fenced.toggle(); continue
            }
            if fenced { code.append(line); continue }
            let heading = line.prefix(while: { $0 == "#" }).count
            if (1...6).contains(heading), line.dropFirst(heading).hasPrefix(" ") { flush(); blocks.append(.init(kind: .heading(heading), text: String(line.dropFirst(heading + 1)))) }
            else if line == "---" || line == "***" { flush(); blocks.append(.init(kind: .rule, text: "")) }
            else if line.hasPrefix("- ") || line.hasPrefix("* ") { flush(); blocks.append(.init(kind: .bullet, text: String(line.dropFirst(2)))) }
            else if line.hasPrefix("> ") { flush(); blocks.append(.init(kind: .quote, text: String(line.dropFirst(2)))) }
            else if line.trimmingCharacters(in: .whitespaces).isEmpty { flush() }
            else { paragraph.append(line) }
        }
        flush(); if fenced { blocks.append(.init(kind: .code, text: code.joined(separator: "\n"))) }
        let html = blocks.map { block -> String in
            let inline = inlineHTML(block.text)
            switch block.kind {
            case .heading(let n): return "<h\(n)>\(inline)</h\(n)>"
            case .paragraph: return "<p>\(inline)</p>"
            case .bullet: return "<ul><li>\(inline)</li></ul>"
            case .quote: return "<blockquote>\(inline)</blockquote>"
            case .code: return "<pre><code>\(escapeHTML(block.text))</code></pre>"
            case .rule: return "<hr>"
            }
        }.joined(separator: "\n")
        return WorkbenchResult(text: html, status: "Offline preview · Copy exports HTML · raw HTML stays text", visual: .markdown(blocks))
    }
}
