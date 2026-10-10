// Generates deterministic OCR fixture PNGs (rendered text, no captured pixels).
// Usage: swift make-fixtures.swift OUTDIR
import AppKit

let out = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)

func render(_ name: String, width: Int, height: Int, draw: (CGContext) -> Void) {
    let space = CGColorSpace(name: CGColorSpace.sRGB)!
    let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                        space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
    // Top-left origin for drawing text lines.
    ctx.translateBy(x: 0, y: CGFloat(height)); ctx.scaleBy(x: 1, y: -1)
    let graphics = NSGraphicsContext(cgContext: ctx, flipped: true)
    NSGraphicsContext.saveGraphicsState(); NSGraphicsContext.current = graphics
    draw(ctx)
    NSGraphicsContext.restoreGraphicsState()
    let image = ctx.makeImage()!
    let dest = CGImageDestinationCreateWithURL(out.appendingPathComponent(name) as CFURL, "public.png" as CFString, 1, nil)!
    CGImageDestinationAddImage(dest, image, nil); CGImageDestinationFinalize(dest)
}

func text(_ s: String, _ x: CGFloat, _ y: CGFloat, size: CGFloat, bold: Bool = false) {
    let font = bold ? NSFont.boldSystemFont(ofSize: size) : NSFont.systemFont(ofSize: size)
    (s as NSString).draw(at: NSPoint(x: x, y: y), withAttributes: [.font: font, .foregroundColor: NSColor.black])
}

render("paragraphs.png", width: 1200, height: 760) { _ in
    text("Release notes for the parser", 60, 50, size: 30, bold: true)
    let first = ["The parser now reads the token stream once, so nested blocks",
                 "keep their offsets and the tree matches what callers expect.",
                 "Large files open about twice as fast as before."]
    for (i, line) in first.enumerated() { text(line, 60, 150 + CGFloat(i) * 30, size: 20) }
    let second = ["Known issue: a comment at the very end of a file",
                  "is attached to the wrong node when it has no newline."]
    for (i, line) in second.enumerated() { text(line, 60, 330 + CGFloat(i) * 30, size: 20) }
    text("Version 2.4.1 - October 2026", 60, 520, size: 16)
}

render("columns.png", width: 1500, height: 640) { _ in
    let left = ["Left column first line", "Left column second line", "Left column third line", "Left column fourth line"]
    let right = ["Right column first line", "Right column second line", "Right column third line", "Right column fourth line"]
    for (i, line) in left.enumerated() { text(line, 60, 80 + CGFloat(i) * 40, size: 22) }
    for (i, line) in right.enumerated() { text(line, 820, 80 + CGFloat(i) * 40, size: 22) }
    text("A footer spanning the page below both columns", 60, 420, size: 22)
}

render("tall.png", width: 1000, height: 8000) { _ in
    for i in 0..<240 {
        text(String(format: "Line %03d: the quick brown fox jumps over the lazy dog", i + 1), 40, 30 + CGFloat(i) * 33, size: 18)
    }
}

render("mixed.png", width: 1200, height: 420) { _ in
    text("Meeting notes", 60, 50, size: 26, bold: true)
    text("会議は午後三時に始まります。", 60, 140, size: 26)
    text("Bring the quarterly report and the budget.", 60, 220, size: 22)
    text("予算の資料も持ってきてください。", 60, 290, size: 26)
}

// Just over one 3200 px band: read in two overlapping bands; a line falls in the overlap.
render("tall-two-bands.png", width: 700, height: 3600) { _ in
    for i in 0..<60 {
        text(String(format: "Row %02d of the tall page", i + 1), 40, 40 + CGFloat(i) * 58, size: 22)
    }
}

render("blank.png", width: 600, height: 400) { _ in }
print("ok")
