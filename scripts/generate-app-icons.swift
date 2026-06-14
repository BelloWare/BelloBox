#!/usr/bin/env swift

// Generates the BelloBox AppIcon PNG set into the asset catalog.
// Usage: swift scripts/generate-app-icons.swift [output-appiconset-dir]
// The icon: warm rounded tile (belloware orange) with a white "selected text"
// card and an accent sparkle, evoking AI acting on selected text.

import AppKit
import Foundation

let defaultOut = "BelloBox/Assets.xcassets/AppIcon.appiconset"
let outDir = CommandLine.arguments.count >= 2 ? CommandLine.arguments[1] : defaultOut
let outURL = URL(fileURLWithPath: outDir)
try FileManager.default.createDirectory(at: outURL, withIntermediateDirectories: true)

func render(_ pixels: Int) -> Data {
    let size = CGFloat(pixels)
    guard let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: pixels,
        pixelsHigh: pixels,
        bitsPerSample: 8,
        samplesPerPixel: 4,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: .deviceRGB,
        bytesPerRow: 0,
        bitsPerPixel: 0
    ) else { fatalError("bitmap") }

    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    let ctx = NSGraphicsContext.current!.cgContext

    let full = NSRect(x: 0, y: 0, width: size, height: size)
    NSColor.clear.set()
    full.fill()

    // Rounded tile with macOS-style continuous corners and inset.
    let inset = size * 0.085
    let tile = full.insetBy(dx: inset, dy: inset)
    let corner = tile.width * 0.225
    let tilePath = NSBezierPath(roundedRect: tile, xRadius: corner, yRadius: corner)
    tilePath.addClip()

    let gradient = NSGradient(colors: [
        NSColor(calibratedRed: 0.93, green: 0.58, blue: 0.18, alpha: 1.0),
        NSColor(calibratedRed: 0.79, green: 0.40, blue: 0.09, alpha: 1.0),
    ])
    gradient?.draw(in: tile, angle: -90)

    NSGraphicsContext.current!.restoreGraphicsState()
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    _ = ctx

    // White "card" representing a text selection.
    let cardW = tile.width * 0.52
    let cardH = tile.height * 0.40
    let cardX = tile.minX + tile.width * 0.20
    let cardY = tile.minY + tile.height * 0.30
    let card = NSRect(x: cardX, y: cardY, width: cardW, height: cardH)
    let cardRadius = card.height * 0.16
    let shadow = NSShadow()
    shadow.shadowBlurRadius = size * 0.03
    shadow.shadowOffset = NSSize(width: 0, height: -size * 0.012)
    shadow.shadowColor = NSColor(calibratedWhite: 0, alpha: 0.18)
    shadow.set()
    NSColor.white.setFill()
    NSBezierPath(roundedRect: card, xRadius: cardRadius, yRadius: cardRadius).fill()

    // Text lines inside the card.
    let lineShadow = NSShadow()
    lineShadow.shadowColor = .clear
    lineShadow.set()
    let lineColor = NSColor(calibratedRed: 0.86, green: 0.52, blue: 0.16, alpha: 0.9)
    let lineH = card.height * 0.12
    let lineX = card.minX + card.width * 0.13
    let widths: [CGFloat] = [0.74, 0.58, 0.66]
    for (i, w) in widths.enumerated() {
        let ly = card.maxY - card.height * 0.26 - CGFloat(i) * (lineH + card.height * 0.12)
        let line = NSRect(x: lineX, y: ly, width: card.width * w, height: lineH)
        (i == widths.count - 1 ? lineColor.withAlphaComponent(0.45) : lineColor).setFill()
        NSBezierPath(roundedRect: line, xRadius: lineH / 2, yRadius: lineH / 2).fill()
    }

    // Accent sparkle (four-point star) over the top-right of the card.
    func sparkle(center: CGPoint, radius r: CGFloat, color: NSColor) {
        let p = NSBezierPath()
        let waist = r * 0.30
        p.move(to: CGPoint(x: center.x, y: center.y + r))
        p.line(to: CGPoint(x: center.x + waist, y: center.y + waist))
        p.line(to: CGPoint(x: center.x + r, y: center.y))
        p.line(to: CGPoint(x: center.x + waist, y: center.y - waist))
        p.line(to: CGPoint(x: center.x, y: center.y - r))
        p.line(to: CGPoint(x: center.x - waist, y: center.y - waist))
        p.line(to: CGPoint(x: center.x - r, y: center.y))
        p.line(to: CGPoint(x: center.x - waist, y: center.y + waist))
        p.close()
        color.setFill()
        p.fill()
    }
    let starShadow = NSShadow()
    starShadow.shadowBlurRadius = size * 0.02
    starShadow.shadowOffset = NSSize(width: 0, height: -size * 0.006)
    starShadow.shadowColor = NSColor(calibratedRed: 0.5, green: 0.22, blue: 0.0, alpha: 0.25)
    starShadow.set()
    sparkle(center: CGPoint(x: card.maxX - card.width * 0.02, y: card.maxY + card.height * 0.05),
            radius: tile.width * 0.105, color: .white)
    sparkle(center: CGPoint(x: card.maxX + card.width * 0.16, y: card.maxY - card.height * 0.16),
            radius: tile.width * 0.05, color: NSColor.white.withAlphaComponent(0.85))

    NSGraphicsContext.current!.restoreGraphicsState()
    return bitmap.representation(using: .png, properties: [:])!
}

// (filename, pixel size) for each macOS AppIcon slot.
let slots: [(String, Int)] = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
]

var cache: [Int: Data] = [:]
for (name, px) in slots {
    let data = cache[px] ?? render(px)
    cache[px] = data
    try data.write(to: outURL.appendingPathComponent(name), options: .atomic)
}

let contents = """
{
  "images" : [
    { "filename" : "icon_16x16.png", "idiom" : "mac", "scale" : "1x", "size" : "16x16" },
    { "filename" : "icon_16x16@2x.png", "idiom" : "mac", "scale" : "2x", "size" : "16x16" },
    { "filename" : "icon_32x32.png", "idiom" : "mac", "scale" : "1x", "size" : "32x32" },
    { "filename" : "icon_32x32@2x.png", "idiom" : "mac", "scale" : "2x", "size" : "32x32" },
    { "filename" : "icon_128x128.png", "idiom" : "mac", "scale" : "1x", "size" : "128x128" },
    { "filename" : "icon_128x128@2x.png", "idiom" : "mac", "scale" : "2x", "size" : "128x128" },
    { "filename" : "icon_256x256.png", "idiom" : "mac", "scale" : "1x", "size" : "256x256" },
    { "filename" : "icon_256x256@2x.png", "idiom" : "mac", "scale" : "2x", "size" : "256x256" },
    { "filename" : "icon_512x512.png", "idiom" : "mac", "scale" : "1x", "size" : "512x512" },
    { "filename" : "icon_512x512@2x.png", "idiom" : "mac", "scale" : "2x", "size" : "512x512" }
  ],
  "info" : { "author" : "xcode", "version" : 1 }
}
"""
try contents.write(to: outURL.appendingPathComponent("Contents.json"), atomically: true, encoding: .utf8)
print("Wrote \(slots.count) icon files to \(outDir)")
