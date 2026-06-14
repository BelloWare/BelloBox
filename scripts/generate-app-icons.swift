#!/usr/bin/env swift

// Generates the BelloBox AppIcon PNG set into the asset catalog.
// Usage: swift scripts/generate-app-icons.swift [output-appiconset-dir] [source-png]
//
// The source image should be a 1024x1024 square PNG. The default source is the
// selected BelloBox "B toolbox" icon generated during the 0.0.14 icon refresh.

import AppKit
import Foundation

let defaultOut = "BelloBox/Assets.xcassets/AppIcon.appiconset"
let defaultSource = "scripts/assets/bellobox-icon-source.png"

let outDir = CommandLine.arguments.count >= 2 ? CommandLine.arguments[1] : defaultOut
let sourcePath = CommandLine.arguments.count >= 3 ? CommandLine.arguments[2] : defaultSource
let outURL = URL(fileURLWithPath: outDir)

guard let source = NSImage(contentsOfFile: sourcePath) else {
    fputs("Couldn't read source icon at \(sourcePath)\n", stderr)
    exit(1)
}

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
    let context = NSGraphicsContext.current!
    context.imageInterpolation = .high

    let full = NSRect(x: 0, y: 0, width: size, height: size)
    NSColor.clear.set()
    full.fill()
    source.draw(in: full, from: .zero, operation: .copy, fraction: 1.0)

    NSGraphicsContext.current = nil
    NSGraphicsContext.restoreGraphicsState()
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
print("Wrote \(slots.count) icon files to \(outDir) from \(sourcePath)")
