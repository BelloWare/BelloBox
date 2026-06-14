import AppKit
import SwiftUI

@MainActor
final class TextToolsPopupViewModel: ObservableObject {
    enum Category: String, CaseIterable, Identifiable {
        case caseConvert = "Case"
        case encode = "Encode"
        case decode = "Decode"
        case pretty = "Pretty"
        case hash = "Hash"
        case lines = "Lines"
        case count = "Count"

        var id: String { rawValue }
        var symbol: String {
            switch self {
            case .caseConvert: return "textformat"
            case .encode: return "lock"
            case .decode: return "lock.open"
            case .pretty: return "chevron.left.forward.slash.chevron.right"
            case .hash: return "number"
            case .lines: return "list.bullet"
            case .count: return "sum"
            }
        }
    }

    @Published var input: String
    @Published var category: Category = .caseConvert
    @Published var caseStyle: CaseConverter.Style = .upper
    @Published var encodeMethod: TextEncoder.Method = .base64
    @Published var decodeFormat: TextDecoder.Format = .auto
    @Published var lineOp: LineTool.Operation = .sortAscending

    private let selection: TextSelection
    private let accessibility: AccessibilityService
    private let settings: AppSettings

    var onClose: () -> Void = {}

    init(selection: TextSelection, settings: AppSettings, accessibility: AccessibilityService) {
        self.selection = selection
        self.input = selection.text
        self.accessibility = accessibility
        self.settings = settings
    }

    var model: String { settings.currentConfig.model }
    var provider: ProviderKind { settings.providerKind }

    // Outputs
    var caseOutput: String { CaseConverter.convert(input, to: caseStyle) }
    var encodeOutput: String { TextEncoder.encode(input, encodeMethod) }
    var decodeResult: TextDecoder.Decoded? { TextDecoder.decode(input, as: decodeFormat) }
    var prettyResult: PrettyPrinter.Result? { PrettyPrinter.prettyPrint(input) }
    var lineOutput: String { LineTool.apply(input, lineOp) }
    var hashes: [(HashTool.Algorithm, String)] {
        HashTool.Algorithm.allCases.map { ($0, HashTool.hash(input, $0)) }
    }
    var stats: [(String, String)] {
        [
            ("Characters", "\(TextStats.characters(input))"),
            ("Characters (no spaces)", "\(TextStats.charactersNoSpaces(input))"),
            ("Words", "\(TextStats.words(input))"),
            ("Lines", "\(TextStats.lines(input))"),
        ]
    }
    var tokenEstimate: Int { TokenEstimator.estimate(input, model: model, provider: provider) }
    var tokenFamily: String { TokenEstimator.familyLabel(model: model, provider: provider) }
    var modelLabel: String { model.isEmpty ? "no model set" : model }

    var primaryOutput: String? {
        switch category {
        case .caseConvert: return caseOutput
        case .encode: return encodeOutput
        case .decode: return decodeResult?.output
        case .pretty: return prettyResult?.output
        case .lines: return lineOutput
        case .hash, .count: return nil
        }
    }

    func copy(_ text: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
    }

    func replace(_ text: String) {
        let pid = selection.pid
        onClose()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [accessibility] in
            accessibility.replaceSelection(with: text, pid: pid)
        }
    }

    func close() { onClose() }
}

struct TextToolsPopupView: View {
    static let preferredSize = CGSize(width: 720, height: 760)

