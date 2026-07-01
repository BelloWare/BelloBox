import AppKit
import CoreGraphics
import Foundation

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

struct CodableColor: Equatable, Codable {
    var red: CGFloat
    var green: CGFloat
    var blue: CGFloat
    var alpha: CGFloat

    init(red: CGFloat, green: CGFloat, blue: CGFloat, alpha: CGFloat = 1) {
        self.red = red
        self.green = green
        self.blue = blue
        self.alpha = alpha
    }

    init(_ color: NSColor) {
        let converted = color.usingColorSpace(.deviceRGB) ?? color
        red = converted.redComponent
        green = converted.greenComponent
        blue = converted.blueComponent
        alpha = converted.alphaComponent
    }

    var nsColor: NSColor {
        NSColor(calibratedRed: red, green: green, blue: blue, alpha: alpha)
    }

    var cgColor: CGColor { nsColor.cgColor }
}

struct ScreenshotDocument: Identifiable, Equatable {
    let id: UUID
    var baseImage: CGImage
    var scale: CGFloat
    var source: ScreenshotSource
    var annotations: [ScreenshotAnnotation]
    var cropRect: CGRect?
    var ocrResults: [OCRResult]
    var activeOCRResultID: UUID?
    var createdAt: Date

    init(
        id: UUID = UUID(),
        baseImage: CGImage,
        scale: CGFloat,
        source: ScreenshotSource,
        annotations: [ScreenshotAnnotation] = [],
        cropRect: CGRect? = nil,
        ocrResults: [OCRResult] = [],
        activeOCRResultID: UUID? = nil,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.baseImage = baseImage
        self.scale = scale
        self.source = source
        self.annotations = annotations
        self.cropRect = cropRect
        self.ocrResults = ocrResults
        self.activeOCRResultID = activeOCRResultID
        self.createdAt = createdAt
    }

    static func == (lhs: ScreenshotDocument, rhs: ScreenshotDocument) -> Bool {
        lhs.id == rhs.id
            && lhs.baseImage.width == rhs.baseImage.width
            && lhs.baseImage.height == rhs.baseImage.height
            && lhs.scale == rhs.scale
            && lhs.source == rhs.source
            && lhs.annotations == rhs.annotations
            && lhs.cropRect == rhs.cropRect
            && lhs.ocrResults == rhs.ocrResults
            && lhs.activeOCRResultID == rhs.activeOCRResultID
            && lhs.createdAt == rhs.createdAt
    }

    var activeOCRResult: OCRResult? {
        guard let activeOCRResultID else { return nil }
        return ocrResults.first { $0.id == activeOCRResultID }
    }

    var imageSize: CGSize {
        CGSize(width: baseImage.width, height: baseImage.height)
    }
}

struct DisplaySnapshot {
    var displayID: CGDirectDisplayID
    var screenFrame: CGRect
    var scale: CGFloat
    var image: CGImage
}

enum ScreenshotSource: Equatable {
    case area(rect: CGRect, displayID: CGDirectDisplayID?)
    case window(title: String?, ownerName: String?, windowID: UInt32?)
    case display(displayID: CGDirectDisplayID?)
    case scrolling(target: ScrollCaptureTargetSummary, frameCount: Int)
    case importedClipboard
}

enum CaptureTarget: Equatable {
    case area(CaptureArea)
    case window(CaptureWindow)
    case display(CaptureDisplay)
}

struct CaptureArea: Equatable {
    var cocoaRect: CGRect
    var displayID: CGDirectDisplayID?
}

struct CaptureWindow: Equatable, Identifiable {
    var windowID: UInt32
    var title: String?
    var ownerName: String?
    var ownerBundleID: String?
    var ownerProcessID: pid_t?
    var frame: CGRect?
    var captureMode: CaptureWindowCaptureMode = .independentWindow
    var layer: Int? = nil
    var allowsVisibleFrameFallback: Bool = false

    var id: UInt32 { windowID }
}

enum CaptureWindowCaptureMode: Equatable {
    case independentWindow
    case visibleFrame
}

struct CaptureDisplay: Equatable {
    var displayID: CGDirectDisplayID
    var frame: CGRect
}

enum CaptureSelection: Equatable {
    case area(CaptureArea)
    case window(CaptureWindow)
    case display(CaptureDisplay)

