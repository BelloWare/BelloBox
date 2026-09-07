import AppKit
import SwiftUI

/// The interactive preview of a developer tool. It edits the same
/// `UtilityWorkbenchModel` the full tool opens on Enter, so every option and
/// draft carries over. Calculations are the workbench's own: debounced,
/// bounded, off the main actor, and guarded against stale results.
struct LauncherWorkbenchPreviewView: View {
    @ObservedObject var model: UtilityWorkbenchModel
    let preview: LauncherPreview?
    let height: CGFloat?
    var onEscape: () -> Void
    var onOpen: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if model.draftExceedsPreviewLimit {
                // A paste, a loaded snippet, "Use as Input" or editing in the
                // full tool grew the draft: the row shows neither editors nor a
                // stale result, and Enter opens the complete draft.
                LauncherDraftLimitNotice(byteCount: max(model.input.utf8.count, model.secondInput.utf8.count),
                    detail: "The complete draft stays in \(model.command.title); nothing has been truncated. Open it to keep working, or replace the input to preview here again.",
                    openTitle: "Open \(model.command.title)", onOpen: onOpen)
            } else {
                header
                controls
                output.frame(maxWidth: .infinity, maxHeight: .infinity)
                footer
            }
        }
        .padding(.horizontal, 10).padding(.top, 4).padding(.bottom, 12)
        .frame(height: height, alignment: .top)
        .clipped()
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(model.command.title) preview")
    }

    // MARK: Header and footer

    private var isUnrecognized: Bool { model.error != nil && !model.busy }
    /// Always the live state of the draft, never the original selection's summary.
    private var headerTitle: String {
        if model.busy { return model.sending ? "Sending…" : "Working…" }
        if model.error != nil { return "Not recognized for \(model.command.title)" }
        if let result = model.result, !result.status.isEmpty { return result.status }
        return model.command.title
    }
    private var headerSubtitle: String? {
        if model.error != nil { return "Edit the input in the full tool ↵" }
        switch model.command {
        case .json: return model.result.map { "\($0.text.count.formatted()) characters · complete output below" }
        case .compare: return model.secondInput.isEmpty ? "Add a second text below" : nil
        case .jwt: return nil
        case .regex: return model.regexPattern.isEmpty ? "Type a pattern to see live matches" : nil
        case .url: return nil
        case .time: return nil
        case .cron: return model.zoneID
        case .convert: return model.result?.table.map { "\($0.totalRows) rows · \($0.columns.count) columns" }
        case .snippets: return model.customFields.isEmpty ? "No {{fields}} to fill" : "\(model.customFields.count) field\(model.customFields.count == 1 ? "" : "s") to fill"
        case .http: return "Nothing is sent until you choose Send"
        case .generate: return "The selection is not needed"
        default: return nil
        }
    }
    private var header: some View {
        LauncherPreviewHeader(title: headerTitle, subtitle: headerSubtitle, warning: isUnrecognized || model.command == .jwt) {
            if model.command == .json {
                LauncherChoiceBar(selection: $model.jsonMode, choices: [("Pretty-print", "Pretty"), ("Minify", "Minify"), ("Validate", "Validate")], label: "JSON action")
            } else if model.command == .regex {
                LauncherChoiceBar(selection: $model.regexOutput, choices: [("Matches", "Matches"), ("Extract", "Extract"), ("Replace", "Replace")], label: "Regex output")
            } else if model.command == .compare {
                LauncherChoiceBar(selection: $model.comparisonMode, choices: ComparisonMode.allCases.map { ($0, $0.rawValue) }, label: "Compare")
            } else if model.command == .generate {
                LauncherChoiceBar(selection: $model.generatorKind, choices: GeneratorKind.allCases.map { ($0, $0 == .random ? "Random" : $0 == .records ? "Records" : $0.rawValue) }, label: "Generate")
            }
        }
    }

    private var footerText: String {
        if let error = model.error { return error }
        if model.busy { return model.sending ? "Waiting for the server…" : "Calculating…" }
        if let message = model.message { return message }
        switch model.command {
        case .jwt: return "Decoded locally · signature not verified"
        case .url: return "+ stays a literal plus · edits rebuild the URL below"
        case .http: return "Redirects are shown, not followed · nothing is saved"
        case .regex: return model.result?.status ?? "ICU regular expressions"
        case .snippets: return "Name and save it in the full tool ↵ · date and timestamp use UTC"
        case .compare: return model.result.map { "\($0.comparison?.added ?? 0) added · \($0.comparison?.removed ?? 0) removed" } ?? "Paste, pin, or type the second text"
        default: return model.result?.status ?? ""
        }
    }
    private var footer: some View {
        HStack(spacing: 8) {
            LauncherStatusLine(text: footerText, warning: model.error != nil)
            Spacer(minLength: 6)
            if model.busy {
                ProgressView().controlSize(.mini)
                Button("Cancel", action: model.cancel).buttonStyle(LauncherChipButtonStyle())
            }
            if model.canChain, model.command != .url {
                Button("Use as Input", action: model.useOutputAsInput).buttonStyle(LauncherChipButtonStyle())
                    .help("Continue with this result as the input")
            }
            Button { copy() } label: { Label("Copy", systemImage: "doc.on.doc") }
                .buttonStyle(LauncherChipButtonStyle()).disabled(!canCopy)
                .help("Copy the complete result")
                .accessibilityIdentifier("launcherPreviewCopy")
        }
    }
    private var canCopy: Bool {
        if model.command == .url { return model.urlDraft != nil && !model.busy }
        return !model.output.isEmpty && !model.busy && model.error == nil
    }
    private func copy() {
        if model.command == .url { model.buildURL() }
        model.copyOutput()
    }

    // MARK: Controls

    @ViewBuilder private var controls: some View {
        switch model.command {
        case .compare: compareControls
        case .regex: regexControls
        case .url: urlControls
        case .time: timeControls
        case .cron: cronControls
        case .convert: convertControls
        case .snippets: snippetControls
        case .http: httpControls
        case .generate: generateControls
        default: EmptyView()
        }
    }

    private var compareControls: some View {
        HStack(spacing: 8) {
            Toggle("Ignore whitespace", isOn: $model.ignoreWhitespace).toggleStyle(.checkbox).controlSize(.mini)
                .disabled(model.comparisonMode != .lines).previewCaption()
            Spacer(minLength: 4)
            Text("Second text").previewCaption()
            Button("Paste") { model.pasteInput(second: true) }.buttonStyle(LauncherChipButtonStyle()).help("Use the clipboard as the second text")
            Button("Use Pinned", action: model.usePinned).buttonStyle(LauncherChipButtonStyle()).help("Compare with the text you pinned earlier")
            Button("Pin First", action: model.pin).buttonStyle(LauncherChipButtonStyle()).help("Keep the first text until Bello Box quits, then compare it with another selection")
        }
    }

    private var regexControls: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                LauncherPreviewField(text: $model.regexPattern, placeholder: "Pattern, for example [A-Z]+-\\d+", label: "Regular expression", onEscape: onEscape)
                Toggle("Ignore case", isOn: $model.regexIgnoreCase).toggleStyle(.checkbox).controlSize(.mini).previewCaption().fixedSize()
                Toggle("Multiline", isOn: $model.regexMultiline).toggleStyle(.checkbox).controlSize(.mini).previewCaption().fixedSize()
            }
            if model.regexOutput == "Replace" {
                HStack(spacing: 8) {
                    Text("Replace with").previewCaption()
                    LauncherPreviewField(text: $model.replacement, placeholder: "$1, $2… for groups", label: "Replacement", onEscape: onEscape)
                }
            }
        }
    }

    @ViewBuilder private var urlControls: some View {
        if model.urlDraft != nil {
            HStack(spacing: 6) {
                LauncherChoiceBar(selection: urlBinding(\.scheme), choices: [("https", "https"), ("http", "http")], label: "Scheme")
                LauncherPreviewField(text: urlBinding(\.host), placeholder: "host", label: "Host", onEscape: onEscape)
                Text(":").previewCaption()
                LauncherPreviewField(text: urlBinding(\.port), placeholder: "port", label: "Port", onEscape: onEscape).frame(width: 54)
                LauncherPreviewField(text: urlBinding(\.path), placeholder: "/path", label: "Path", onEscape: onEscape)
                Text("#").previewCaption()
                LauncherPreviewField(text: urlBinding(\.fragment), placeholder: "fragment", label: "Fragment", onEscape: onEscape).frame(width: 96)
            }
        }
    }
    private func urlBinding(_ keyPath: WritableKeyPath<URLInspection, String>) -> Binding<String> {
        Binding(get: { model.urlDraft?[keyPath: keyPath] ?? "" }, set: { model.urlDraft?[keyPath: keyPath] = $0 })
    }

    private var timeControls: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                LauncherPreviewField(text: $model.input, placeholder: "Unix seconds, milliseconds, or an ISO date", label: "Timestamp", onEscape: onEscape)
                LauncherChoiceBar(selection: $model.epochUnit, choices: EpochUnit.allCases.map { ($0, $0 == .milliseconds ? "ms" : $0 == .seconds ? "s" : "Auto") }, label: "Numeric input")
                Button("Now") { model.input = String(Int64(Date().timeIntervalSince1970)); model.epochUnit = .seconds }
                    .buttonStyle(LauncherChipButtonStyle()).help("Use the current time")
            }
            HStack(spacing: 8) {
                zoneMenu
                Text("Compare with").previewCaption()
                LauncherPreviewField(text: $model.secondInput, placeholder: "Optional second timestamp", label: "Second timestamp", onEscape: onEscape)
            }
        }
    }

    private var cronControls: some View {
        HStack(spacing: 8) {
            LauncherPreviewField(text: $model.input, placeholder: "*/15 9-17 * * MON-FRI", label: "Cron expression", onEscape: onEscape)
            zoneMenu
        }
    }
    private var zoneMenu: some View {
        Menu {
            Button("My time zone (\(TimeZone.current.identifier))") { model.zoneID = TimeZone.current.identifier }
            ForEach(["UTC", "Asia/Singapore", "Asia/Tokyo", "Asia/Kolkata", "Europe/London", "Europe/Berlin", "America/New_York", "America/Los_Angeles"], id: \.self) { zone in
                Button(zone) { model.zoneID = zone }
            }
        } label: {
            Label(model.zoneID, systemImage: "globe").font(.system(size: 10)).lineLimit(1)
        }
        .menuStyle(.borderlessButton).fixedSize()
        .help("Time zone for the result; type any IANA zone in the full tool")
        .accessibilityIdentifier("launcherPreviewZone")
    }

    private var convertControls: some View {
        HStack(spacing: 8) {
            LauncherChoiceBar(selection: $model.fromFormat, choices: DataFormat.allCases.map { ($0, $0.rawValue) }, label: "From")
            Image(systemName: "arrow.right").font(.system(size: 9)).foregroundStyle(.secondary)
            LauncherChoiceBar(selection: $model.toFormat, choices: DataFormat.allCases.map { ($0, $0.rawValue) }, label: "To")
            if model.fromFormat == .csv || model.toFormat == .csv {
                Menu {
                    Button("Comma") { model.delimiter = "," }
                    Button("Tab") { model.delimiter = "\t" }
                    Button("Semicolon") { model.delimiter = ";" }
                } label: {
                    Text(model.delimiter == "\t" ? "Tab" : model.delimiter == ";" ? "Semicolon" : "Comma").font(.system(size: 10))
                }.menuStyle(.borderlessButton).fixedSize().help("CSV delimiter")
                if model.fromFormat == .csv {
                    Toggle("Infer types", isOn: $model.inferTypes).toggleStyle(.checkbox).controlSize(.mini).previewCaption().fixedSize()
                        .help("Numbers, booleans, and null instead of strings")
                }
            }
            Spacer(minLength: 0)
        }
    }

    private var snippetControls: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Menu {
                    ForEach(model.snippets.snippets) { snippet in Button(snippet.name) { model.loadSnippet(snippet) } }
                    if model.snippets.snippets.isEmpty { Text("No snippets saved yet") }
                } label: {
                    Label(model.snippetID.flatMap { id in model.snippets.snippets.first { $0.id == id }?.name } ?? "Saved snippets (\(model.snippets.snippets.count))",
                          systemImage: "folder").font(.system(size: 10)).lineLimit(1)
                }.menuStyle(.borderlessButton).fixedSize().help("Load a saved snippet as the template")
                    .accessibilityIdentifier("launcherPreviewSnippets")
                Text("\(model.input.count.formatted()) characters · {{selection}}, {{date}}, {{timestamp}}, {{uuid}} are built in").previewCaption()
                Spacer(minLength: 0)
            }
            if !model.customFields.isEmpty {
                HStack(spacing: 8) {
                    ForEach(model.customFields.prefix(3), id: \.self) { field in
                        HStack(spacing: 4) {
                            Text(field).previewCaption().frame(maxWidth: 70, alignment: .trailing)
                            LauncherPreviewField(text: Binding(get: { model.snippetValues[field] ?? "" }, set: { model.snippetValues[field] = $0 }),
                                                 placeholder: "Value", label: "Field \(field)", monospaced: false, onEscape: onEscape)
                        }
                    }
                    if model.customFields.count > 3 { Text("+\(model.customFields.count - 3) more ↵").previewCaption() }
                }
            }
        }
    }

    private var httpControls: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Menu {
                    ForEach(Array(Set(["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", model.request.method])).sorted(), id: \.self) { method in
                        Button(method) { model.request.method = method }
                    }
                } label: { Text(model.request.method).font(.system(size: 10, weight: .semibold, design: .monospaced)) }
                    .menuStyle(.borderlessButton).fixedSize().accessibilityLabel("Method")
                LauncherPreviewField(text: $model.request.url, placeholder: "https://example.com", label: "Request URL", onEscape: onEscape)
                Button { model.sendRequest() } label: { Label("Send", systemImage: "paperplane") }
                    .buttonStyle(LauncherChipButtonStyle(prominent: true))
                    .disabled(model.busy || model.request.url.isEmpty)
                    .help("Send this request now. Nothing is sent otherwise.")
                    .accessibilityIdentifier("launcherPreviewSend")
            }
            Text(requestSummary).previewCaption()
        }.disabled(model.sending)
    }
    private var requestSummary: String {
        let headers = model.request.headers.split(separator: "\n").filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }.count
        let body = model.request.body.utf8.count
        return "\(headers) header\(headers == 1 ? "" : "s") · body \(body.formatted()) bytes · edit headers and body in the full tool ↵"
    }

    private var generateControls: some View {
        HStack(spacing: 8) {
            counter("items", value: $model.generatorCount, range: 1...1_000, step: model.generatorCount >= 100 ? 50 : model.generatorCount >= 10 ? 5 : 1)
            if model.generatorKind == .random { counter("chars", value: $model.generatorLength, range: 1...256, step: 4) }
            LauncherChoiceBar(selection: $model.generatorFormat, choices: GeneratorFormat.allCases.map { ($0, $0.rawValue) }, label: "Output")
            Spacer(minLength: 0)
            Button { model.schedule() } label: { Label("Regenerate", systemImage: "arrow.clockwise") }
                .buttonStyle(LauncherChipButtonStyle()).help("Generate new values")
        }
    }
    private func counter(_ unit: String, value: Binding<Int>, range: ClosedRange<Int>, step: Int) -> some View {
        HStack(spacing: 3) {
            Button { value.wrappedValue = max(range.lowerBound, value.wrappedValue - step) } label: { Image(systemName: "minus") }
                .buttonStyle(LauncherChipButtonStyle()).disabled(value.wrappedValue <= range.lowerBound).accessibilityLabel("Fewer \(unit)")
            Text("\(value.wrappedValue) \(unit)").font(.system(size: 10, weight: .medium)).monospacedDigit().frame(minWidth: 52)
            Button { value.wrappedValue = min(range.upperBound, value.wrappedValue + step) } label: { Image(systemName: "plus") }
                .buttonStyle(LauncherChipButtonStyle()).disabled(value.wrappedValue >= range.upperBound).accessibilityLabel("More \(unit)")
        }
    }

    // MARK: Output

    @ViewBuilder private var output: some View {
        switch model.command {
        case .compare: compareOutput
        case .url: urlOutput
        case .http: httpOutput
        default: textOutput(model.output, placeholder: placeholder)
        }
    }
    private var placeholder: String {
        switch model.command {
        case .regex: return model.regexPattern.isEmpty ? "Matches, groups, extracted text, or replacements appear here." : "No matches."
        case .time: return "Enter a Unix timestamp or an ISO date."
        case .cron: return "Enter a five-field cron expression."
        case .snippets: return "The rendered template appears here."
        default: return "Paste text or use the clipboard to begin."
        }
    }
    private func textOutput(_ text: String, placeholder: String) -> some View {
        LauncherOutputWell() {
            ZStack(alignment: .topLeading) {
                if let error = model.error {
                    Text(error).font(.system(size: 11)).foregroundStyle(BoxTheme.danger).textSelection(.enabled)
                        .padding(8).frame(maxWidth: .infinity, alignment: .leading)
                } else if !text.isEmpty {
                    LauncherOutputText(text: text, label: "\(model.command.title) result")
                } else if !model.busy {
                    Text(placeholder).font(.system(size: 11)).foregroundStyle(.secondary).padding(8)
                }
            }
        }
    }
    private var compareOutput: some View {
        HStack(spacing: 8) {
            LauncherPreviewEditor(text: $model.secondInput, label: "Second text")
                .frame(width: 210)
            LauncherOutputWell() {
                if let error = model.error {
                    Text(error).font(.system(size: 11)).foregroundStyle(BoxTheme.danger).padding(8).frame(maxWidth: .infinity, alignment: .topLeading)
                } else if let result = model.result, let comparison = result.comparison, !model.secondInput.isEmpty || !comparison.rows.isEmpty {
                    LauncherDiffOutput(result: result, comparison: comparison, mode: model.comparisonMode)
                } else if !model.busy {
                    Text("Type, paste, or use the pinned text on the left to see the differences.")
                        .font(.system(size: 11)).foregroundStyle(.secondary).padding(8).frame(maxWidth: .infinity, alignment: .topLeading)
                }
            }
        }
    }
    private var rebuiltURL: (text: String, error: String?) {
        guard let draft = model.urlDraft else { return (model.error ?? "", model.error) }
        do { return (try draft.rebuilt(), nil) } catch { return (error.localizedDescription, error.localizedDescription) }
    }
    private var urlOutput: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let draft = model.urlDraft {
                LauncherOutputWell() {
                    LauncherURLParameterList(draft: Binding(get: { model.urlDraft ?? draft }, set: { model.urlDraft = $0 }), onEscape: onEscape, onOpen: onOpen)
                }
            } else {
                textOutput("", placeholder: "Enter a complete http:// or https:// URL.")
            }
            let rebuilt = rebuiltURL
            HStack(spacing: 6) {
                Image(systemName: rebuilt.error == nil ? "link" : "exclamationmark.triangle").font(.system(size: 9))
                    .foregroundStyle(rebuilt.error == nil ? BoxTheme.accent : BoxTheme.warning)
                Text(rebuilt.text).font(.system(size: 10, design: .monospaced)).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                    .foregroundStyle(rebuilt.error == nil ? Color.primary : BoxTheme.warning)
                    .accessibilityIdentifier("launcherPreviewRebuiltURL")
            }
        }
    }
    private var httpOutput: some View {
        LauncherOutputWell() {
            ZStack(alignment: .topLeading) {
                if let error = model.error {
                    Text(error).font(.system(size: 11)).foregroundStyle(BoxTheme.danger).textSelection(.enabled).padding(8).frame(maxWidth: .infinity, alignment: .leading)
                } else if let result = model.result, !result.text.isEmpty {
                    LauncherOutputText(text: result.text, label: "Response")
                } else if model.sending {
                    HStack(spacing: 6) { ProgressView().controlSize(.small); Text("Waiting for the server…").font(.system(size: 11)).foregroundStyle(.secondary) }.padding(8)
                } else if !model.busy {
                    Text(model.result == nil ? "Import a cURL command or enter a URL, then choose Send." : "The response appears here after you choose Send.")
                        .font(.system(size: 11)).foregroundStyle(.secondary).padding(8)
                }
            }
        }
    }
}

