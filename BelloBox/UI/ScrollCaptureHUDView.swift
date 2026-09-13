import AppKit
import SwiftUI

/// The control card shown next to the live selection during a scrolling capture.
/// It lives in its own key-capable panel because the capture overlay passes mouse
/// events through to the content underneath during this mode.
///
/// The card answers three questions at a glance: which mode is active (you scroll,
/// or Bello Box auto-scrolls), how much has been captured (screens, pixel height,
/// frames against the limit), and what Finish and Cancel do.
struct ScrollCaptureHUDView: View {
    /// The card comes in two sizes: the full card with the preview strip, progress
    /// and hint, and a single-row card for selections that leave no room for it.
    enum Layout: String, CaseIterable {
        case full
        case compact
    }

    /// Fixed card sizes (every row is always reserved) so the hosting panel can be laid
    /// out before the view exists.
    static let preferredSize = CGSize(width: 560, height: 156)
    static let compactSize = CGSize(width: 560, height: 56)
    /// Transparent margin around the card (room for its shadow); it may overlap the
    /// sampled selection because it is invisible and lets clicks through.
    static let outerPadding: CGFloat = 20
    static let previewSize = CGSize(width: 92, height: 104)
    static let title = "Scrolling Capture"
    /// Short enough for two tooltip lines, where the compact card shows it.
    static let hint = "Scroll the page inside the orange frame yourself, or press Auto-scroll and Bello Box scrolls until the content ends. Finish stitches everything captured so far."

    static func preferredSize(for layout: Layout) -> CGSize {
        switch layout {
        case .full:
            return preferredSize
        case .compact:
            return compactSize
        }
    }

    @ObservedObject var engine: ScrollCaptureEngine
    var layout: Layout = .full
    var onDone: () -> Void
    var onCancel: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Group {
            switch layout {
            case .full:
                fullCard
            case .compact:
                compactCard
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .frame(width: size.width, height: size.height, alignment: .topLeading)
        .popupCard()
        .padding(Self.outerPadding)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Self.title)
        .accessibilityValue(Self.accessibilitySummary(for: engine))
    }

    private var size: CGSize { Self.preferredSize(for: layout) }

