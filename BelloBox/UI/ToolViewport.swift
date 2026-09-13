import AppKit
import SwiftUI

/// Keep the native window inside the display while making every control
/// reachable on displays smaller than a tool's usable layout. The scroll view
/// has no active axes at normal sizes, leaving the tool's own canvas/editor
/// scrolling in charge.
struct ToolViewport<Content: View>: View {
    let minimumSize: CGSize
    let content: Content

    init(minimumSize: CGSize, @ViewBuilder content: () -> Content) {
        self.minimumSize = minimumSize
        self.content = content()
    }

    var body: some View {
        GeometryReader { geometry in
            let layout = ToolViewportLayout(minimum: minimumSize, available: geometry.size,
                                            scrollerWidth: NSScroller.preferredScrollerStyle == .legacy
                                                ? NSScroller.scrollerWidth(for: .regular, scrollerStyle: .legacy) : 0)
            ScrollView(layout.axes) {
                content.frame(width: layout.contentSize.width, height: layout.contentSize.height)
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
        }
        // The hosted tool owns its glass/solid surface. An opaque viewport
        // underneath would cover the desktop through translucent materials.
    }

}

struct ToolViewportLayout {
    let contentSize: CGSize
    let axes: Axis.Set

    init(minimum: CGSize, available: CGSize, scrollerWidth: CGFloat) {
        var horizontal = false
        var vertical = false
        // A legacy scroller can make the other axis overflow as well.
        for _ in 0..<3 {
            horizontal = minimum.width > available.width - (vertical ? scrollerWidth : 0)
            vertical = minimum.height > available.height - (horizontal ? scrollerWidth : 0)
        }
        axes = [horizontal ? .horizontal : [], vertical ? .vertical : []]
        contentSize = CGSize(width: max(minimum.width, available.width - (vertical ? scrollerWidth : 0)),
                             height: max(minimum.height, available.height - (horizontal ? scrollerWidth : 0)))
    }
}