/// Editable query parameters, one compact row each, in a scrolling list. Rows
/// are lazy and the row edits the first `compactLimit` parameters; the model
/// keeps every parameter, the rebuilt URL and Copy include all of them, and
/// the full tool edits the rest.
struct LauncherURLParameterList: View {
    @Binding var draft: URLInspection
    var onEscape: () -> Void
    var onOpen: () -> Void = {}
    static let compactLimit = 100

    private var shown: ArraySlice<URLParameter> { draft.parameters.prefix(Self.compactLimit) }
    private var hiddenCount: Int { max(0, draft.parameters.count - Self.compactLimit) }

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 4) {
                if draft.parameters.isEmpty {
                    Text("No query parameters").font(.system(size: 10)).foregroundStyle(.secondary).padding(.leading, 2)
                }
                ForEach(shown) { parameter in row(parameter) }
                if hiddenCount > 0 {
                    HStack(spacing: 6) {
                        Text("Editing the first \(Self.compactLimit) of \(draft.parameters.count.formatted()) parameters · all of them stay in the URL")
                            .previewCaption()
                        Button("Open for all", action: onOpen).buttonStyle(LauncherChipButtonStyle())
                            .help("Edit every parameter in the full tool")
                            .accessibilityIdentifier("launcherPreviewURLOpenAll")
                    }
                }
                Button { draft.parameters.append(URLParameter(name: "", value: "")) } label: { Label("Add parameter", systemImage: "plus") }
                    .buttonStyle(LauncherChipButtonStyle())
                    .disabled(hiddenCount > 0)
                    .help(hiddenCount > 0 ? "Add parameters in the full tool" : "Add a query parameter")
            }.padding(6)
        }
    }
    private func row(_ parameter: URLParameter) -> some View {
        HStack(spacing: 5) {
            LauncherPreviewField(text: binding(parameter.id, \.name), placeholder: "name", label: "Parameter name", onEscape: onEscape)
            Toggle("=", isOn: Binding(get: { draft.parameters.first(where: { $0.id == parameter.id })?.hasValue ?? true },
                                      set: { value in if let i = draft.parameters.firstIndex(where: { $0.id == parameter.id }) { draft.parameters[i].hasValue = value } }))
                .toggleStyle(.checkbox).controlSize(.mini).previewCaption().help("Include an equals sign; off makes a flag parameter")
            LauncherPreviewField(text: binding(parameter.id, \.value), placeholder: "value", label: "Parameter value", onEscape: onEscape)
                .disabled(!(draft.parameters.first(where: { $0.id == parameter.id })?.hasValue ?? true))
            Button { draft.parameters.removeAll { $0.id == parameter.id } } label: { Image(systemName: "minus.circle").font(.system(size: 11)) }
                .buttonStyle(.plain).foregroundStyle(.secondary).accessibilityLabel("Remove parameter")
        }
    }
    private func binding(_ id: UUID, _ keyPath: WritableKeyPath<URLParameter, String>) -> Binding<String> {
        Binding(get: { draft.parameters.first(where: { $0.id == id })?[keyPath: keyPath] ?? "" },
                set: { value in if let i = draft.parameters.firstIndex(where: { $0.id == id }) { draft.parameters[i][keyPath: keyPath] = value } })
    }
}