    private var fullCard: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(spacing: 4) {
                ScrollCapturePreviewStrip(pieces: engine.previewPieces, isLive: engine.isAutoScrolling)
                    .frame(width: Self.previewSize.width, height: Self.previewSize.height)
                Text("Stitched so far")
                    .font(.caption2)
                    .foregroundStyle(BoxTheme.secondaryText)
                    .lineLimit(1)
            }
            .overlayTooltip("The screenshot as it will be stitched: every frame captured so far, overlap removed")

            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    badge
                    Text(Self.title)
                        .font(.system(size: 13, weight: .semibold))
                        .lineLimit(1)
                    Spacer(minLength: 0)
                    modeChip
                }

                progressRow

                Text(engine.message ?? Self.hint)
                    .font(.caption)
                    .foregroundStyle(engine.message == nil ? BoxTheme.secondaryText : messageColor)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, minHeight: 30, alignment: .topLeading)
                    .animation(reduceMotion ? nil : .easeOut(duration: 0.15), value: engine.message)

                HStack(spacing: 8) {
                    autoScrollButton
                    Spacer(minLength: 0)
                    cancelButton
                    doneButton(compact: false)
                }
            }
        }
    }

    /// One row: the title (or the current message), the extent and the buttons.
    /// The hint moves into the title's tooltip.
    private var compactCard: some View {
        HStack(spacing: 8) {
            badge
            if let message = engine.message {
                Text(message)
                    .font(.caption)
                    .foregroundStyle(messageColor)
                    .lineLimit(1)
                    .overlayTooltip(message)
            } else {
                Text(engine.isAutoScrolling ? "Auto-scrolling…" : Self.title)
                    .font(.system(size: 13, weight: .semibold))
                    .lineLimit(1)
                    .overlayTooltip(Self.hint)
            }
            Text(Self.extentText(for: engine, compact: true))
                .font(.caption.monospacedDigit())
                .foregroundStyle(BoxTheme.secondaryText)
                .lineLimit(1)
                .layoutPriority(1)
            Spacer(minLength: 8)
            HStack(spacing: 8) {
                autoScrollButton
                cancelButton
                doneButton(compact: true)
            }
            .fixedSize()
        }
        .frame(maxHeight: .infinity)
    }

    private var badge: some View {
        Image(systemName: "arrow.down.doc")
            .font(.system(size: 12, weight: .bold))
            .foregroundStyle(.white)
            .frame(width: 24, height: 24)
            .background(RoundedRectangle(cornerRadius: 7, style: .continuous).fill(BoxTheme.accentGradient))
            .accessibilityHidden(true)
    }

    /// Which of the two ways of scrolling is in effect right now.
    private var modeChip: some View {
        let auto = engine.isAutoScrolling
        return HStack(spacing: 5) {
            Circle().fill(auto ? BoxTheme.success : BoxTheme.accent).frame(width: 6, height: 6)
                .accessibilityHidden(true)
            Text(auto ? "Auto-scrolling for you" : "Manual · you scroll the frame")
                .font(.caption2.weight(.medium))
                .foregroundStyle(BoxTheme.secondaryText)
                .lineLimit(1)
        }
        .padding(.horizontal, 8).padding(.vertical, 3)
        .background { ToolSurface(role: .control).clipShape(Capsule()) }
        .overlay(Capsule().strokeBorder(BoxTheme.border))
        .overlayTooltip(auto ? "Bello Box is scrolling the content for you until it ends. Pause any time and scroll yourself."
                             : "Scroll the content inside the orange frame; each new screen is captured once it settles.")
    }

    /// Captured extent in screens and pixels, plus a bar of frames against the limit.
    private var progressRow: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Text(Self.extentText(for: engine, compact: false))
                    .font(.caption.monospacedDigit())
                    .lineLimit(1)
                Spacer(minLength: 4)
                Text(Self.frameCountText(for: engine))
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(BoxTheme.secondaryText)
                    .lineLimit(1)
            }
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    Capsule().fill(BoxTheme.well)
                    Capsule().fill(engine.reachedEnd ? BoxTheme.success : BoxTheme.accent)
                        .frame(width: max(4, geometry.size.width * Self.frameFraction(for: engine)))
                        .animation(reduceMotion ? nil : .easeOut(duration: 0.2), value: engine.frames.count)
                }
            }
            .frame(height: 4)
            .accessibilityHidden(true)
        }
        .overlayTooltip("How much has been captured: screens of the selection, the stitched height, and frames used of the maximum in Settings")
    }

    private var messageColor: Color {
        engine.reachedEnd ? BoxTheme.success : BoxTheme.warning
    }

    private var autoScrollButton: some View {
        Button {
            engine.toggleAutoScroll()
        } label: {
            Label(engine.isAutoScrolling ? "Pause" : "Auto-scroll", systemImage: engine.isAutoScrolling ? "pause.circle.fill" : "play.circle.fill")
                .symbolRenderingMode(.hierarchical)
        }
        .buttonStyle(SecondaryButtonStyle())
        .disabled(engine.phase != .watching)
        .accessibilityLabel(engine.isAutoScrolling ? "Pause auto-scroll" : "Start auto-scroll")
        .overlayTooltip(engine.isAutoScrolling ? "Stop scrolling automatically; you can keep scrolling yourself"
                                               : "Let Bello Box scroll the content until it ends, capturing each screen as it goes")
    }

    private var cancelButton: some View {
        Button("Cancel") { onCancel() }
            .buttonStyle(SecondaryButtonStyle())
            .keyboardShortcut(.cancelAction)
            .accessibilityLabel("Cancel scrolling capture")
            .overlayTooltip("Discard the captured frames and return to the editor with the original screenshot (esc)")
    }

    private func doneButton(compact: Bool) -> some View {
        Button {
            onDone()
        } label: {
            if engine.phase == .stitching {
                HStack(spacing: 6) {
                    ProgressView().controlSize(.small)
                    if !compact { Text("Stitching…") }
                }.frame(minWidth: 40)
            } else {
                Text(compact ? "Finish" : Self.finishTitle(for: engine))
            }
        }
        .buttonStyle(PrimaryButtonStyle())
        .keyboardShortcut(.defaultAction)
        .disabled(!engine.canFinish)
        .accessibilityLabel("Finish and stitch")
        .overlayTooltip("Stop capturing and stitch the frames into one tall screenshot, then open it in the editor (return)")
    }

    static func finishTitle(for engine: ScrollCaptureEngine) -> String {
        let count = engine.frames.count
        return count > 1 ? "Finish · Stitch \(count)" : "Finish"
    }

    /// "≈ 2.4 screens · 1,880 px tall", or just the pixel height on the compact card.
    static func extentText(for engine: ScrollCaptureEngine, compact: Bool) -> String {
        let rows = engine.previewRowCount
        guard rows > 0 else { return compact ? "Waiting for the first frame" : "Nothing captured yet" }
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        let pixels = "\(formatter.string(from: NSNumber(value: rows)) ?? "\(rows)") px tall"
        if compact { return "\(engine.frames.count) frame\(engine.frames.count == 1 ? "" : "s") · \(pixels)" }
        let screens = engine.screensCaptured
        let screenText = screens < 1.05 ? "1 screen" : String(format: "≈ %.1f screens", screens)
        return "\(screenText) · \(pixels)"
    }

    static func frameCountText(for engine: ScrollCaptureEngine) -> String {
        let count = engine.frames.count
        switch engine.phase {
        case .stitching: return "Stitching \(count) frame\(count == 1 ? "" : "s")…"
        case .finished: return "Done"
        case let .failed(message): return message
        case .idle, .watching: return "\(count) of \(engine.configuration.maxFrames) frames"
        }
    }

    static func frameFraction(for engine: ScrollCaptureEngine) -> CGFloat {
        let limit = max(1, engine.configuration.maxFrames)
        return min(1, CGFloat(engine.frames.count) / CGFloat(limit))
    }

    static func accessibilitySummary(for engine: ScrollCaptureEngine) -> String {
        let mode = engine.isAutoScrolling ? "Auto-scrolling" : "Manual scrolling"
        return [mode, extentText(for: engine, compact: false), frameCountText(for: engine), engine.message].compactMap { $0 }.joined(separator: ". ")
    }
}

