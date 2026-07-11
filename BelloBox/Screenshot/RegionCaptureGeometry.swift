import CoreGraphics

enum RegionCaptureGeometry {
    static let dragThreshold: CGFloat = 6
    static let minimumAreaSize: CGFloat = 8

    static func selectionRect(from start: CGPoint, to end: CGPoint) -> CGRect {
        CGRect(
            x: min(start.x, end.x),
            y: min(start.y, end.y),
            width: abs(start.x - end.x),
            height: abs(start.y - end.y)
        )
    }

    static func clampedPoint(_ point: CGPoint, to rect: CGRect) -> CGPoint {
        CGPoint(
            x: min(max(point.x, rect.minX), rect.maxX),
            y: min(max(point.y, rect.minY), rect.maxY)
        )
    }

    static func clampedSelectionRect(from start: CGPoint, to end: CGPoint, bounds: CGRect) -> CGRect {
        selectionRect(
            from: clampedPoint(start, to: bounds),
            to: clampedPoint(end, to: bounds)
        )
    }

    static func localFlippedPointToGlobalCocoa(_ point: CGPoint, screenFrame: CGRect) -> CGPoint {
        CGPoint(x: screenFrame.minX + point.x, y: screenFrame.maxY - point.y)
    }

    static func globalCocoaPointToLocalFlipped(_ point: CGPoint, screenFrame: CGRect) -> CGPoint {
        CGPoint(x: point.x - screenFrame.minX, y: screenFrame.maxY - point.y)
    }

    static func localFlippedRectToGlobalCocoa(_ rect: CGRect, screenFrame: CGRect) -> CGRect {
        CGRect(
            x: screenFrame.minX + rect.minX,
            y: screenFrame.maxY - rect.maxY,
            width: rect.width,
            height: rect.height
        ).standardized
    }

    static func globalCocoaRectToLocalFlipped(_ rect: CGRect, screenFrame: CGRect) -> CGRect {
        CGRect(
            x: rect.minX - screenFrame.minX,
            y: screenFrame.maxY - rect.maxY,
            width: rect.width,
            height: rect.height
        ).standardized
    }
}