    var cocoaRect: CGRect {
        switch self {
        case let .area(area):
            return area.cocoaRect
        case let .window(window):
            return window.frame ?? .zero
        case let .display(display):
            return display.frame
        }
    }
}

enum CaptureSelectionPolicy: Equatable {
    case any
    case areaOnly
    case windowOnly
    case displayOnly
    case areaOrWindow
}

enum ScreenshotCaptureMode: String, CaseIterable, Identifiable {
    case area
    case window
    case screen
    case scrolling

    var id: String { rawValue }

    var label: String {
        switch self {
        case .area: return "Area"
        case .window: return "Window"
        case .screen: return "Screen"
        case .scrolling: return "Scrolling"
        }
    }

    var symbol: String {
        switch self {
        case .area: return "selection.pin.in.out"
        case .window: return "macwindow"
        case .screen: return "display"
        case .scrolling: return "arrow.down.doc"
        }
    }
}

struct ScrollCaptureTargetSummary: Equatable, Codable {
    var title: String?
    var ownerName: String?
    var frame: CGRectCodable?
}

struct ScrollCaptureSession: Identifiable {
    let id: UUID
    var target: ScrollCaptureTarget
    var frames: [ScrollCapturedFrame]
    var direction: ScrollDirection
    var status: ScrollCaptureStatus
    var config: StitchConfig

    init(
        id: UUID = UUID(),
        target: ScrollCaptureTarget,
        frames: [ScrollCapturedFrame] = [],
        direction: ScrollDirection = .down,
        status: ScrollCaptureStatus = .idle,
        config: StitchConfig = .default
    ) {
        self.id = id
        self.target = target
        self.frames = frames
        self.direction = direction
        self.status = status
        self.config = config
    }
}

struct ScrollCapturedFrame: Identifiable, Equatable {
    let id: UUID
    var image: CGImage
    var captureDate: Date
    var targetRect: CGRect

    init(id: UUID = UUID(), image: CGImage, captureDate: Date = Date(), targetRect: CGRect) {
        self.id = id
        self.image = image
        self.captureDate = captureDate
        self.targetRect = targetRect
    }

    static func == (lhs: ScrollCapturedFrame, rhs: ScrollCapturedFrame) -> Bool {
        lhs.id == rhs.id
            && lhs.image.width == rhs.image.width
            && lhs.image.height == rhs.image.height
            && lhs.captureDate == rhs.captureDate
            && lhs.targetRect == rhs.targetRect
    }
}

enum ScrollCaptureTarget: Equatable {
    case area(CaptureArea)
    case window(CaptureWindow)

    var summary: ScrollCaptureTargetSummary {
        switch self {
        case let .area(area):
            return ScrollCaptureTargetSummary(title: "Area", ownerName: nil, frame: CGRectCodable(area.cocoaRect))
        case let .window(window):
            return ScrollCaptureTargetSummary(title: window.title, ownerName: window.ownerName, frame: window.frame.map(CGRectCodable.init))
        }
    }
}

enum ScrollDirection: Equatable, Codable {
    case down
    case up
}

enum ScrollCaptureStatus: Equatable {
    case idle
    case capturing
    case waitingForScroll
    case stitching
    case finished
    case failed(String)
}

struct StitchConfig: Equatable {
    var direction: ScrollDirection
    var minOverlapPx: Int
    var maxOverlapFraction: CGFloat
    var downsampleWidth: Int
    var scoreThreshold: Double
    var removeRepeatedHeaderFooter: Bool
    var maxOutputHeightPx: Int

    static let `default` = StitchConfig(
        direction: .down,
        minOverlapPx: 80,
        maxOverlapFraction: 0.70,
        downsampleWidth: 420,
        scoreThreshold: 0.08,
        removeRepeatedHeaderFooter: true,
        maxOutputHeightPx: 60_000
    )
}

struct StitchResult: Equatable {
    var image: CGImage
    var placements: [FramePlacement]
    var warnings: [String]

    static func == (lhs: StitchResult, rhs: StitchResult) -> Bool {
        lhs.image.width == rhs.image.width
            && lhs.image.height == rhs.image.height
            && lhs.placements == rhs.placements
            && lhs.warnings == rhs.warnings
    }
}

struct FramePlacement: Equatable {
    var frameIndex: Int
    var y: Int
    var overlapWithPrevious: Int
    var confidence: Double
    var croppedTop: Int
    var croppedBottom: Int
}
