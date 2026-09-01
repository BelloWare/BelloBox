import CoreGraphics

/// The eight grab points around a capture selection.
enum SelectionHandle: CaseIterable, Hashable {
    case topLeft
    case top
    case topRight
    case right
    case bottomRight
    case bottom
    case bottomLeft
    case left

    var movesLeftEdge: Bool { self == .topLeft || self == .left || self == .bottomLeft }
    var movesRightEdge: Bool { self == .topRight || self == .right || self == .bottomRight }
    var movesTopEdge: Bool { self == .topLeft || self == .top || self == .topRight }
    var movesBottomEdge: Bool { self == .bottomLeft || self == .bottom || self == .bottomRight }
    var isCorner: Bool { movesLeftEdge != movesRightEdge && movesTopEdge != movesBottomEdge }
}

/// Pure geometry for adjusting a locked capture selection. All rectangles use the
/// overlay's flipped local coordinates (origin at the top-left of the display).
enum SelectionResizeGeometry {
    static func handlePosition(_ handle: SelectionHandle, in rect: CGRect) -> CGPoint {
        switch handle {
        case .topLeft: return CGPoint(x: rect.minX, y: rect.minY)
        case .top: return CGPoint(x: rect.midX, y: rect.minY)
        case .topRight: return CGPoint(x: rect.maxX, y: rect.minY)
        case .right: return CGPoint(x: rect.maxX, y: rect.midY)
        case .bottomRight: return CGPoint(x: rect.maxX, y: rect.maxY)
        case .bottom: return CGPoint(x: rect.midX, y: rect.maxY)
        case .bottomLeft: return CGPoint(x: rect.minX, y: rect.maxY)
        case .left: return CGPoint(x: rect.minX, y: rect.midY)
        }
    }

    /// Resizes `start` by dragging `handle` by `translation`, keeping every edge inside
    /// `bounds` and never letting the selection collapse below `minimumSize`.
    static func resizedRect(
        from start: CGRect,
        handle: SelectionHandle,
        translation: CGSize,
        bounds: CGRect,
        minimumSize: CGFloat
    ) -> CGRect {
        let start = start.standardized
        let bounds = bounds.standardized
        let minimum = max(1, minimumSize)
        var minX = start.minX
        var maxX = start.maxX
        var minY = start.minY
        var maxY = start.maxY

        if handle.movesLeftEdge {
            minX = clamp(start.minX + translation.width, lower: bounds.minX, upper: start.maxX - minimum)
        }
        if handle.movesRightEdge {
            maxX = clamp(start.maxX + translation.width, lower: start.minX + minimum, upper: bounds.maxX)
        }
        if handle.movesTopEdge {
            minY = clamp(start.minY + translation.height, lower: bounds.minY, upper: start.maxY - minimum)
        }
        if handle.movesBottomEdge {
            maxY = clamp(start.maxY + translation.height, lower: start.minY + minimum, upper: bounds.maxY)
        }

        return CGRect(x: minX, y: minY, width: maxX - minX, height: maxY - minY).standardized
    }

    /// Moves `start` by `translation` without letting it leave `bounds`.
    static func movedRect(from start: CGRect, translation: CGSize, bounds: CGRect) -> CGRect {
        let start = start.standardized
        let bounds = bounds.standardized
        let width = min(start.width, bounds.width)
        let height = min(start.height, bounds.height)
        let x = clamp(start.minX + translation.width, lower: bounds.minX, upper: bounds.maxX - width)
        let y = clamp(start.minY + translation.height, lower: bounds.minY, upper: bounds.maxY - height)
        return CGRect(x: x, y: y, width: width, height: height)
    }

    /// Keeps `rect` inside `bounds` and at least `minimumSize` on each axis, growing it in
    /// place when a drag produced a degenerate rectangle.
    static func clamped(_ rect: CGRect, in bounds: CGRect, minimumSize: CGSize) -> CGRect {
        let bounds = bounds.standardized
        guard bounds.width > 0, bounds.height > 0 else { return bounds }
        var result = rect.standardized.intersection(bounds)
        if result.isNull || result.isEmpty {
            let origin = CGPoint(
                x: clamp(rect.standardized.minX, lower: bounds.minX, upper: bounds.maxX),
                y: clamp(rect.standardized.minY, lower: bounds.minY, upper: bounds.maxY)
            )
            result = CGRect(origin: origin, size: .zero)
        }
        let minWidth = min(max(1, minimumSize.width), bounds.width)
        let minHeight = min(max(1, minimumSize.height), bounds.height)
        if result.width < minWidth {
            let x = clamp(result.minX, lower: bounds.minX, upper: bounds.maxX - minWidth)
            result = CGRect(x: x, y: result.minY, width: minWidth, height: result.height)
        }
        if result.height < minHeight {
            let y = clamp(result.minY, lower: bounds.minY, upper: bounds.maxY - minHeight)
            result = CGRect(x: result.minX, y: y, width: result.width, height: minHeight)
        }
        return result
    }

    private static func clamp(_ value: CGFloat, lower: CGFloat, upper: CGFloat) -> CGFloat {
        guard lower <= upper else { return lower }
        return min(max(value, lower), upper)
    }
}

/// Geometry for the four dim bands that surround a selection without overlapping it.
enum CaptureOverlayDimGeometry {
    /// Returns exactly four rectangles (top, bottom, left, right). When there is no
    /// selection the first band covers everything and the rest are empty.
    static func bands(bounds: CGRect, selection: CGRect?) -> [CGRect] {
        let bounds = bounds.standardized
        guard let selection = selection?.standardized.intersection(bounds),
              !selection.isNull, !selection.isEmpty
        else {
            return [bounds, .zero, .zero, .zero]
        }
        let top = CGRect(x: bounds.minX, y: bounds.minY, width: bounds.width, height: selection.minY - bounds.minY)
        let bottom = CGRect(x: bounds.minX, y: selection.maxY, width: bounds.width, height: bounds.maxY - selection.maxY)
        let left = CGRect(x: bounds.minX, y: selection.minY, width: selection.minX - bounds.minX, height: selection.height)
        let right = CGRect(x: selection.maxX, y: selection.minY, width: bounds.maxX - selection.maxX, height: selection.height)
        return [top, bottom, left, right]
    }
}
