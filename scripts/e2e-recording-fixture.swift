import AppKit

private final class PulseView: NSView {
    private var timer: Timer?
    private var tick = 0

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            guard let self else { return }
            self.tick += 1
            self.needsDisplay = true
        }
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        timer?.invalidate()
    }

    override func draw(_ dirtyRect: NSRect) {
        let colors: [NSColor] = [.systemBlue, .systemGreen, .systemOrange, .systemPink]
        colors[tick % colors.count].setFill()
        bounds.fill()

        NSColor.white.withAlphaComponent(0.9).setStroke()
        let inset = CGFloat(12 + (tick % 8) * 4)
        let ring = NSBezierPath(ovalIn: bounds.insetBy(dx: inset, dy: inset))
        ring.lineWidth = 8
        ring.stroke()

        let label = "Bello Box E2E \(tick)"
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.boldSystemFont(ofSize: 28),
            .foregroundColor: NSColor.white
        ]
        let size = label.size(withAttributes: attributes)
        label.draw(
            at: CGPoint(x: bounds.midX - size.width / 2, y: bounds.midY - size.height / 2),
            withAttributes: attributes
        )
    }
}

private final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        guard let screen = NSScreen.main else {
            NSApp.terminate(nil)
            return
        }

        let rect = Self.fixtureRect(on: screen)
        let window = NSWindow(
            contentRect: rect,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.level = .floating
        window.isOpaque = true
        window.backgroundColor = .black
        window.hasShadow = false
        window.isReleasedWhenClosed = false
        window.contentView = PulseView(frame: CGRect(origin: .zero, size: rect.size))
        window.orderFrontRegardless()
        self.window = window

        if CommandLine.arguments.count > 1 {
            let markerURL = URL(fileURLWithPath: CommandLine.arguments[1])
            let lines = [
                "status=ready",
                "rect=\(Self.serialize(rect))",
                "timestamp=\(Date().timeIntervalSince1970)"
            ]
            try? FileManager.default.createDirectory(at: markerURL.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? lines.joined(separator: "\n").write(to: markerURL, atomically: true, encoding: .utf8)
        }
    }

    private static func fixtureRect(on screen: NSScreen) -> CGRect {
        let width = min(CGFloat(360), max(80, screen.frame.width * 0.4))
        let height = min(CGFloat(220), max(80, screen.frame.height * 0.3))
        return CGRect(
            x: screen.frame.midX - width / 2,
            y: screen.frame.midY - height / 2,
            width: width,
            height: height
        )
    }

    private static func serialize(_ rect: CGRect) -> String {
        "\(rect.origin.x),\(rect.origin.y),\(rect.size.width),\(rect.size.height)"
    }
}

let app = NSApplication.shared
private let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