/// The diff, coloured per row, rendered once per calculation.
struct LauncherDiffOutput: View {
    let result: WorkbenchResult
    let comparison: ComparisonResult
    let mode: ComparisonMode
    @State private var cache: (id: UUID, text: NSAttributedString)?

    var body: some View {
        LauncherOutputText(text: "", attributed: attributed, label: "Differences")
            .onChange(of: result.id) { _ in cache = nil }
    }
    private var attributed: NSAttributedString {
        if let cache, cache.id == result.id { return cache.text }
        let text = Self.attributed(comparison, mode: mode)
        DispatchQueue.main.async { cache = (result.id, text) }
        return text
    }
    static func attributed(_ comparison: ComparisonResult, mode: ComparisonMode) -> NSAttributedString {
        let output = NSMutableAttributedString()
        let font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)
        for (index, row) in comparison.rows.enumerated() {
            let color: NSColor? = row.kind == .added ? NSColor.systemGreen.withAlphaComponent(0.18) : row.kind == .removed ? NSColor.systemRed.withAlphaComponent(0.16) : nil
            var attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: NSColor.labelColor]
            if let color { attributes[.backgroundColor] = color }
            if row.kind == .removed, mode == .words { attributes[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
            let prefix = mode == .words ? "" : (row.kind == .added ? "+ " : row.kind == .removed ? "− " : "  ")
            let separator = mode == .words ? (index == comparison.rows.count - 1 ? "" : " ") : "\n"
            output.append(NSAttributedString(string: prefix + row.text, attributes: attributes))
            output.append(NSAttributedString(string: separator, attributes: [.font: font]))
        }
        return output
    }
}
