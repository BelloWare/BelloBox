import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class OCRPanelViewModel: ObservableObject {
    @Published var result: OCRResult? {
        didSet { statusMessage = nil }
    }
    @Published var isRunning = false
    @Published var showTextRegions = false
    @Published var errorMessage: String?
    @Published var statusMessage: String?
    @Published var activeDisplayMode: OCRDisplayMode = .text

    var onRunMacOCR: () -> Void = {}
    var onRunLLMOCR: () -> Void = {}
    var onCopyPlainText: () -> Void = {}
    var onCopyMarkdown: () -> Void = {}
    var onCancel: () -> Void = {}

    var plainText: String { result.map(OCRResultFormatter.plainText) ?? "" }
    var markdown: String { result.map(OCRResultFormatter.markdown) ?? "" }
    var canCopyText: Bool { !plainText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    var canCopyMarkdown: Bool { !markdown.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    func saveText() {
        let isMarkdown = activeDisplayMode == .markdown
        let text = isMarkdown ? markdown : plainText
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = isMarkdown ? [UTType(filenameExtension: "md") ?? .plainText] : [.plainText]
        panel.nameFieldStringValue = isMarkdown ? "screenshot-text.md" : "screenshot-text.txt"
        panel.canCreateDirectories = true
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            Task { @MainActor in
                do {
                    try text.write(to: url, atomically: true, encoding: .utf8)
                    self.errorMessage = nil
                    self.statusMessage = "Saved to \(url.lastPathComponent)."
                } catch {
                    self.errorMessage = "Could not save text: \(error.localizedDescription)"
                }
            }
        }
    }
}

struct OCRPanelView: View {
    @ObservedObject var viewModel: OCRPanelViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Text Reader")
                    .font(.headline)
                Spacer()
                Toggle("Boxes", isOn: $viewModel.showTextRegions)
                    .toggleStyle(.checkbox)
                    .disabled(viewModel.result?.regions.isEmpty ?? true)
                    .help("Show the locations of recognized text on the screenshot")
            }

            HStack(spacing: 8) {
                if viewModel.isRunning {
                    ProgressView().controlSize(.small)
                    Text("Reading…").font(.caption)
                    Spacer()
                    Button("Cancel", action: viewModel.onCancel)
                        .buttonStyle(SecondaryButtonStyle())
                        .help("Stop reading and keep the previous result")
                } else {
                    Button(action: viewModel.onRunMacOCR) {
                        Label("Read on Mac", systemImage: "text.viewfinder")
                    }
                    .buttonStyle(SecondaryButtonStyle())
                    .help("Recognize text locally on your Mac")

                    Button(action: viewModel.onRunLLMOCR) {
                        Label("AI OCR…", systemImage: "sparkles")
                    }
                    .buttonStyle(SecondaryButtonStyle())
                    .help("Review and approve an image upload to your AI provider")
                }
            }

            if viewModel.result?.markdownText != nil {
                Picker("OCR display", selection: $viewModel.activeDisplayMode) {
                    ForEach(OCRDisplayMode.allCases) { mode in
                        Text(mode.label).tag(mode)
                    }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }

            ScrollView {
                if displayText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Label(viewModel.result == nil ? "Read text from this image" : "No text found", systemImage: "text.viewfinder")
                            .font(.caption.weight(.semibold))
                        Text(viewModel.result == nil
                             ? "Choose Read on Mac to extract text without uploading the image."
                             : "Try a closer crop or use AI OCR for difficult text.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(10)
                } else {
                    Text(displayText)
                        .font(.system(.caption, design: .monospaced))
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(10)
                        .accessibilityLabel("Recognized text")
                        .accessibilityValue(displayText)
                }
            }
            .background(RoundedRectangle(cornerRadius: 8).fill(.primary.opacity(0.05)))
            .frame(minHeight: 130, maxHeight: .infinity)

            HStack {
                Button("Copy Text") { viewModel.onCopyPlainText() }
                    .buttonStyle(SecondaryButtonStyle())
                    .disabled(!viewModel.canCopyText)
                Button("Copy Markdown") { viewModel.onCopyMarkdown() }
                    .buttonStyle(SecondaryButtonStyle())
                    .disabled(!viewModel.canCopyMarkdown)
                Button(action: viewModel.saveText) {
                    Image(systemName: "square.and.arrow.down")
                }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(!(viewModel.activeDisplayMode == .markdown ? viewModel.canCopyMarkdown : viewModel.canCopyText))
                .accessibilityLabel("Save recognized text")
                .help("Save the displayed text or Markdown to a file")
            }

            if let warnings = viewModel.result?.warnings, !warnings.isEmpty {
                ForEach(warnings, id: \.self) { warning in
                    Label(warning, systemImage: "exclamationmark.triangle")
                        .font(.caption2)
                        .foregroundStyle(BoxTheme.warning)
                }
            }

            if let error = viewModel.errorMessage {
                Label(error, systemImage: "xmark.octagon.fill")
                    .font(.caption)
                    .foregroundStyle(BoxTheme.danger)
                    .lineLimit(3)
            } else if let status = viewModel.statusMessage {
                Label(status, systemImage: "checkmark.circle")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var displayText: String {
        switch viewModel.activeDisplayMode {
        case .text:
            return viewModel.plainText
        case .markdown:
            return viewModel.markdown
        }
    }
}
