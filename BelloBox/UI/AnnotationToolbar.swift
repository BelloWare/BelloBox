import SwiftUI

struct AnnotationToolbarView: View {
    @ObservedObject var viewModel: ScreenshotPopupViewModel
    var showExportActions = false
    var onClose: (() -> Void)?
    /// Shown only in the capture overlay for area and window captures.
    var onScrollCapture: (() -> Void)?

    var body: some View {
        HStack(spacing: 6) {
            ForEach(Array(AnnotationTool.allCases.enumerated()), id: \.element.id) { index, tool in
                Button {
                    viewModel.activeTool = tool
                } label: {
                    Image(systemName: tool.symbol)
                        .frame(width: 28, height: 26)
                }
                .buttonStyle(.plain)
                .background(RoundedRectangle(cornerRadius: 7).fill(viewModel.activeTool == tool ? BoxTheme.accentSoft : .clear))
                .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(viewModel.activeTool == tool ? BoxTheme.accent : .clear, lineWidth: 1.5))
                .accessibilityLabel(tool.label)
                .accessibilityValue(viewModel.activeTool == tool ? "Selected" : "Not selected")
                .keyboardShortcut(KeyEquivalent(Character(String(index + 1))), modifiers: [.command, .option])
                .overlayTooltip("\(Self.tooltip(for: tool)) (⌥⌘\(index + 1))")
            }

            Divider().frame(height: 24)

            styleControls
                .frame(width: 150, alignment: .leading)

            Menu {
                Text("\(Int(viewModel.visibleImageSize.width)) × \(Int(viewModel.visibleImageSize.height)) pixels")
                Button("Reset Crop", action: viewModel.resetCrop)
                    .disabled(!viewModel.canResetCrop)
                Divider()
                Text("Choose tools with ⌥⌘1 through ⌥⌘9")
            } label: { Image(systemName: "ellipsis.circle") }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .accessibilityLabel("Image options")
            .overlayTooltip("Image size and reset crop")

            Spacer(minLength: 8)

            Button { viewModel.undo() } label: { Image(systemName: "arrow.uturn.backward") }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(!viewModel.canUndo)
                .keyboardShortcut("z", modifiers: .command)
                .accessibilityLabel("Undo")
                .overlayTooltip("Undo (⌘Z)")

            Button { viewModel.redo() } label: { Image(systemName: "arrow.uturn.forward") }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(!viewModel.canRedo)
                .keyboardShortcut("z", modifiers: [.command, .shift])
                .accessibilityLabel("Redo")
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
                    .keyboardShortcut("c", modifiers: [.command, .shift])
                    .accessibilityLabel("Copy Image")
                    .overlayTooltip("Copy image to the clipboard (⇧⌘C)")
                Button { viewModel.saveRenderedImage() } label: { Image(systemName: "square.and.arrow.down") }
                    .buttonStyle(SecondaryButtonStyle())
                    .keyboardShortcut("s", modifiers: .command)
                    .accessibilityLabel("Save PNG")
                    .overlayTooltip("Save as PNG… (⌘S)")
                if let onClose {
                    Button(action: onClose) {
                        Image(systemName: "xmark")
                    }
                    .buttonStyle(SecondaryButtonStyle())
                    .accessibilityLabel("Cancel Screenshot")
                    .overlayTooltip("Cancel (esc)")
                }
            }

            Button { viewModel.finish() } label: { Image(systemName: "checkmark") }
                .buttonStyle(PrimaryButtonStyle())
                .keyboardShortcut(.defaultAction)
                .accessibilityLabel("Copy and Finish")
                .overlayTooltip("Copy the image and finish (return)")
        }
    }

    @ViewBuilder
    private var styleControls: some View {
        switch viewModel.activeTool {
        case .pen, .arrow, .rectangle:
            HStack(spacing: 5) {
                colorPicker
                Slider(value: $viewModel.style.lineWidth, in: 1...12, step: 1)
                    .accessibilityLabel("Line width")
                    .accessibilityValue("\(Int(viewModel.style.lineWidth)) pixels")
                    .overlayTooltip("Line width")
                Text("\(Int(viewModel.style.lineWidth)) px")
                    .font(.caption2.monospacedDigit())
                    .fixedSize()
            }
        case .text:
            HStack(spacing: 5) {
                colorPicker
                Stepper(value: $viewModel.style.fontSize, in: 10...72, step: 2) {
                    Text("\(Int(viewModel.style.fontSize)) px")
                        .font(.caption.monospacedDigit())
                }
                .accessibilityLabel("Text size")
                .accessibilityValue("\(Int(viewModel.style.fontSize)) pixels")
                .overlayTooltip("Text size in the exported image")
            }
        case .select, .crop, .eraser, .highlight, .blur:
            Text(Self.tooltip(for: viewModel.activeTool))
                .font(.caption2)
                .foregroundStyle(.secondary)
                .lineLimit(2)
        }
    }

    private var colorPicker: some View {
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
