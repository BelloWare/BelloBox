import AppKit
import SwiftUI

/// Full workspace windows share native controls; floating tool popups keep their
/// non-activating panel behavior and the shared PopupHeader.
enum AppWindowChrome {
    static let styleMask: NSWindow.StyleMask = [.titled, .closable, .miniaturizable, .resizable]

    static func apply(to window: NSWindow, title: String) {
        window.styleMask = styleMask
        window.title = title
        window.titlebarAppearsTransparent = true
        window.backgroundColor = NSColor(BoxTheme.background)
        window.isReleasedWhenClosed = false
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
