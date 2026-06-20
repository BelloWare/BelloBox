import AppKit
import SwiftUI

@MainActor
final class ScreenshotOverlayEditorController {
    private var windows: [ScreenshotOverlayEditorWindow] = []

    func show(viewModel: ScreenshotPopupViewModel, captureFrame: CGRect) {
        close()
        let targetScreen = ScreenCoordinateSpace.displayForCocoaRect(captureFrame)
            ?? ScreenPlacement.screen(containing: CGPoint(x: captureFrame.midX, y: captureFrame.midY))

        var targetWindow: ScreenshotOverlayEditorWindow?
        for screen in NSScreen.screens {
            let window = ScreenshotOverlayEditorWindow(screen: screen)
            if screen == targetScreen {
                let view = ScreenshotOverlayEditorView(
                    viewModel: viewModel,
                    screenFrame: screen.frame,
                    captureFrame: captureFrame
                )
                window.contentView = NSHostingView(rootView: view)
                targetWindow = window
            } else {
                window.contentView = NSHostingView(rootView: ScreenshotOverlayDimView())
            }
            window.orderFrontRegardless()
            windows.append(window)
        }

        targetWindow?.makeKeyAndOrderFront(nil)
    }

    func close() {
        for window in windows {
            window.orderOut(nil)
        }
        windows.removeAll()
    }
}

