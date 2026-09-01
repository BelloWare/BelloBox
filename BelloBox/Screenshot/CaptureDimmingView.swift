import AppKit

/// Layer-hosting chrome shared by the capture overlays: an optional frozen display
/// snapshot, four dim bands around the active selection, the selection border and the
/// size label.
///
/// Everything here is a plain Core Animation layer. Moving the selection only changes a
/// few GPU-composited quads, which avoids re-rasterising a full-screen `draw(_:)` view on
/// every mouse event. The old drawing path flickered while dragging and could leave a
/// large Retina display without its dim layer at all.
final class CaptureDimmingView: NSView {
    static let dimColor = NSColor.black.withAlphaComponent(0.34)
    static let selectionColor = NSColor(calibratedRed: 0.95, green: 0.42, blue: 0.08, alpha: 1)
    static let labelHeight: CGFloat = 20

    /// The size badge shown next to a selection, in device pixels ("640 × 400").
    static func sizeLabel(for selection: CGRect, scale: CGFloat) -> String {
        "\(Int(selection.width * scale)) × \(Int(selection.height * scale))"
    }

    private let snapshotLayer = CALayer()
    private let dimLayers: [CALayer] = (0..<4).map { _ in CALayer() }
    private let borderLayer = CAShapeLayer()
    private let labelContainer = CALayer()
    private let labelLayer = CATextLayer()
    private let contentsScale: CGFloat

    /// The dim band frames in this view's coordinates (bottom-left origin).
    private(set) var dimBandFrames: [CGRect] = []
    /// The selection cut-out in this view's coordinates, or nil when everything is dimmed.
    private(set) var selectionFrame: CGRect?
    private var borderWidth: CGFloat = 2
    private var labelText: String?

#if DEBUG
    /// The four dim band layers, for tests that assert the dim is really attached.
    var debugDimLayers: [CALayer] { dimLayers }
#endif

    var snapshotImage: CGImage? {
        didSet {
            performWithoutAnimation {
                snapshotLayer.contents = snapshotImage
                snapshotLayer.isHidden = snapshotImage == nil
            }
        }
    }

    init(frame: CGRect, contentsScale: CGFloat) {
        self.contentsScale = max(contentsScale, 1)
        super.init(frame: frame)

        let root = CALayer()
        root.masksToBounds = true
        layer = root
        wantsLayer = true
        autoresizingMask = [.width, .height]

        snapshotLayer.contentsGravity = .resize
        snapshotLayer.contentsScale = self.contentsScale
        snapshotLayer.minificationFilter = .trilinear
        snapshotLayer.isHidden = true
        root.addSublayer(snapshotLayer)

        for dimLayer in dimLayers {
            dimLayer.backgroundColor = Self.dimColor.cgColor
            root.addSublayer(dimLayer)
        }

        borderLayer.fillColor = nil
        borderLayer.strokeColor = Self.selectionColor.cgColor
        borderLayer.lineWidth = borderWidth
        borderLayer.contentsScale = self.contentsScale
        borderLayer.isHidden = true
        root.addSublayer(borderLayer)

        labelContainer.backgroundColor = NSColor.black.withAlphaComponent(0.55).cgColor
        labelContainer.cornerRadius = 4
        labelContainer.isHidden = true
        labelLayer.contentsScale = self.contentsScale
        labelLayer.alignmentMode = .center
        labelLayer.truncationMode = .end
        labelLayer.isWrapped = false
        labelContainer.addSublayer(labelLayer)
        root.addSublayer(labelContainer)

        applyGeometry()
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var isFlipped: Bool { false }
    override var isOpaque: Bool { false }

    /// The chrome is purely decorative; mouse events belong to the overlay view behind it.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        applyGeometry()
    }

    /// Converts between the flipped overlay coordinates (top-left origin) and this view's
    /// bottom-left origin. The conversion is its own inverse.
    static func flip(_ rect: CGRect, height: CGFloat) -> CGRect {
        let rect = rect.standardized
        return CGRect(x: rect.minX, y: height - rect.maxY, width: rect.width, height: rect.height)
    }

    /// `selection` is expressed in this view's own (bottom-left origin) coordinates.
    func update(selection: CGRect?, borderWidth: CGFloat = 2, label: String? = nil) {
        var clipped = selection?.standardized.intersection(bounds)
        if let candidate = clipped, candidate.isNull || candidate.isEmpty {
            clipped = nil
        }
        selectionFrame = clipped
        self.borderWidth = borderWidth
        labelText = label
        applyGeometry()
    }

    private func applyGeometry() {
        performWithoutAnimation {
            snapshotLayer.frame = bounds
            let bands = CaptureOverlayDimGeometry.bands(bounds: bounds, selection: selectionFrame)
            dimBandFrames = bands
            for (dimLayer, band) in zip(dimLayers, bands) {
                dimLayer.frame = band
                dimLayer.isHidden = band.isEmpty
            }

            guard let selection = selectionFrame else {
                borderLayer.isHidden = true
                labelContainer.isHidden = true
                return
            }
            borderLayer.isHidden = false
            borderLayer.frame = bounds
            borderLayer.lineWidth = borderWidth
            borderLayer.path = CGPath(rect: selection, transform: nil)
            layoutLabel(around: selection)
        }
    }

    private func layoutLabel(around selection: CGRect) {
        guard let text = labelText, !text.isEmpty else {
            labelContainer.isHidden = true
            return
        }
        let font = NSFont.systemFont(ofSize: 12, weight: .semibold)
        let attributed = NSAttributedString(
            string: text,
            attributes: [.font: font, .foregroundColor: NSColor.white]
        )
        let textSize = attributed.size()
        let width = ceil(textSize.width) + 12
        let height = Self.labelHeight
        var origin = CGPoint(x: selection.minX + 8, y: selection.maxY + 4)
        if origin.y + height > bounds.maxY - 4 {
            origin.y = selection.maxY - height - 6
        }
        origin.x = min(max(origin.x, bounds.minX + 4), max(bounds.minX + 4, bounds.maxX - width - 4))

        labelContainer.isHidden = false
        labelContainer.frame = CGRect(x: origin.x, y: origin.y, width: width, height: height)
        labelLayer.string = attributed
        let textHeight = ceil(textSize.height)
        labelLayer.frame = CGRect(x: 0, y: (height - textHeight) / 2, width: width, height: textHeight)
    }

    private func performWithoutAnimation(_ body: () -> Void) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        body()
        CATransaction.commit()
    }
}