    @ObservedObject var viewModel: TextToolsPopupViewModel
    @ObservedObject var settings: AppSettings

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            categoryBar
            inputField
            Divider()
            content
            Spacer(minLength: 0)
        }
        .padding(16)
        .frame(width: Self.preferredSize.width, height: Self.preferredSize.height, alignment: .topLeading)
        .popupCard()
        .appearPop()
        .onExitCommand { viewModel.close() }
    }

    private var header: some View {
        PopupHeader(icon: "wrench.and.screwdriver", title: "Text Tools") { viewModel.close() }
    }

    private var categoryBar: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 100), spacing: 8)], alignment: .leading, spacing: 8) {
            ForEach(TextToolsPopupViewModel.Category.allCases) { category in
                let selected = viewModel.category == category
                Button {
                    viewModel.category = category
                } label: {
                    Label(category.rawValue, systemImage: category.symbol)
                        .font(.caption.weight(.semibold))
                        .lineLimit(1)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 5)
                        .padding(.horizontal, 8)
                        .background(
                            Capsule().fill(selected ? BoxTheme.accent : Color.primary.opacity(0.07))
                        )
                        .foregroundStyle(selected ? .white : .primary)
                        .contentShape(Capsule())
                }
                .buttonStyle(.plain)
            }
        }
    }

    private var inputField: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text("Input").font(.caption2.bold()).foregroundStyle(.secondary)
            TextEditor(text: $viewModel.input)
                .font(.callout)
                .scrollContentBackground(.hidden)
                .frame(height: 120)
                .padding(6)
                .background(RoundedRectangle(cornerRadius: 8).fill(.primary.opacity(0.05)))
        }
    }

    @ViewBuilder
    private var content: some View {
        switch viewModel.category {
        case .caseConvert:
            VStack(alignment: .leading, spacing: 10) {
                optionGrid(CaseConverter.Style.allCases, isSelected: { $0 == viewModel.caseStyle }, label: { $0.rawValue }) {
                    viewModel.caseStyle = $0
                }
                outputBlock(viewModel.caseOutput)
            }
        case .encode:
            VStack(alignment: .leading, spacing: 10) {
                optionGrid(TextEncoder.Method.allCases, isSelected: { $0 == viewModel.encodeMethod }, label: { $0.rawValue }) {
                    viewModel.encodeMethod = $0
                }
                outputBlock(viewModel.encodeOutput)
            }
        case .decode:
            decodeControls
        case .pretty:
            prettyContent
        case .hash:
            hashContent
        case .lines:
            VStack(alignment: .leading, spacing: 10) {
                optionGrid(LineTool.Operation.allCases, isSelected: { $0 == viewModel.lineOp }, label: { $0.rawValue }) {
                    viewModel.lineOp = $0
                }
                outputBlock(viewModel.lineOutput)
            }
        case .count:
            countContent
        }
    }

    // MARK: - Category content

    private var decodeControls: some View {
        VStack(alignment: .leading, spacing: 8) {
            Picker("Format", selection: $viewModel.decodeFormat) {
                ForEach(TextDecoder.Format.allCases) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.menu)
            .fixedSize()

            if let result = viewModel.decodeResult {
                Text("Detected: \(result.format)").font(.caption).foregroundStyle(BoxTheme.accent)
                outputBlock(result.output)
            } else {
                notice("Couldn't decode this text. Try choosing a format above.")
            }
        }
    }

    private var prettyContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let result = viewModel.prettyResult {
                Text("Detected: \(result.language)").font(.caption).foregroundStyle(BoxTheme.accent)
                outputBlock(result.output)
            } else {
                notice("Couldn't detect a formattable language (try JSON, XML/HTML, or CSS).")
            }
        }
    }

    private var hashContent: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(viewModel.hashes, id: \.0) { algorithm, value in
                HStack(spacing: 8) {
                    Text(algorithm.rawValue).font(.caption.bold()).frame(width: 80, alignment: .leading)
                    Text(value)
                        .font(.system(.caption, design: .monospaced))
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Button { viewModel.copy(value) } label: { Image(systemName: "doc.on.doc") }
                        .buttonStyle(.plain)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 5)
                .padding(.horizontal, 8)
                .background(RoundedRectangle(cornerRadius: 8).fill(.primary.opacity(0.05)))
            }
        }
    }

    private var countContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(viewModel.stats, id: \.0) { label, value in
                statRow(label, value)
            }
            statRow("≈ Tokens", "\(viewModel.tokenEstimate)", accent: true)

            modelControl

            Text("Token estimate for \(viewModel.modelLabel) · \(viewModel.tokenFamily).")
                .font(.caption2)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var modelControl: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Token model").font(.caption2.bold()).foregroundStyle(.secondary)
            Picker("Provider", selection: $settings.providerKind) {
                ForEach(ProviderKind.allCases) { Text($0.shortName).tag($0) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()

            HStack(spacing: 6) {
                TextField("model", text: modelBinding)
                    .textFieldStyle(.roundedBorder)
                    .font(.callout)
                    .autocorrectionDisabled()
                Menu {
                    ForEach(presetModels, id: \.self) { name in
                        Button(name) { setModel(name) }
                    }
                } label: {
                    Image(systemName: "chevron.down.circle.fill")
                        .font(.system(size: 16))
                        .foregroundStyle(BoxTheme.accent)
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
                .help("Pick a common model")
            }
        }
        .toolPanel()
    }

    private var modelBinding: Binding<String> {
        switch settings.providerKind {
        case .openAI: return $settings.openAIModel
        case .anthropic: return $settings.anthropicModel
        case .codexCLI: return $settings.codexModel
        }
    }

    private var presetModels: [String] {
        switch settings.providerKind {
        case .openAI: return ["gpt-4o-mini", "gpt-4o", "gpt-4.1-mini", "gpt-4.1", "o3-mini", "gpt-3.5-turbo"]
        case .anthropic: return ["claude-3-5-haiku-latest", "claude-3-5-sonnet-latest", "claude-3-7-sonnet-latest", "claude-3-opus-latest"]
        case .codexCLI: return CodexCLI.presetModels
        }
    }

    private func setModel(_ name: String) {
        switch settings.providerKind {
        case .openAI: settings.openAIModel = name
        case .anthropic: settings.anthropicModel = name
        case .codexCLI: settings.codexModel = name
        }
    }

    // MARK: - Reusable pieces

    private func optionGrid<T: Identifiable>(
        _ options: [T],
        isSelected: @escaping (T) -> Bool,
        label: @escaping (T) -> String,
        action: @escaping (T) -> Void
    ) -> some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 130), spacing: 8)], alignment: .leading, spacing: 8) {
            ForEach(options) { option in
                let selected = isSelected(option)
                Button { action(option) } label: {
                    Text(label(option))
                        .font(.caption)
                        .lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, 5)
                        .padding(.horizontal, 8)
                        .background(RoundedRectangle(cornerRadius: 7).fill(selected ? BoxTheme.accentSoft : Color.primary.opacity(0.05)))
                        .overlay(
                            RoundedRectangle(cornerRadius: 7).strokeBorder(selected ? BoxTheme.accent : .clear, lineWidth: 1)
                        )
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func outputBlock(_ text: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            ScrollView {
                Text(text.isEmpty ? " " : text)
                    .font(.system(.callout, design: .monospaced))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
            }
            .frame(height: 300)
            .padding(8)
            .background(RoundedRectangle(cornerRadius: 8).fill(.primary.opacity(0.05)))

            HStack {
                Spacer()
                Button { viewModel.copy(text) } label: { Label("Copy", systemImage: "doc.on.doc") }
                    .buttonStyle(SecondaryButtonStyle())
                    .disabled(text.isEmpty)
                Button { viewModel.replace(text) } label: { Label("Replace", systemImage: "arrow.left.arrow.right") }
                    .buttonStyle(PrimaryButtonStyle())
                    .disabled(text.isEmpty)
            }
        }
    }

    private func statRow(_ label: String, _ value: String, accent: Bool = false) -> some View {
        HStack {
            Text(label).font(.callout)
            Spacer()
            Text(value)
                .font(.system(.callout, design: .monospaced).weight(.semibold))
                .foregroundStyle(accent ? BoxTheme.accent : .primary)
        }
        .padding(.vertical, 6)
        .padding(.horizontal, 10)
        .background(RoundedRectangle(cornerRadius: 8).fill(.primary.opacity(0.05)))
    }

    private func notice(_ text: String) -> some View {
        Text(text)
            .font(.caption)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(10)
            .background(RoundedRectangle(cornerRadius: 8).fill(.primary.opacity(0.04)))
    }
}
