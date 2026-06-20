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

/// A compact editor anchored to the captured screenshot region. It can accept
/// drawing input while staying above the source app.
final class InlineScreenshotEditorPanel: NSPanel {
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
        NSScreen.screens.first { $0.frame.contains(point) } ?? NSScreen.main ?? NSScreen.screens[0]
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
        let visible = screen.visibleFrame
        var x = origin.x
        var y = origin.y
        x = min(max(x, visible.minX + 6), visible.maxX - size.width - 6)
        y = min(max(y, visible.minY + 6), visible.maxY - size.height - 6)
        return CGPoint(x: x, y: y)
    }
}
