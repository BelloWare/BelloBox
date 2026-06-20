import SwiftUI

@MainActor
final class OCRPanelViewModel: ObservableObject {
    @Published var result: OCRResult?
    @Published var isRunning = false
    @Published var showTextRegions = false
    @Published var selectedRegionID: UUID?
    @Published var errorMessage: String?
    @Published var activeDisplayMode: OCRDisplayMode = .text

    var onRunMacOCR: () -> Void = {}
    var onRunLLMOCR: () -> Void = {}
    var onCopyPlainText: () -> Void = {}
    var onCopyMarkdown: () -> Void = {}
}

struct OCRPanelView: View {
    @ObservedObject var viewModel: OCRPanelViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("OCR")
                    .font(.headline)
                Spacer()
                Toggle("Boxes", isOn: $viewModel.showTextRegions)
                    .toggleStyle(.checkbox)
                    .disabled(viewModel.result?.regions.isEmpty ?? true)
            }

            HStack(spacing: 8) {
                Button {
                    viewModel.onRunMacOCR()
                } label: {
                    if viewModel.isRunning {
                        HStack(spacing: 5) { ProgressView().controlSize(.small); Text("Reading…") }
                    } else {
                        Label("Mac OCR", systemImage: "text.viewfinder")
                    }
                }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(viewModel.isRunning)

                Button {
                    viewModel.onRunLLMOCR()
                } label: {
                    Label("Improve with LLM…", systemImage: "sparkles")
                }
                .buttonStyle(SecondaryButtonStyle())
                .disabled(viewModel.isRunning)
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

            TextEditor(text: .constant(displayText))
                .font(.system(.caption, design: .monospaced))
                .scrollContentBackground(.hidden)
                .background(RoundedRectangle(cornerRadius: 8).fill(.primary.opacity(0.05)))
                .frame(minHeight: 130)

            HStack {
                Button("Copy Text") { viewModel.onCopyPlainText() }
                    .buttonStyle(SecondaryButtonStyle())
                    .disabled(viewModel.result == nil)
                Button("Copy Markdown") { viewModel.onCopyMarkdown() }
                    .buttonStyle(SecondaryButtonStyle())
                    .disabled(viewModel.result == nil)
            }

            if let warnings = viewModel.result?.warnings, !warnings.isEmpty {
                ForEach(warnings, id: \.self) { warning in
                    Label(warning, systemImage: "exclamationmark.triangle")
                        .font(.caption2)
                        .foregroundStyle(.orange)
                }
            }

            if let error = viewModel.errorMessage {
                Label(error, systemImage: "xmark.octagon.fill")
                    .font(.caption)
                    .foregroundStyle(.red)
                    .lineLimit(3)
            }
        }
    }

    private var displayText: String {
        guard let result = viewModel.result else { return "No OCR text yet." }
        switch viewModel.activeDisplayMode {
        case .text:
            return OCRResultFormatter.plainText(from: result)
        case .markdown:
            return OCRResultFormatter.markdown(from: result)
        }
    }
}

