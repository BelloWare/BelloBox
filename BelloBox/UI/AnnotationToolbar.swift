import SwiftUI

struct AnnotationToolbarView: View {
    @ObservedObject var viewModel: ScreenshotPopupViewModel
    var showExportActions = false
    var onClose: (() -> Void)?

    var body: some View {
        HStack(spacing: 6) {
            ForEach(AnnotationTool.allCases) { tool in
                Button {
                    viewModel.activeTool = tool
                } label: {
                    Image(systemName: tool.symbol)
                        .frame(width: 28, height: 26)
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
            .frame(width: 34)

            Slider(value: Binding(
                get: { Double(viewModel.style.lineWidth) },
                set: { viewModel.style.lineWidth = CGFloat($0) }
            ), in: 1...12, step: 1)
            .frame(width: 90)

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

            if showExportActions {
                Button { viewModel.copyRenderedImage() } label: { Image(systemName: "doc.on.doc") }
                    .buttonStyle(SecondaryButtonStyle())
                    .help("Copy image")
                Button { viewModel.saveRenderedImage() } label: { Image(systemName: "square.and.arrow.down") }
                    .buttonStyle(SecondaryButtonStyle())
                    .help("Save PNG")
                if let onClose {
                    Button(action: onClose) {
                        Image(systemName: "xmark")
                    }
                    .buttonStyle(SecondaryButtonStyle())
                    .help("Cancel")
                }
            }

            Button { viewModel.finish() } label: { Image(systemName: "checkmark") }
                .buttonStyle(PrimaryButtonStyle())
                .keyboardShortcut(.defaultAction)
                .help("Copy image and finish")
        }
    }
}
