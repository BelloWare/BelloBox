import AppKit
import SwiftUI

/// The control card shown next to the live selection while scrolling to capture more.
/// It lives in its own key-capable panel because the capture overlay passes mouse
/// events through to the content underneath during this mode.
struct ScrollCaptureHUDView: View {
    /// The card comes in two sizes: the full card with the preview strip and the hint,
    /// and a single-row card for selections that leave no room for the full one.
    enum Layout: String, CaseIterable {
        case full
        case compact
    }

    /// Fixed card sizes (every row is always reserved) so the hosting panel can be laid
    /// out before the view exists.
    static let preferredSize = CGSize(width: 540, height: 140)
    static let compactSize = CGSize(width: 540, height: 56)
    /// Transparent margin around the card (room for its shadow); it may overlap the
    /// sampled selection because it is invisible and lets clicks through.
    static let outerPadding: CGFloat = 20
    static let previewSize = CGSize(width: 92, height: 100)
    /// Short enough for two tooltip lines, where the compact card shows it.
    static let hint = "Scroll the content inside the frame yourself, or press Auto-scroll and Bello Box scrolls it for you until it ends."

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
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(.regularMaterial))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(.white.opacity(0.14), lineWidth: 1))
        .shadow(color: .black.opacity(0.28), radius: 16, y: 8)
        .padding(Self.outerPadding)
    }

    private var size: CGSize { Self.preferredSize(for: layout) }

    private var fullCard: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(spacing: 4) {
                ScrollCapturePreviewStrip(pieces: engine.previewPieces, isLive: engine.isAutoScrolling)
                    .frame(width: Self.previewSize.width, height: Self.previewSize.height)
                Text(previewCaption)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            .overlayTooltip("Preview of the stitched screenshot so far")

            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    badge
                    title
                    Spacer(minLength: 0)
                    Text(statusText(compact: false))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                Text(Self.hint)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)

                HStack(spacing: 8) {
                    autoScrollButton
                    Spacer(minLength: 0)
                    cancelButton
                    doneButton
                }

                Label(engine.message ?? " ", systemImage: engine.message == nil ? "circle" : "info.circle")
                    .font(.caption2)
                    .foregroundStyle(.orange)
                    .lineLimit(1)
                    .opacity(engine.message == nil ? 0 : 1)
            }
        }
    }

    /// One row: the title (or the current message), the frame count and the buttons.
    /// The hint moves into the title's tooltip.
    private var compactCard: some View {
        HStack(spacing: 8) {
            if let message = engine.message {
                Label(message, systemImage: "info.circle")
                    .font(.caption)
                    .foregroundStyle(.orange)
                    .lineLimit(1)
                    .overlayTooltip(message)
            } else if engine.isAutoScrolling {
                Text("Auto-scrolling…")
                    .font(.system(size: 13, weight: .semibold))
                    .lineLimit(1)
                    .overlayTooltip(Self.hint)
            } else {
                title.overlayTooltip(Self.hint)
            }
            Text(statusText(compact: true))
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .layoutPriority(1)
            Spacer(minLength: 8)
            HStack(spacing: 8) {
                autoScrollButton
                cancelButton
                doneButton
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
    }

    private var title: some View {
        Text("Scroll to capture more")
            .font(.system(size: 13, weight: .semibold))
            .lineLimit(1)
    }

    private var autoScrollButton: some View {
        Button {
            engine.toggleAutoScroll()
        } label: {
            Label(engine.isAutoScrolling ? "Stop" : "Auto-scroll", systemImage: engine.isAutoScrolling ? "stop.circle.fill" : "play.circle.fill")
                .symbolRenderingMode(.hierarchical)
        }
        .buttonStyle(SecondaryButtonStyle())
        .disabled(engine.phase != .watching)
        .overlayTooltip(engine.isAutoScrolling ? "Stop scrolling automatically" : "Scroll the content automatically until it ends, capturing as it goes")
    }

    private var cancelButton: some View {
        Button("Cancel") { onCancel() }
            .buttonStyle(SecondaryButtonStyle())
            .keyboardShortcut(.cancelAction)
            .overlayTooltip("Back to the editor with the original capture (esc)")
    }

    private var doneButton: some View {
        Button {
            onDone()
        } label: {
            if engine.phase == .stitching {
                ProgressView().controlSize(.small).frame(width: 40)
            } else {
                Text("Done")
            }
        }
        .buttonStyle(PrimaryButtonStyle())
        .keyboardShortcut(.defaultAction)
        .disabled(!engine.canFinish)
        .overlayTooltip("Stitch the captured frames into one tall screenshot (return)")
    }

    /// The compact row has no room for the longer forms, which would squeeze the title.
    private func statusText(compact: Bool) -> String {
        let count = engine.frames.count
        let frames = "\(count) frame\(count == 1 ? "" : "s")"
        switch engine.phase {
        case .stitching:
            return compact ? "Stitching…" : "Stitching \(frames)…"
        case let .failed(message):
            return message
        case .finished:
            return "Done"
        case .idle, .watching:
            return engine.isAutoScrolling && !compact ? "Auto-scrolling · \(frames)" : frames
        }
    }

    private var previewCaption: String {
        let rows = engine.previewRowCount
        guard rows > 0 else { return "—" }
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        return "≈ \(formatter.string(from: NSNumber(value: rows)) ?? "\(rows)") px tall"
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
