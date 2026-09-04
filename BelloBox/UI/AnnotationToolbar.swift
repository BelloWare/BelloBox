import SwiftUI

struct AnnotationToolbarView: View {
    @ObservedObject var viewModel: ScreenshotPopupViewModel
    var showExportActions = false
    var onClose: (() -> Void)?
    /// Shown only in the capture overlay for area and window captures.
    var onScrollCapture: (() -> Void)?

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
                .overlayTooltip(Self.tooltip(for: tool))
            }

            Divider().frame(height: 24)

            ColorPicker("Color", selection: Binding(
                get: { Color(nsColor: viewModel.style.strokeColor.nsColor) },
                set: { color in
                    if let cgColor = color.cgColor, let nsColor = NSColor(cgColor: cgColor) {
                        // Strokes are always fully opaque; the picker's opacity slider is
                        // disabled so lines and masks never see through.
                        var stroke = CodableColor(nsColor)
                        stroke.alpha = 1
                        viewModel.style.strokeColor = stroke
                    }
                }
            ), supportsOpacity: false)
            .labelsHidden()
            .frame(width: 34)
            .overlayTooltip("Stroke and text color")

            Slider(value: Binding(
                get: { Double(viewModel.style.lineWidth) },
                set: { viewModel.style.lineWidth = CGFloat($0) }
            ), in: 1...12, step: 1)
            .frame(width: 90)
            .overlayTooltip("Line width: \(Int(viewModel.style.lineWidth)) px")

            Spacer(minLength: 8)

            Button { viewModel.undo() } label: { Image(systemName: "arrow.uturn.backward") }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(!viewModel.canUndo)
                .keyboardShortcut("z", modifiers: .command)
                .overlayTooltip("Undo (⌘Z)")

            Button { viewModel.redo() } label: { Image(systemName: "arrow.uturn.forward") }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(!viewModel.canRedo)
                .keyboardShortcut("Z", modifiers: [.command, .shift])
                .overlayTooltip("Redo (⇧⌘Z)")

            if let onScrollCapture {
                Divider().frame(height: 24)

                Button(action: onScrollCapture) {
                    Label("Scroll", systemImage: "arrow.down.doc")
                        .foregroundStyle(BoxTheme.accent)
                }
                .buttonStyle(SecondaryButtonStyle())
                .overlayTooltip("Scroll to capture more: keep scrolling (or auto-scroll) and stitch the frames into one tall screenshot")
            }

            if showExportActions {
                Divider().frame(height: 24)

                Button { viewModel.copyRenderedImage() } label: { Image(systemName: "doc.on.doc") }
                    .buttonStyle(SecondaryButtonStyle())
                    .overlayTooltip("Copy image to the clipboard")
                Button { viewModel.saveRenderedImage() } label: { Image(systemName: "square.and.arrow.down") }
                    .buttonStyle(SecondaryButtonStyle())
                    .overlayTooltip("Save as PNG…")
                if let onClose {
                    Button(action: onClose) {
                        Image(systemName: "xmark")
                    }
                    .buttonStyle(SecondaryButtonStyle())
                    .overlayTooltip("Cancel (esc)")
                }
            }

            Button { viewModel.finish() } label: { Image(systemName: "checkmark") }
                .buttonStyle(PrimaryButtonStyle())
                .keyboardShortcut(.defaultAction)
                .overlayTooltip("Copy the image and finish (return)")
        }
    }

    static func tooltip(for tool: AnnotationTool) -> String {
        switch tool {
        case .select: return "Select: move the selection, drag text labels"
        case .pen: return "Pen: draw freehand"
        case .arrow: return "Arrow"
        case .rectangle: return "Rectangle outline"
        case .highlight: return "Highlighter"
        case .text: return "Text label"
        case .crop: return "Crop the screenshot"
        case .blur: return "Mask: hide sensitive content"
        case .eraser: return "Eraser: remove an annotation"
        }
    }
}