/// The frames captured so far, stacked the way they will be stitched and scaled to fit,
/// so the user sees the screenshot grow while scrolling.
struct ScrollCapturePreviewStrip: View {
    var pieces: [ScrollCaptureEngine.PreviewPiece]
    var isLive: Bool

    var body: some View {
        Canvas { context, size in
            let inset: CGFloat = 4
            let box = CGRect(origin: .zero, size: size).insetBy(dx: inset, dy: inset)
            let totalHeight = CGFloat(pieces.reduce(0) { $0 + $1.image.height })
            guard totalHeight > 0, let width = pieces.first.map({ CGFloat($0.image.width) }), width > 0 else { return }
            let scale = min(box.height / totalHeight, box.width / width)
            let drawWidth = width * scale
            let x = box.midX - drawWidth / 2
            var y = box.minY
            for piece in pieces {
                let height = CGFloat(piece.image.height) * scale
                let rect = CGRect(x: x, y: y, width: drawWidth, height: height)
                context.draw(Image(nsImage: NSImage(cgImage: piece.image, size: NSSize(width: piece.image.width, height: piece.image.height))), in: rect)
                y += height
            }
            // A thin accent line marks the end of the last captured frame.
            let marker = Path(CGRect(x: x, y: min(y, box.maxY) - 1.5, width: drawWidth, height: 1.5))
            context.fill(marker, with: .color(BoxTheme.accent.opacity(0.9)))
        }
        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Color.primary.opacity(0.06)))
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(isLive ? BoxTheme.accent.opacity(0.7) : Color.primary.opacity(0.12), lineWidth: isLive ? 1.5 : 1)
        )
        .animation(.easeOut(duration: 0.2), value: pieces)
        .accessibilityHidden(true)
    }
}

extension CGSize {
    /// The size grown by `padding` on every side.
    func applying(padding: CGFloat) -> CGSize {
        CGSize(width: width + padding * 2, height: height + padding * 2)
    }
}

/// Floating, non-activating panel that hosts `ScrollCaptureHUDView` above the capture
/// overlay. It can become key so Return and Escape reach the HUD when Bello Box is
/// frontmost, without pulling focus from the app being scrolled.
final class ScrollCaptureHUDPanel: NSPanel {
    var onEscape: (() -> Void)?

    init() {
        super.init(
            contentRect: CGRect(origin: .zero, size: ScrollCaptureHUDView.preferredSize),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        isFloatingPanel = true
        level = NSWindow.Level(rawValue: NSWindow.Level.screenSaver.rawValue + 1)
        collectionBehavior = [.canJoinAllSpaces, .canJoinAllApplications, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        becomesKeyOnlyIfNeeded = false
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func cancelOperation(_ sender: Any?) {
        onEscape?()
    }
}
