import SwiftUI

/// The popup shown when the user clicks the floating button: a selected-text
/// preview, one-click actions, a custom prompt, and the streamed result.
struct ActionPopupView: View {
    static let preferredSize = CGSize(width: 388, height: 440)

    @ObservedObject var viewModel: ActionPopupViewModel

    private let columns = [GridItem(.adaptive(minimum: 168), spacing: 8)]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            selectionPreview

            if !viewModel.isConfigured {
                setupBanner
            }

            actionsGrid
            customPromptRow

            if viewModel.didRun {
                resultSection
            } else {
                Spacer(minLength: 0)
                footerHint
            }
        }
        .padding(16)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height, alignment: .topLeading)
        .background(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(.regularMaterial)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(.primary.opacity(0.08), lineWidth: 1)
        )
        .onExitCommand { viewModel.close() }
    }

    // MARK: - Sections

    private var header: some View {
        HStack(spacing: 8) {
            Image(systemName: "wand.and.stars")
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(BoxTheme.accent)
            Text("BelloBox")
                .font(.headline)
            Spacer()
            Text(viewModel.providerSummary)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
            Button {
                viewModel.close()
            } label: {
                Image(systemName: "xmark.circle.fill")
                    .font(.system(size: 14))
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("Close")
        }
    }

    private var selectionPreview: some View {
        ScrollView {
            Text(viewModel.selection.text)
                .font(.callout)
                .foregroundStyle(.primary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .textSelection(.enabled)
        }
        .frame(height: 56)
        .padding(8)
        .background(
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .fill(BoxTheme.accentSoft)
        )
    }

    private var setupBanner: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.orange)
            Text("No AI provider configured.")
                .font(.caption)
            Spacer()
            Button("Open Settings") { viewModel.openSettings() }
                .font(.caption)
        }
        .padding(8)
        .background(RoundedRectangle(cornerRadius: 8).fill(.orange.opacity(0.12)))
    }

    private var actionsGrid: some View {
        LazyVGrid(columns: columns, alignment: .leading, spacing: 8) {
            ForEach(viewModel.quickActions) { action in
                Button {
                    viewModel.run(action)
                } label: {
                    Label(action.title, systemImage: action.symbol)
                        .font(.caption)
                        .lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, 6)
                        .padding(.horizontal, 8)
                        .background(RoundedRectangle(cornerRadius: 8).fill(.primary.opacity(0.06)))
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .disabled(viewModel.isStreaming)
            }
        }
    }

    private var customPromptRow: some View {
        HStack(spacing: 8) {
            TextField("Ask BelloBox to…", text: $viewModel.instruction)
                .textFieldStyle(.roundedBorder)
                .onSubmit { viewModel.runCustom() }
            Button {
                viewModel.runCustom()
            } label: {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.system(size: 18))
                    .foregroundStyle(BoxTheme.accent)
            }
            .buttonStyle(.plain)
            .disabled(viewModel.isStreaming || viewModel.instruction.trimmingCharacters(in: .whitespaces).isEmpty)
            .help("Run custom instruction")
        }
    }

    private var resultSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                if viewModel.isStreaming {
                    ProgressView().controlSize(.small)
                    Text("Thinking…").font(.caption).foregroundStyle(.secondary)
                } else {
                    Text("Result").font(.caption.bold()).foregroundStyle(.secondary)
                }
                Spacer()
                if viewModel.isStreaming {
                    Button("Stop") { viewModel.cancel() }.font(.caption)
                }
            }

            ScrollView {
                if let error = viewModel.errorMessage {
                    Text(error)
                        .font(.callout)
                        .foregroundStyle(.red)
                        .frame(maxWidth: .infinity, alignment: .leading)
                } else {
                    Text(viewModel.resultText.isEmpty ? " " : viewModel.resultText)
                        .font(.callout)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .textSelection(.enabled)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(8)
            .background(RoundedRectangle(cornerRadius: 9).fill(.primary.opacity(0.05)))

            HStack(spacing: 8) {
                Spacer()
                Button {
                    viewModel.copyResult()
                } label: {
                    Label("Copy", systemImage: "doc.on.doc")
                }
                .disabled(!viewModel.canCopy)

                Button {
                    viewModel.replaceSelection()
                } label: {
                    Label("Replace", systemImage: "arrow.left.arrow.right")
                }
                .keyboardShortcut(.return, modifiers: [.command])
                .disabled(!viewModel.canReplace)
                .help("Replace the original selection with this result")
            }
        }
        .frame(maxHeight: .infinity, alignment: .top)
    }

    private var footerHint: some View {
        Text("Tip: select text anywhere, then click the BelloBox button — or press ⌃⌥⌘B.")
            .font(.caption2)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }
}
