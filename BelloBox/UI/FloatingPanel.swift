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
    private let container = NSView()

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
        container.layer?.backgroundColor = NSColor(calibratedWhite: 0.12, alpha: 0.97).cgColor
        container.layer?.cornerRadius = 6
        label.font = .systemFont(ofSize: 12, weight: .medium)
        label.textColor = .white
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
