import AppKit

/// A borderless, non-activating panel used for the small action button. It must
/// never steal key focus from the app the user is working in.
final class FloatingButtonPanel: NSPanel {
    init(contentRect: NSRect) {
        super.init(
            contentRect: contentRect,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        isFloatingPanel = true
        level = .statusBar
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        animationBehavior = .utilityWindow
    }

    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

/// A controller-owned tooltip for the non-activating floating toolbar. Native
/// AppKit help tags are not reliably delivered while another app remains key.
final class FloatingTooltipPanel: NSPanel {
    private let label = NSTextField(wrappingLabelWithString: "")
    private let container = FloatingTooltipBackgroundView()

    init() {
        super.init(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        level = NSWindow.Level(rawValue: NSWindow.Level.statusBar.rawValue + 1)
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        ignoresMouseEvents = true
        hidesOnDeactivate = false
        isReleasedWhenClosed = false

        container.wantsLayer = true
        container.layer?.cornerRadius = 6
        label.font = .systemFont(ofSize: 12, weight: .medium)
        label.textColor = .labelColor
        label.maximumNumberOfLines = 3
        label.cell?.truncatesLastVisibleLine = true
        label.preferredMaxLayoutWidth = 320
        label.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 9),
            label.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -9),
            label.topAnchor.constraint(equalTo: container.topAnchor, constant: 6),
            label.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -6),
            label.widthAnchor.constraint(lessThanOrEqualToConstant: 320),
        ])
        contentView = container
    }

    func update(text: String) {
        label.stringValue = text
        container.layoutSubtreeIfNeeded()
        let fittingSize = container.fittingSize
        setContentSize(NSSize(width: ceil(fittingSize.width), height: ceil(fittingSize.height)))
    }

    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

/// Help text stays opaque and follows the app's light/dark appearance.
private final class FloatingTooltipBackgroundView: NSView {
    override func draw(_ dirtyRect: NSRect) {
        NSColor.windowBackgroundColor.setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 6, yRadius: 6).fill()
    }
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        needsDisplay = true
    }
}

/// The interactive popup. It can become key so the user can type a custom
/// instruction without pulling focus away from the app containing the selection.
final class PopupPanel: NSPanel {
    init(contentRect: NSRect) {
        super.init(
            contentRect: contentRect,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        isFloatingPanel = true
        level = .statusBar
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        isOpaque = false
        backgroundColor = .clear
        isMovableByWindowBackground = true
        hasShadow = true
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        animationBehavior = .utilityWindow
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

/// Geometry helpers for placing overlays near a selection while keeping them
/// fully on-screen.
enum ScreenPlacement {
    /// Exact visual centering, with the Dock/menu bar excluded. Unlike
    /// NSWindow.center(), this also centers vertically rather than above center.
    static func centeredFrame(size: CGSize, visibleFrame: CGRect) -> CGRect {
        let fitted = fittedSize(size, visibleFrame: visibleFrame)
        return CGRect(x: visibleFrame.midX - fitted.width / 2,
                      y: visibleFrame.midY - fitted.height / 2,
                      width: fitted.width, height: fitted.height)
    }

    static func fittedSize(_ size: CGSize, visibleFrame: CGRect) -> CGSize {
        CGSize(width: min(size.width, max(1, visibleFrame.width - 12)),
               height: min(size.height, max(1, visibleFrame.height - 12)))
    }

    /// A selection-relative popup must fit in size as well as position.
    /// Without a selection, use the same centered placement as Ask AI.
    static func popupFrame(size: CGSize, anchorRect: CGRect?, visibleFrame: CGRect) -> CGRect {
        guard let reference = anchorRect else { return centeredFrame(size: size, visibleFrame: visibleFrame) }
        let fitted = fittedSize(size, visibleFrame: visibleFrame)
        var origin = CGPoint(x: reference.minX, y: reference.minY - 12 - fitted.height)
        if origin.y < visibleFrame.minY { origin.y = reference.maxY + 12 }
        return CGRect(origin: clamp(origin: origin, size: fitted, visibleFrame: visibleFrame), size: fitted)
    }

    static func screen(containing point: CGPoint) -> NSScreen {
        ScreenCoordinateSpace.screen(containingOrNearestTo: point)
    }

    /// Origin (bottom-left) for the action button, placed just above the end of
    /// the selection, or above-right of the mouse when bounds are unknown.
    static func buttonOrigin(anchorRect: CGRect?, mouse: CGPoint, size: CGSize) -> CGPoint {
        let reference = anchorRect ?? CGRect(x: mouse.x, y: mouse.y, width: 0, height: 0)
        let desired = CGPoint(x: reference.maxX + 8, y: reference.maxY + 8)
        return clamp(origin: desired, size: size, into: screen(containing: CGPoint(x: reference.midX, y: reference.midY)))
    }

    /// Origin (bottom-left) for the popup, preferring just below the selection,
    /// flipping above it when there is not enough room.
    static func popupOrigin(anchorRect: CGRect?, mouse: CGPoint, size: CGSize) -> CGPoint {
        let reference = anchorRect ?? CGRect(x: mouse.x, y: mouse.y, width: 1, height: 1)
        let host = screen(containing: CGPoint(x: reference.midX, y: reference.midY))
        let visible = host.visibleFrame
        let gap: CGFloat = 12

        var origin = CGPoint(x: reference.minX, y: reference.minY - gap - size.height)
        if origin.y < visible.minY {
            // Not enough space below — place above the selection instead.
            origin.y = reference.maxY + gap
        }
        return clamp(origin: origin, size: size, into: host)
    }

    static func clamp(origin: CGPoint, size: CGSize, into screen: NSScreen) -> CGPoint {
        clamp(origin: origin, size: size, visibleFrame: screen.visibleFrame)
    }

    static func clamp(origin: CGPoint, size: CGSize, visibleFrame visible: CGRect) -> CGPoint {
        var x = origin.x
        var y = origin.y
        let minX = visible.minX + 6
        let minY = visible.minY + 6
        let maxX = max(minX, visible.maxX - size.width - 6)
        let maxY = max(minY, visible.maxY - size.height - 6)
        x = min(max(x, minX), maxX)
        y = min(max(y, minY), maxY)
        return CGPoint(x: x, y: y)
    }
}
