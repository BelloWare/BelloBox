import AppKit
import SwiftUI

/// The compact control strip shown below the live selection while scrolling to capture
/// more. It lives in its own key-capable panel because the capture overlay passes mouse
/// events through to the content underneath during this mode.
struct ScrollCaptureHUDView: View {
    /// Fixed card size (the message row is always reserved) so the hosting panel can be
    /// laid out before the view exists.
    static let preferredSize = CGSize(width: 470, height: 76)
    /// Transparent margin around the card (room for its shadow); it may overlap the
    /// sampled selection because it is invisible and lets clicks through.
    static let outerPadding: CGFloat = 20

    @ObservedObject var engine: ScrollCaptureEngine
    var onDone: () -> Void
    var onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Image(systemName: "arrow.down.doc")
                    .font(.system(size: 13, weight: .bold))
                    .foregroundStyle(.white)
                    .frame(width: 28, height: 28)
                    .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(BoxTheme.accentGradient))

                VStack(alignment: .leading, spacing: 1) {
                    Text("Scroll to capture more")
                        .font(.system(size: 13, weight: .semibold))
                    Text(statusText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                Spacer(minLength: 8)

                Button(engine.isAutoScrolling ? "Stop" : "Auto-scroll") {
                    engine.toggleAutoScroll()
                }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(engine.phase != .watching)
                .help(engine.isAutoScrolling ? "Stop scrolling automatically" : "Scroll the content automatically until it ends")

                Button("Cancel") { onCancel() }
                    .buttonStyle(SecondaryButtonStyle())
                    .keyboardShortcut(.cancelAction)
                    .disabled(engine.phase == .stitching)
                    .help("Back to the editor with the original capture")

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
                .help("Stitch the captured frames")
            }

            Label(engine.message ?? " ", systemImage: engine.message == nil ? "circle" : "info.circle")
                .font(.caption2)
                .foregroundStyle(.orange)
                .lineLimit(1)
                .opacity(engine.message == nil ? 0 : 1)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 9)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height, alignment: .topLeading)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(.regularMaterial))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(.white.opacity(0.14), lineWidth: 1))
        .shadow(color: .black.opacity(0.28), radius: 16, y: 8)
        .padding(Self.outerPadding)
    }

    private var statusText: String {
        let count = engine.frames.count
        let frames = "\(count) frame\(count == 1 ? "" : "s")"
        switch engine.phase {
        case .stitching:
            return "Stitching \(frames)…"
        case let .failed(message):
            return message
        case .finished:
            return "Done"
        case .idle, .watching:
            if engine.isAutoScrolling {
                return "Auto-scrolling · \(frames) captured"
            }
            return "Scroll the content in the frame · \(frames) captured"
        }
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
