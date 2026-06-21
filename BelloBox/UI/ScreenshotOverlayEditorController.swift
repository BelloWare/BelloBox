import AppKit
import SwiftUI

@MainActor
final class ScreenshotOverlayEditorController {
    private var windows: [ScreenshotOverlayEditorWindow] = []
    private var keyMonitor: Any?
    private var viewModel: ScreenshotPopupViewModel?
    private var isClosing = false

    deinit {
        MainActor.assumeIsolated {
            close()
        }
    }

    func show(viewModel: ScreenshotPopupViewModel, captureFrame: CGRect) {
        close()
        self.viewModel = viewModel
        installKeyMonitor(viewModel: viewModel)
        AppActivation.bringAppForward()
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
        close(notifyViewModel: true)
    }

    func closeFromViewModel() {
        close(notifyViewModel: false)
    }

    private func close(notifyViewModel: Bool) {
        guard !isClosing else { return }
        isClosing = true
        let closingViewModel = viewModel
        viewModel = nil
        if let keyMonitor {
            NSEvent.removeMonitor(keyMonitor)
            self.keyMonitor = nil
        }
        for window in windows {
            window.orderOut(nil)
        }
        windows.removeAll()
        if notifyViewModel {
            closingViewModel?.close()
        }
        isClosing = false
    }

    private func installKeyMonitor(viewModel: ScreenshotPopupViewModel) {
        if let keyMonitor {
            NSEvent.removeMonitor(keyMonitor)
        }
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { [weak viewModel] event in
            guard event.keyCode == 53 else { return event }
            Task { @MainActor in viewModel?.close() }
            return nil
        }
    }
}

private final class ScreenshotOverlayEditorWindow: NSPanel {
    init(screen: NSScreen) {
        // This editor must activate so local Esc handling and inline text fields
        // receive keyboard events while the full-screen overlay is open.
        super.init(
            contentRect: screen.frame,
            styleMask: [.borderless],
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

            AnnotationToolbarView(viewModel: viewModel, showExportActions: true, onClose: viewModel.close)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(.regularMaterial))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(.white.opacity(0.14), lineWidth: 1))
        .shadow(color: .black.opacity(0.28), radius: 16, y: 8)
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
