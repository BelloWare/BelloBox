import AppKit
import SwiftUI

struct InlineScreenshotEditorView: View {
    static let toolbarHeight: CGFloat = 58
    static let minimumWidth: CGFloat = 620
    static let minimumCanvasHeight: CGFloat = 180

    @ObservedObject var viewModel: ScreenshotPopupViewModel
    var canvasSize: CGSize
    var onMinimize: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            toolbar

            AnnotationCanvasView(viewModel: viewModel)
                .frame(width: max(canvasSize.width, 1), height: max(canvasSize.height, Self.minimumCanvasHeight))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(.primary.opacity(0.10), lineWidth: 1))

            if let error = viewModel.errorMessage {
                Label(error, systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(.orange)
                    .lineLimit(2)
            }
        }
        .padding(10)
        .frame(width: contentWidth, height: contentHeight)
        .background(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(.regularMaterial)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(.white.opacity(0.12), lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .shadow(color: .black.opacity(0.24), radius: 18, y: 10)
        .onExitCommand(perform: viewModel.close)
    }

    private var canvasHeight: CGFloat {
        max(canvasSize.height, Self.minimumCanvasHeight)
    }

    private var contentWidth: CGFloat {
        max(canvasSize.width + 20, Self.minimumWidth)
    }

    private var contentHeight: CGFloat {
        Self.toolbarHeight + canvasHeight + (viewModel.errorMessage == nil ? 20 : 42)
    }

    private var toolbar: some View {
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
                        .frame(width: 26, height: 24)
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
            .frame(width: 32)

            Slider(value: Binding(
                get: { Double(viewModel.style.lineWidth) },
                set: { viewModel.style.lineWidth = CGFloat($0) }
            ), in: 1...12, step: 1)
            .frame(width: 84)

            TextField("Text", text: $viewModel.pendingTextLabel)
                .textFieldStyle(.roundedBorder)
                .frame(width: 106)

            Spacer(minLength: 8)

            Button { viewModel.undo() } label: { Image(systemName: "arrow.uturn.backward") }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(!viewModel.canUndo)
                .keyboardShortcut("z", modifiers: .command)
                .help("Undo")

            Button { viewModel.redo() } label: { Image(systemName: "arrow.uturn.forward") }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(!viewModel.canRedo)
                .keyboardShortcut("Z", modifiers: [.command, .shift])
                .help("Redo")

            Button { viewModel.copyRenderedImage() } label: { Image(systemName: "doc.on.doc") }
                .buttonStyle(SecondaryButtonStyle())
                .help("Copy image")

            Button { viewModel.saveRenderedImage() } label: { Image(systemName: "square.and.arrow.down") }
                .buttonStyle(SecondaryButtonStyle())
                .help("Save PNG")

            if let onMinimize {
                Button(action: onMinimize) { Image(systemName: "minus") }
                    .buttonStyle(SecondaryButtonStyle())
                    .help("Minify")
            }

            Button { viewModel.close() } label: { Image(systemName: "xmark") }
                .buttonStyle(SecondaryButtonStyle())
                .help("Close")

            Button { viewModel.finish() } label: { Image(systemName: "checkmark") }
                .buttonStyle(PrimaryButtonStyle())
                .keyboardShortcut(.defaultAction)
                .help("Copy image and finish")
        }
    }
}
