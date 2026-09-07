import SwiftUI

struct AnnotationToolbarView: View {
    @ObservedObject var viewModel: ScreenshotPopupViewModel
    var showExportActions = false
    var onClose: (() -> Void)?
    /// Shown only in the capture overlay for area and window captures.
    var onScrollCapture: (() -> Void)?

    static let scrollCaptureTitle = "Scrolling Capture"
    /// Width the capture overlay reserves for the toolbar with every action shown
    /// (mask controls are the widest style row); `AnnotationToolbarLayoutTests` keeps it honest.
    static let overlayToolbarWidth: CGFloat = 1_060
    static let scrollCaptureTooltip = "Capture a scrolling page: the selection goes live, you scroll it (or let Bello Box auto-scroll) and every screen is stitched into one tall screenshot"

    var body: some View {
        HStack(spacing: 6) {
            ForEach(Array(AnnotationTool.allCases.enumerated()), id: \.element.id) { index, tool in
                Button {
                    viewModel.activeTool = tool
                } label: {
                    Image(systemName: tool.symbol)
                        .font(.system(size: 13, weight: .medium))
                        .frame(width: 30, height: 30)
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
                .frame(width: styleControlsWidth, alignment: .leading)

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
                    Label(Self.scrollCaptureTitle, systemImage: "arrow.down.doc")
                        .foregroundStyle(BoxTheme.accent)
                }
                .buttonStyle(SecondaryButtonStyle())
                .accessibilityLabel(Self.scrollCaptureTitle)
                .accessibilityHint("Turns the selection live and stitches the screens you scroll through")
                .overlayTooltip(Self.scrollCaptureTooltip)
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
        }.padding(showExportActions ? 0 : 8).surfaceCard().tint(BoxTheme.accent)
    }

    /// The mask row carries swatches, a custom colour and the pattern menu, so it
    /// gets a little more room than a slider does.
    private var styleControlsWidth: CGFloat {
        viewModel.activeTool == .blur ? 214 : 150
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
        case .eraser:
            HStack(spacing: 5) {
                Image(systemName: "circle").font(.system(size: 9)).foregroundStyle(.secondary)
                    .accessibilityHidden(true)
                Slider(value: $viewModel.eraserWidth, in: ScreenshotPopupViewModel.eraserWidthRange, step: 2)
                    .accessibilityLabel("Eraser size")
                    .accessibilityValue("\(Int(viewModel.eraserWidth)) pixels")
                    .overlayTooltip("Eraser size: drag over an arrow, line, shape, label, or mask to remove just that part")
                Image(systemName: "circle").font(.system(size: 15)).foregroundStyle(.secondary)
                    .accessibilityHidden(true)
                Text("\(Int(viewModel.eraserWidth)) px")
                    .font(.caption2.monospacedDigit())
                    .fixedSize()
            }
        case .blur:
            maskControls
        case .select, .crop, .highlight:
            Text(Self.tooltip(for: viewModel.activeTool))
                .font(.caption2)
                .foregroundStyle(.secondary)
                .lineLimit(2)
        }
    }

    /// Opaque fills only: the swatches and the picker both refuse translucency, and the
    /// pattern is drawn over the fill, so a mask never lets pixels show through.
    private var maskControls: some View {
        HStack(spacing: 6) {
            ForEach(AnnotationStyle.maskFillPresets, id: \.name) { preset in
                let selected = viewModel.maskStyle.maskFill == preset.color
                Button {
                    viewModel.maskStyle = .mask(fill: preset.color, pattern: viewModel.maskStyle.maskPattern)
                } label: {
                    Circle()
                        .fill(Color(nsColor: preset.color.nsColor))
                        .frame(width: 16, height: 16)
                        .overlay(Circle().strokeBorder(BoxTheme.border, lineWidth: 1))
                        .overlay(Circle().strokeBorder(BoxTheme.accent, lineWidth: selected ? 2 : 0).padding(-2))
                        .frame(width: 22, height: 22)
                        .contentShape(Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("\(preset.name) mask")
                .accessibilityValue(selected ? "Selected" : "Not selected")
                .overlayTooltip("\(preset.name) mask fill")
            }
            ColorPicker("Custom mask color", selection: Binding(
                get: { Color(nsColor: viewModel.maskStyle.maskFill.nsColor) },
                set: { color in
                    if let cgColor = color.cgColor, let nsColor = NSColor(cgColor: cgColor) {
                        viewModel.maskStyle = .mask(fill: CodableColor(nsColor), pattern: viewModel.maskStyle.maskPattern)
                    }
                }
            ), supportsOpacity: false)
            .labelsHidden()
            .frame(width: 30)
            .overlayTooltip("Custom mask fill (always opaque)")
            Menu {
                ForEach(MaskPattern.allCases) { pattern in
                    Button {
                        viewModel.maskStyle = .mask(fill: viewModel.maskStyle.maskFill, pattern: pattern)
                    } label: {
                        if pattern == viewModel.maskStyle.maskPattern {
                            Label(pattern.label, systemImage: "checkmark")
                        } else {
                            Label(pattern.label, systemImage: pattern.symbol)
                        }
                    }
                }
            } label: {
                Image(systemName: viewModel.maskStyle.maskPattern.symbol)
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 22, height: 22)
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .accessibilityLabel("Mask pattern")
            .accessibilityValue(viewModel.maskStyle.maskPattern.label)
            .overlayTooltip("Mask pattern: \(viewModel.maskStyle.maskPattern.label). Every pattern sits on the solid fill")
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
        case .blur: return "Mask: hide sensitive content behind an opaque fill"
        case .eraser: return "Eraser: brush away part of an annotation; the screenshot stays intact"
        }
    }
}
