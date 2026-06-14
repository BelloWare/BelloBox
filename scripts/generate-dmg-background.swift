#!/usr/bin/env swift

import AppKit
import Foundation

enum BackgroundError: Error {
    case missingOutputPath
    case imageEncodingFailed
}

let arguments = CommandLine.arguments
guard arguments.count == 2 || arguments.count == 3 else {
    throw BackgroundError.missingOutputPath
}

let outputURL = URL(fileURLWithPath: arguments[1])
let canvasSize = NSSize(width: 660, height: 400)
let scale = CGFloat(arguments.count == 3 ? (Double(arguments[2]) ?? 1.0) : 1.0)
guard let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: Int(canvasSize.width * scale),
    pixelsHigh: Int(canvasSize.height * scale),
    bitsPerSample: 8,
    samplesPerPixel: 4,
    hasAlpha: true,
    isPlanar: false,
    colorSpaceName: .deviceRGB,
    bytesPerRow: 0,
    bitsPerPixel: 0
) else {
    throw BackgroundError.imageEncodingFailed
}

bitmap.size = canvasSize
guard let context = NSGraphicsContext(bitmapImageRep: bitmap) else {
    throw BackgroundError.imageEncodingFailed
}
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = context

let bounds = NSRect(origin: .zero, size: canvasSize)
let baseGradient = NSGradient(
    starting: NSColor(calibratedRed: 0.965, green: 0.965, blue: 0.973, alpha: 1.0),
    ending: NSColor(calibratedRed: 0.952, green: 0.954, blue: 0.965, alpha: 1.0)
)
baseGradient?.draw(in: bounds, angle: 270)

NSGraphicsContext.current?.saveGraphicsState()
let chevronShadow = NSShadow()
chevronShadow.shadowBlurRadius = 6
chevronShadow.shadowOffset = NSSize(width: 0, height: -1)
chevronShadow.shadowColor = NSColor(calibratedWhite: 0.0, alpha: 0.08)
chevronShadow.set()

let chevron = NSBezierPath()
chevron.lineWidth = 7
chevron.lineCapStyle = .round
chevron.lineJoinStyle = .round
chevron.move(to: NSPoint(x: 323, y: 184))
chevron.line(to: NSPoint(x: 343, y: 204))
chevron.line(to: NSPoint(x: 323, y: 224))
NSColor(calibratedWhite: 0.18, alpha: 0.92).setStroke()
chevron.stroke()
NSGraphicsContext.current?.restoreGraphicsState()
NSGraphicsContext.restoreGraphicsState()

guard let pngData = bitmap.representation(using: .png, properties: [:]) else {
    throw BackgroundError.imageEncodingFailed
}

try FileManager.default.createDirectory(at: outputURL.deletingLastPathComponent(), withIntermediateDirectories: true)
try pngData.write(to: outputURL, options: .atomic)
