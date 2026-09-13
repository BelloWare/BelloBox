import AppKit
import SwiftUI

/// Full workspace windows share native controls; floating tool popups keep their
/// non-activating panel behavior and the shared PopupHeader.
enum AppWindowChrome {
    static let styleMask: NSWindow.StyleMask = [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView]

    static func apply(to window: NSWindow, title: String) {
        window.styleMask = styleMask
        window.title = title
        window.titlebarAppearsTransparent = true
        window.backgroundColor = .clear
        window.isOpaque = false
        window.isReleasedWhenClosed = false
    }

    /// Full-size content includes the title bar. Preserve the requested usable
    /// area below it, including the minimum size used by ToolViewport.
    static func size(_ window: NSWindow, content: CGSize, minimum: CGSize) {
        // A newly created NSHostingController window can still have a zero
        // frame. Give AppKit a real layout before reading the native inset.
        window.setContentSize(content)
        let titlebarHeight = max(0, window.frame.height - window.contentLayoutRect.height)
        window.setContentSize(CGSize(width: content.width, height: content.height + titlebarHeight))
        window.contentMinSize = CGSize(width: minimum.width, height: minimum.height + titlebarHeight)
    }

    /// Use the invocation display, including its title bar in the fitted frame.
    /// Existing windows keep their position unless a display change strands them.
    static func place(_ window: NSWindow, on screen: NSScreen? = nil, centered: Bool) {
        let host = screen ?? ScreenPlacement.screen(containing: NSEvent.mouseLocation)
        let size = ScreenPlacement.fittedSize(window.frame.size, visibleFrame: host.visibleFrame)
        let fitted = centered
            ? ScreenPlacement.centeredFrame(size: size, visibleFrame: host.visibleFrame)
            : CGRect(origin: ScreenPlacement.clamp(origin: window.frame.origin, size: size, into: host), size: size)
        let contentSize = window.contentRect(forFrameRect: fitted).size
        window.contentMinSize = NSSize(width: min(window.contentMinSize.width, contentSize.width),
                                       height: min(window.contentMinSize.height, contentSize.height))
        window.setFrame(fitted, display: false)
    }
}

/// Use the shipped orange toolbox artwork wherever the app identifies itself.
enum AppBrandAssets {
    static let icon: NSImage = {
        let name = ((Bundle.main.object(forInfoDictionaryKey: "CFBundleIconFile") as? String) ?? "AppIcon")
            .replacingOccurrences(of: ".icns", with: "")
        if let url = Bundle.main.url(forResource: name, withExtension: "icns"),
           let image = NSImage(contentsOf: url) { return image }
        return NSImage(named: NSImage.applicationIconName) ?? NSImage()
    }()
}

struct AppBrandIcon: View {
    var size: CGFloat = 34

    var body: some View {
        Image(nsImage: AppBrandAssets.icon)
            .resizable().interpolation(.high).scaledToFit()
            .frame(width: size, height: size)
            .accessibilityLabel("Bello Box app icon")
    }
}