private final class ScreenshotOverlayEditorWindow: NSPanel {
    init(screen: NSScreen) {
        super.init(
            contentRect: screen.frame,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        level = .screenSaver
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        hidesOnDeactivate = false
        isReleasedWhenClosed = false
        ignoresMouseEvents = false
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

private struct ScreenshotOverlayDimView: View {
    var body: some View {
        Color.black.opacity(0.34)
            .ignoresSafeArea()
    }
}

private struct ScreenshotOverlayEditorView: View {
    private static let toolbarHeight: CGFloat = 54
    private static let toolbarMinimumWidth: CGFloat = 700
    private static let toolbarMaximumWidth: CGFloat = 920
    private static let gap: CGFloat = 10
    private static let inset: CGFloat = 12

    @ObservedObject var viewModel: ScreenshotPopupViewModel
    var screenFrame: CGRect
    var captureFrame: CGRect

    var body: some View {
        GeometryReader { geometry in
            let bounds = CGRect(origin: .zero, size: geometry.size)
            let selected = localSelectionFrame(in: bounds)
            let toolbar = toolbarFrame(selection: selected, bounds: bounds)
            let error = errorFrame(toolbar: toolbar, bounds: bounds)

            ZStack(alignment: .topLeading) {
                Color.black.opacity(0.34)
                    .ignoresSafeArea()

                AnnotationCanvasView(viewModel: viewModel)
                    .frame(width: max(selected.width, 1), height: max(selected.height, 1))
                    .clipShape(Rectangle())
                    .overlay(Rectangle().strokeBorder(BoxTheme.accent, lineWidth: 2))
                    .position(x: selected.midX, y: selected.midY)

                editorToolbar
                    .frame(width: toolbar.width, height: toolbar.height)
                    .position(x: toolbar.midX, y: toolbar.midY)

                if let message = viewModel.errorMessage {
                    Label(message, systemImage: "exclamationmark.triangle.fill")
                        .font(.caption)
                        .foregroundStyle(.orange)
                        .lineLimit(2)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 7)
                        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(.regularMaterial))
                        .frame(width: error.width, alignment: .leading)
                        .position(x: error.midX, y: error.midY)
                }
            }
            .frame(width: geometry.size.width, height: geometry.size.height)
            .onExitCommand(perform: viewModel.close)
        }
    }

    private var editorToolbar: some View {
        HStack(spacing: 6) {
            Image(systemName: "camera.viewfinder")
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(.white)
                .frame(width: 28, height: 28)
                .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(BoxTheme.accentGradient))

            ForEach(AnnotationTool.allCases) { tool in
                Button {
                    viewModel.activeTool = tool
                } label: {
                    Image(systemName: tool.symbol)
                        .frame(width: 24, height: 24)
                }
                .buttonStyle(.plain)
                .background(RoundedRectangle(cornerRadius: 7).fill(viewModel.activeTool == tool ? BoxTheme.accentSoft : .clear))
                .help(tool.label)
            }

            Divider().frame(height: 24)

            ColorPicker("Color", selection: Binding(
                get: { Color(nsColor: viewModel.style.strokeColor.nsColor) },
                set: { color in
                    if let cgColor = color.cgColor, let nsColor = NSColor(cgColor: cgColor) {
                        viewModel.style.strokeColor = CodableColor(nsColor)
                    }
                }
            ))
            .labelsHidden()
            .frame(width: 30)

            Slider(value: Binding(
                get: { Double(viewModel.style.lineWidth) },
                set: { viewModel.style.lineWidth = CGFloat($0) }
            ), in: 1...12, step: 1)
            .frame(width: 76)

            TextField("Text", text: $viewModel.pendingTextLabel)
                .textFieldStyle(.roundedBorder)
                .frame(width: 104)

            Spacer(minLength: 6)

            iconButton("arrow.uturn.backward", help: "Undo", disabled: !viewModel.canUndo) { viewModel.undo() }
                .keyboardShortcut("z", modifiers: .command)
            iconButton("arrow.uturn.forward", help: "Redo", disabled: !viewModel.canRedo) { viewModel.redo() }
                .keyboardShortcut("Z", modifiers: [.command, .shift])
            iconButton("doc.on.doc", help: "Copy image") { viewModel.copyRenderedImage() }
            iconButton("square.and.arrow.down", help: "Save PNG") { viewModel.saveRenderedImage() }
            iconButton("xmark", help: "Close") { viewModel.close() }

            Button { viewModel.finish() } label: {
                Image(systemName: "checkmark")
                    .frame(width: 24, height: 24)
            }
            .buttonStyle(PrimaryButtonStyle())
            .keyboardShortcut(.defaultAction)
            .help("Copy image and finish")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(.regularMaterial))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(.white.opacity(0.14), lineWidth: 1))
        .shadow(color: .black.opacity(0.28), radius: 16, y: 8)
    }

    private func iconButton(_ systemName: String, help: String, disabled: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .frame(width: 24, height: 24)
        }
        .buttonStyle(SecondaryButtonStyle())
        .disabled(disabled)
        .help(help)
    }

    private func localSelectionFrame(in bounds: CGRect) -> CGRect {
        RegionCaptureGeometry
            .globalCocoaRectToLocalFlipped(captureFrame, screenFrame: screenFrame)
            .intersection(bounds)
            .standardized
    }

    private func toolbarFrame(selection: CGRect, bounds: CGRect) -> CGRect {
        let availableWidth = max(280, bounds.width - Self.inset * 2)
        let width = min(max(Self.toolbarMinimumWidth, min(Self.toolbarMaximumWidth, availableWidth)), availableWidth)
        let x = min(max(selection.minX, Self.inset), bounds.maxX - width - Self.inset)
        var y = selection.minY - Self.toolbarHeight - Self.gap
        if y < bounds.minY + Self.inset {
            y = selection.maxY + Self.gap
        }
        y = min(max(y, bounds.minY + Self.inset), bounds.maxY - Self.toolbarHeight - Self.inset)
        return CGRect(x: x, y: y, width: width, height: Self.toolbarHeight)
    }

    private func errorFrame(toolbar: CGRect, bounds: CGRect) -> CGRect {
        let width = min(toolbar.width, bounds.width - Self.inset * 2)
        let height: CGFloat = 38
        var y = toolbar.maxY + 8
        if y + height > bounds.maxY - Self.inset {
            y = toolbar.minY - height - 8
        }
        return CGRect(x: toolbar.minX, y: y, width: width, height: height)
    }
}
