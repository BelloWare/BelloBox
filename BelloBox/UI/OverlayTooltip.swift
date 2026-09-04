import AppKit
import SwiftUI

/// Tooltips for views hosted in Bello Box's non-activating panels (the capture overlay,
/// its editor toolbar, the scroll-capture HUD). AppKit help tags never appear there
/// because another app stays key, so this shows the same small dark tooltip the floating
/// toolbar uses, above the pointer, after a short delay. The popup editor uses it too,
/// so every toolbar looks the same and only one tooltip is ever visible.
@MainActor
final class OverlayTooltipPresenter {
    static let shared = OverlayTooltipPresenter()

    /// Screen rect (Cocoa coordinates) the tooltip must never cover: the region sampled
    /// by scroll-to-capture, which would otherwise capture it. A tooltip that cannot
    /// keep clear of it is not shown at all.
    var exclusionRect: CGRect?

    private var panel: FloatingTooltipPanel?
    private var pending: DispatchWorkItem?
    private var anchor: CGPoint?
    private var owner: UUID?
    private var dismissMonitor: Any?
    private(set) var visibleText: String?

    private init() {}

    /// `owner` identifies the control asking, so that only its own later `update` and
    /// `hide(owner:)` calls affect the tooltip it requested.
    func show(_ text: String, owner: UUID? = nil, delay: TimeInterval = 0.35) {
        installDismissMonitorIfNeeded()
        pending?.cancel()
        self.owner = owner
        let work = DispatchWorkItem { [weak self] in
            self?.present(text, at: NSEvent.mouseLocation)
        }
        pending = work
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: work)
    }

    /// Swaps the text of the tooltip that is showing, or about to show, for the control
    /// under the pointer (a live value such as the line width).
    func update(_ text: String, owner: UUID? = nil) {
        guard owner == nil || owner == self.owner else { return }
        if visibleText != nil, let anchor {
            present(text, at: anchor)
        } else if pending != nil {
            show(text, owner: self.owner)
        }
    }

    /// Hides the tooltip. With an `owner`, only when that control's tooltip is the one
    /// showing or pending, so a sibling view going away never hides another's tooltip.
    func hide(owner: UUID? = nil) {
        guard owner == nil || owner == self.owner else { return }
        pending?.cancel()
        pending = nil
        visibleText = nil
        anchor = nil
        self.owner = nil
        panel?.orderOut(nil)
    }

    private func present(_ text: String, at point: CGPoint) {
        pending = nil
        let tooltip = panel ?? FloatingTooltipPanel()
        // Above the capture overlay (screen saver level) and its HUD (+1). No shadow: its
        // soft edge could otherwise reach into an excluded region.
        tooltip.level = NSWindow.Level(rawValue: NSWindow.Level.screenSaver.rawValue + 2)
        tooltip.hasShadow = false
        panel = tooltip
        tooltip.update(text: text)

        let screen = ScreenPlacement.screen(containing: point)
        guard let origin = Self.origin(
            for: tooltip.frame.size,
            near: point,
            avoiding: exclusionRect,
            visibleFrame: screen.visibleFrame
        ) else {
            // Nowhere to put it without covering the sampled region: no tooltip beats a
            // tooltip in the screenshot.
            visibleText = nil
            anchor = nil
            tooltip.orderOut(nil)
            return
        }
        tooltip.setFrameOrigin(origin)
        tooltip.orderFrontRegardless()
        anchor = point
        visibleText = text
    }

    /// Above the pointer by preference (below it near the top of the screen). With an
    /// `excluded` region, the first of above, below, or pushed sideways past the region
    /// that stays clear of it once clamped to the screen; nil when none does.
    static func origin(for size: CGSize, near point: CGPoint, avoiding excluded: CGRect?, visibleFrame: CGRect) -> CGPoint? {
        let above = CGPoint(x: point.x - size.width / 2, y: point.y + 18)
        let below = CGPoint(x: point.x - size.width / 2, y: point.y - size.height - 18)
        let preferred = above.y + size.height > visibleFrame.maxY - 6 ? [below, above] : [above, below]
        func clamped(_ origin: CGPoint) -> CGPoint {
            ScreenPlacement.clamp(origin: origin, size: size, visibleFrame: visibleFrame)
        }
        guard let excluded else { return clamped(preferred[0]) }
        let clear = excluded.insetBy(dx: -6, dy: -6)
        var candidates = preferred
        for base in preferred {
            candidates.append(CGPoint(x: clear.maxX, y: base.y))
            candidates.append(CGPoint(x: clear.minX - size.width, y: base.y))
        }
        return candidates.map(clamped).first { !CGRect(origin: $0, size: size).intersects(clear) }
    }

    /// Like AppKit help tags, the tooltip goes away as soon as the user clicks or scrolls,
    /// so it never outlives a control that disappears on that click.
    private func installDismissMonitorIfNeeded() {
        guard dismissMonitor == nil else { return }
        dismissMonitor = NSEvent.addLocalMonitorForEvents(
            matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown, .scrollWheel]
        ) { [weak self] event in
            self?.hide()
            return event
        }
    }

#if DEBUG
    func showImmediately(_ text: String, at point: CGPoint) {
        installDismissMonitorIfNeeded()
        pending?.cancel()
        owner = nil
        present(text, at: point)
    }

    var debugPanel: NSPanel? { panel }
#endif
}

private struct OverlayTooltipModifier: ViewModifier {
    let text: String
    @State private var owner = UUID()
    @State private var hovering = false

    func body(content: Content) -> some View {
        content
            .accessibilityHint(text)
            .onHover { inside in
                hovering = inside
                if inside {
                    OverlayTooltipPresenter.shared.show(text, owner: owner)
                } else {
                    OverlayTooltipPresenter.shared.hide(owner: owner)
                }
            }
            .onChange(of: text) { newText in
                if hovering {
                    OverlayTooltipPresenter.shared.update(newText, owner: owner)
                }
            }
            .onDisappear {
                hovering = false
                OverlayTooltipPresenter.shared.hide(owner: owner)
            }
    }
}

extension View {
    /// A tooltip that also works inside Bello Box's non-activating overlay panels. It
    /// replaces `.help`, whose native tag would show up next to it whenever Bello Box
    /// is the active app.
    func overlayTooltip(_ text: String) -> some View {
        modifier(OverlayTooltipModifier(text: text))
    }
}
