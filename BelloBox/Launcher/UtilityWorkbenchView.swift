import SwiftUI

struct UtilityWorkbenchView: View {
    @ObservedObject var model: UtilityWorkbenchModel
    var onBack: () -> Void
    @State private var showDeleteSnippet = false

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Button(action: onBack) { Image(systemName: "chevron.left").font(.headline) }
                    .buttonStyle(SecondaryButtonStyle()).help("All tools (⌘K or Esc)").accessibilityLabel("Back to all tools")
                Image(systemName: model.command.symbol).font(.title3).foregroundStyle(BoxTheme.accent)
                VStack(alignment: .leading, spacing: 3) {
                    Text(model.command.title).font(.headline)
                    Text(model.command == .http ? "Requests run when you choose Send" : "Processed on your Mac")
                        .font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                Text("⌘K  All tools").font(.caption).foregroundStyle(.secondary)
            }.padding(18)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    controls
                    inputArea
                    extraControls
                    resultArea
                }.padding(20)
            }
            Divider()
            footer
        }
    }

    @ViewBuilder private var controls: some View {
        switch model.command {
        case .json:
            Picker("Action", selection: $model.jsonMode) { ForEach(["Pretty-print", "Minify", "Validate"], id: \.self) { Text($0) } }
                .pickerStyle(.segmented).labelsHidden()
        case .compare:
            HStack {
                Picker("Compare", selection: $model.comparisonMode) { ForEach(ComparisonMode.allCases, id: \.self) { Text($0.rawValue).tag($0) } }.pickerStyle(.segmented)
                Toggle("Ignore whitespace", isOn: $model.ignoreWhitespace).disabled(model.comparisonMode != .lines)
            }
        case .regex:
            VStack(alignment: .leading, spacing: 10) {
                TextField("Pattern, for example [A-Z]+-\\d+", text: $model.regexPattern).textFieldStyle(.roundedBorder).font(.system(.body, design: .monospaced)).accessibilityLabel("Regular expression")
                HStack {
                    Toggle("Ignore case", isOn: $model.regexIgnoreCase)
                    Toggle("Multiline anchors", isOn: $model.regexMultiline)
                    Spacer()
                    Text("ICU engine").font(.caption).foregroundStyle(.secondary)
                }
                Picker("Output", selection: $model.regexOutput) { ForEach(["Matches", "Extract", "Replace"], id: \.self) { Text($0) } }.pickerStyle(.segmented)
                if model.regexOutput == "Replace" {
                    TextField("Replacement ($1, $2… for groups)", text: $model.replacement).textFieldStyle(.roundedBorder)
                }
            }
        case .time:
            HStack {
                Picker("Numeric input", selection: $model.epochUnit) { ForEach(EpochUnit.allCases, id: \.self) { Text($0.rawValue).tag($0) } }
                zonePicker
                Button("Now") { model.input = String(Int64(Date().timeIntervalSince1970)); model.epochUnit = .seconds }
            }
        case .cron: zonePicker
        case .convert:
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Picker("From", selection: $model.fromFormat) { ForEach(DataFormat.allCases, id: \.self) { Text($0.rawValue).tag($0) } }
                    Image(systemName: "arrow.right").foregroundStyle(.secondary)
                    Picker("To", selection: $model.toFormat) { ForEach(DataFormat.allCases, id: \.self) { Text($0.rawValue).tag($0) } }
                }
                if model.fromFormat == .csv || model.toFormat == .csv {
                    HStack {
                        Picker("Delimiter", selection: $model.delimiter) { Text("Comma").tag(","); Text("Tab").tag("\t"); Text("Semicolon").tag(";") }
                        if model.fromFormat == .csv { Toggle("Infer numbers, booleans, and null", isOn: $model.inferTypes) }
                    }
                    Text("The first CSV row contains column names. Values remain strings unless inference is enabled.").font(.caption).foregroundStyle(.secondary)
                }
            }
        case .snippets: snippetControls
        case .generate:
            VStack(alignment: .leading, spacing: 12) {
                Picker("Generate", selection: $model.generatorKind) { ForEach(GeneratorKind.allCases, id: \.self) { Text($0.rawValue).tag($0) } }.pickerStyle(.segmented)
                HStack {
                    Stepper("\(model.generatorCount) items", value: $model.generatorCount, in: 1...1_000)
                    if model.generatorKind == .random { Stepper("Length \(model.generatorLength)", value: $model.generatorLength, in: 1...256) }
                    Picker("Output", selection: $model.generatorFormat) { ForEach(GeneratorFormat.allCases, id: \.self) { Text($0.rawValue).tag($0) } }
                    Button("Regenerate", action: model.schedule).buttonStyle(SecondaryButtonStyle())
                }
                if model.generatorKind == .timestamps { Text("Unix seconds, starting now, one second apart.").font(.caption).foregroundStyle(.secondary) }
            }
        default: EmptyView()
        }
    }

    private var zonePicker: some View {
        HStack {
            Text("Time zone").font(.caption).foregroundStyle(.secondary)
            TextField("Asia/Singapore", text: $model.zoneID).textFieldStyle(.roundedBorder).accessibilityLabel("Time zone")
            Menu("Choose") {
                Button("My time zone") { model.zoneID = TimeZone.current.identifier }
                ForEach(["UTC", "Asia/Singapore", "Asia/Tokyo", "Europe/London", "America/New_York", "America/Los_Angeles"], id: \.self) { zone in Button(zone) { model.zoneID = zone } }
            }.fixedSize()
        }
    }
    @ViewBuilder private var inputArea: some View {
        if model.command != .generate {
            if model.command == .compare {
                HStack(alignment: .top, spacing: 14) {
                    VStack(alignment: .leading, spacing: 8) {
                        inputLabel("First text", second: false)
                        editor("First text", text: $model.input, height: 190)
                        Button("Pin this text", action: model.pin).buttonStyle(.link)
                    }
                    VStack(alignment: .leading, spacing: 8) {
                        inputLabel("Second text", second: true)
                        editor("Second text", text: $model.secondInput, height: 190)
                        Button("Use pinned text", action: model.usePinned).buttonStyle(.link)
                    }
                }
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    inputLabel(model.command == .snippets ? "Template" : model.command == .http ? "Import cURL or URL" : "Input", second: false)
                    editor(model.command == .snippets ? "Snippet template" : "Utility input", text: $model.input, height: inputHeight)
                    if model.command == .time { TextField("Optional second timestamp to compare", text: $model.secondInput).textFieldStyle(.roundedBorder) }
                }
            }
        }
    }
    private var inputHeight: CGFloat {
        switch model.command {
        case .time, .cron, .url: return 70
        case .http: return 80
        default: return 150
        }
    }
    private func inputLabel(_ title: String, second: Bool) -> some View {
        HStack {
            Text(title).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
            Spacer()
            if !second && ![LauncherCommand.snippets, .compare].contains(model.command) {
                Button("Example") { model.input = example }.buttonStyle(.link).font(.caption)
            }
            Button("Paste") { model.pasteInput(second: second) }.buttonStyle(.link).font(.caption)
            Button("Clear") { if second { model.secondInput = "" } else { model.input = "" } }.buttonStyle(.link).font(.caption)
        }
    }
    private func editor(_ title: String, text: Binding<String>, height: CGFloat) -> some View {
        TextEditor(text: text)
            .font(.system(size: 12, design: .monospaced))
            .scrollContentBackground(.hidden)
            .padding(9).frame(height: height)
            .background(RoundedRectangle(cornerRadius: 10).fill(Color(nsColor: .textBackgroundColor)))
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(.primary.opacity(0.1)))
            .accessibilityLabel(title)
    }
    @ViewBuilder private var extraControls: some View {
        if model.command == .url { urlFields }
        if model.command == .http { requestFields }
        if model.command == .snippets && !model.customFields.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("Fill template fields").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                ForEach(model.customFields, id: \.self) { field in
                    HStack {
                        Text(field).frame(width: 130, alignment: .leading)
                        TextField("Value", text: Binding(get: { model.snippetValues[field] ?? "" }, set: { model.snippetValues[field] = $0 })).textFieldStyle(.roundedBorder)
                    }
                }
            }
        }
        if model.command == .regex, let regex = model.result?.regex {
            VStack(alignment: .leading, spacing: 8) {
                Text("Matches in your text").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                ScrollView { Text(highlighted(regex.ranges)).font(.system(.body, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding(12) }
                    .frame(height: 120).background(RoundedRectangle(cornerRadius: 10).fill(.primary.opacity(0.035)))
            }
        }
    }
    @ViewBuilder private var resultArea: some View {
        if let error = model.error {
            Label(error, systemImage: "exclamationmark.circle").font(.callout).foregroundStyle(.red).textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
                .padding(12).frame(maxWidth: .infinity, alignment: .leading).background(RoundedRectangle(cornerRadius: 10).fill(.red.opacity(0.07)))
        }
        if let result = model.result {
            if let table = result.table { tablePreview(table) }
            if !result.text.isEmpty {
                HStack {
                    Text("Result").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                    Spacer()
                    Text(result.status).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                }
                if let comparison = result.comparison {
                    if model.comparisonMode == .words {
                        Text(wordDiff(comparison)).font(.system(.body, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding(12)
                    } else {
                        LazyVStack(alignment: .leading, spacing: 0) {
                            ForEach(comparison.rows) { row in
                                HStack(alignment: .top, spacing: 10) {
                                    Text(row.kind == .added ? "+" : row.kind == .removed ? "−" : " ").frame(width: 14)
                                    Text(row.text.isEmpty ? " " : row.text).frame(maxWidth: .infinity, alignment: .leading).textSelection(.enabled)
                                }.font(.system(size: 12, design: .monospaced)).padding(.horizontal, 10).padding(.vertical, 4)
                                    .background(diffColor(row.kind).opacity(0.1))
                            }
                        }.background(RoundedRectangle(cornerRadius: 10).fill(.primary.opacity(0.025)))
                    }
                } else {
                    GeometryReader { geometry in
                        ScrollView([.horizontal, .vertical]) {
                            Text(result.text).font(.system(size: 12, design: .monospaced)).textSelection(.enabled)
                                .fixedSize(horizontal: true, vertical: true)
                                .frame(minWidth: max(0, geometry.size.width - 24), minHeight: max(0, geometry.size.height - 24), alignment: .topLeading).padding(12)
                        }
                    }.frame(height: 210).background(RoundedRectangle(cornerRadius: 10).fill(Color(nsColor: .textBackgroundColor)))
                }
            } else if !result.status.isEmpty {
                Text(result.status).font(.callout).foregroundStyle(.secondary)
            }
        } else if !model.busy && model.error == nil {
            VStack(spacing: 8) {
                Image(systemName: model.command.symbol).font(.title).foregroundStyle(BoxTheme.accent)
                Text(model.command == .http ? "Import a cURL command or edit a request above." : model.command == .url && model.urlDraft != nil ? "Build the URL to preview your edits." : "Paste text or use an example to begin.").foregroundStyle(.secondary)
            }.frame(maxWidth: .infinity).padding(28)
        }
    }
    private var footer: some View {
        HStack(spacing: 10) {
            if model.busy {
                ProgressView().controlSize(.small)
                Text(model.sending ? "Sending…" : "Working…").font(.caption).foregroundStyle(.secondary)
                Button("Cancel", action: model.cancel).buttonStyle(SecondaryButtonStyle())
            } else if let message = model.message { Text(message).font(.caption).foregroundStyle(.secondary).lineLimit(2) }
            Spacer()
            if !model.busy && model.result == nil && !model.input.isEmpty && model.command != .http && model.command != .url {
                Button("Refresh", action: model.schedule).buttonStyle(SecondaryButtonStyle())
            }
            if model.canChain {
                Button("Use as Input", action: model.useOutputAsInput).buttonStyle(SecondaryButtonStyle())
            }
            Button(action: model.copyOutput) { Label("Copy Result", systemImage: "doc.on.doc") }
                .buttonStyle(SecondaryButtonStyle()).keyboardShortcut("c", modifiers: [.command, .shift])
                .disabled(model.output.isEmpty || model.busy || model.error != nil)
            if model.selection.pid != nil {
                Button("Replace Selection") { model.onReplace(model.output) }.buttonStyle(PrimaryButtonStyle()).disabled(!model.canReplace)
            }
        }.padding(16)
    }
    @ViewBuilder private var urlFields: some View {
        if model.urlDraft != nil {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Picker("Scheme", selection: Binding(get: { model.urlDraft?.scheme ?? "https" }, set: { model.urlDraft?.scheme = $0 })) { Text("https").tag("https"); Text("http").tag("http") }.frame(width: 135)
                    Text("Host").font(.caption).foregroundStyle(.secondary)
                    TextField("example.com", text: Binding(get: { model.urlDraft?.host ?? "" }, set: { model.urlDraft?.host = $0 })).textFieldStyle(.roundedBorder)
                    Text("Port").font(.caption).foregroundStyle(.secondary)
                    TextField("Default", text: Binding(get: { model.urlDraft?.port ?? "" }, set: { model.urlDraft?.port = $0 })).textFieldStyle(.roundedBorder).frame(width: 80)
                }
                HStack {
                    Text("Path").font(.caption).foregroundStyle(.secondary).frame(width: 55, alignment: .leading)
                    TextField("/", text: Binding(get: { model.urlDraft?.path ?? "" }, set: { model.urlDraft?.path = $0 })).textFieldStyle(.roundedBorder)
                }
                HStack {
                    Text("Fragment").font(.caption).foregroundStyle(.secondary).frame(width: 55, alignment: .leading)
                    TextField("Optional", text: Binding(get: { model.urlDraft?.fragment ?? "" }, set: { model.urlDraft?.fragment = $0 })).textFieldStyle(.roundedBorder)
                }
                Text("Query parameters").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                ForEach(model.urlDraft?.parameters ?? []) { parameter in
                    HStack {
                        TextField("Name", text: parameterBinding(parameter.id, name: true)).textFieldStyle(.roundedBorder)
                        TextField("Value", text: parameterBinding(parameter.id, name: false)).textFieldStyle(.roundedBorder)
                        Toggle("=", isOn: Binding(get: { model.urlDraft?.parameters.first(where: { $0.id == parameter.id })?.hasValue ?? true }, set: { value in
                            if let i = model.urlDraft?.parameters.firstIndex(where: { $0.id == parameter.id }) { model.urlDraft?.parameters[i].hasValue = value }
                        })).help("Include an equals sign; turn off for a flag parameter")
                        Button { model.urlDraft?.parameters.removeAll(where: { $0.id == parameter.id }) } label: { Image(systemName: "minus.circle") }.buttonStyle(.plain).accessibilityLabel("Remove parameter")
                    }
                }
                HStack {
                    Button("Add Parameter") { model.urlDraft?.parameters.append(URLParameter(name: "", value: "")) }.buttonStyle(SecondaryButtonStyle())
                    Spacer()
                    Button("Build URL", action: model.buildURL).buttonStyle(PrimaryButtonStyle())
                }
            }
        }
    }
    private func parameterBinding(_ id: UUID, name: Bool) -> Binding<String> {
        Binding(get: {
            guard let value = model.urlDraft?.parameters.first(where: { $0.id == id }) else { return "" }
            return name ? value.name : value.value
        }, set: { value in
            guard let i = model.urlDraft?.parameters.firstIndex(where: { $0.id == id }) else { return }
            if name { model.urlDraft?.parameters[i].name = value } else { model.urlDraft?.parameters[i].value = value }
        })
    }
    private var requestFields: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Picker("Method", selection: $model.request.method) {
                    ForEach(Array(Set(["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", model.request.method])).sorted(), id: \.self) { Text($0) }
                }.frame(width: 140)
                TextField("https://example.com", text: $model.request.url).textFieldStyle(.roundedBorder).accessibilityLabel("Request URL")
                Button("Send", action: model.sendRequest).buttonStyle(PrimaryButtonStyle()).disabled(model.busy || model.request.url.isEmpty).keyboardShortcut(.return, modifiers: .command)
            }
            HStack(alignment: .top, spacing: 12) {
                VStack(alignment: .leading, spacing: 6) { Text("Headers · one per line").font(.caption).foregroundStyle(.secondary); editor("Request headers", text: $model.request.headers, height: 140) }
                VStack(alignment: .leading, spacing: 6) { Text("Body").font(.caption).foregroundStyle(.secondary); editor("Request body", text: $model.request.body, height: 140) }
            }
            Text("Redirects are shown for inspection. Requests and responses are not saved to history.").font(.caption).foregroundStyle(.secondary)
        }.disabled(model.sending)
    }
    private var snippetControls: some View {
        VStack(alignment: .leading, spacing: 10) {
            SnippetLibraryMenu(store: model.snippets, onSelect: model.loadSnippet)
            HStack {
                TextField("Snippet name", text: $model.snippetName).textFieldStyle(.roundedBorder)
                Button("New", action: model.newSnippet).buttonStyle(SecondaryButtonStyle())
                Button("Save Snippet", action: model.saveSnippet).buttonStyle(PrimaryButtonStyle()).disabled(model.snippetName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.input.isEmpty)
                if model.snippetID != nil { Button("Delete…") { showDeleteSnippet = true }.buttonStyle(SecondaryButtonStyle()) }
            }
            Text("Use {{name}} for a field, or {{selection}}, {{date}}, {{timestamp}}, and {{uuid}}. Fill fields below and copy or replace with the preview.").font(.caption).foregroundStyle(.secondary)
        }.alert("Delete this snippet?", isPresented: $showDeleteSnippet) {
            Button("Delete", role: .destructive, action: model.deleteSnippet)
            Button("Cancel", role: .cancel) {}
        }
    }
    private func tablePreview(_ table: DataTable) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Table preview · \(table.totalRows) rows · showing up to 30 rows and columns").font(.caption).foregroundStyle(.secondary)
            GeometryReader { geometry in
              ScrollView([.horizontal, .vertical]) {
                HStack(alignment: .top, spacing: 1) {
                    ForEach(Array(table.columns.enumerated()), id: \.offset) { column, name in
                        VStack(alignment: .leading, spacing: 0) {
                            Text(name).fontWeight(.semibold).padding(8).frame(width: 170, alignment: .leading).background(BoxTheme.accentSoft)
                            ForEach(Array(table.rows.enumerated()), id: \.offset) { index, row in
                                Text(String(row[column].prefix(200))).lineLimit(2).padding(8).frame(width: 170, height: 46, alignment: .leading).background(.primary.opacity(index % 2 == 0 ? 0.025 : 0.06))
                            }
                        }
                    }
                }.font(.system(size: 11, design: .monospaced)).textSelection(.enabled)
                    .frame(minWidth: geometry.size.width, minHeight: geometry.size.height, alignment: .topLeading)
              }
            }.frame(height: min(180, CGFloat(table.rows.count * 46 + 36)))
        }
    }
    private func highlighted(_ ranges: [NSRange]) -> AttributedString {
        var text = AttributedString(model.input)
        for range in ranges {
            if let swiftRange = Range(range, in: model.input), let start = AttributedString.Index(swiftRange.lowerBound, within: text), let end = AttributedString.Index(swiftRange.upperBound, within: text) {
                text[start..<end].backgroundColor = .orange.opacity(0.25)
            }
        }
        return text
    }
    private func diffColor(_ kind: ComparisonRow.Kind) -> Color { kind == .added ? .green : kind == .removed ? .red : .clear }
    private func wordDiff(_ result: ComparisonResult) -> AttributedString {
        var output = AttributedString()
        for row in result.rows {
            var word = AttributedString(row.text + " ")
            if row.kind != .same { word.backgroundColor = diffColor(row.kind).opacity(0.2) }
            if row.kind == .removed { word.strikethroughStyle = .single }
            output += word
        }
        return output
    }
    private var example: String {
        switch model.command {
        case .json, .convert: return model.command == .convert && model.fromFormat == .yaml ? "name: Bello Box\nactive: true\nitems:\n  - clock\n  - screenshot" : model.command == .convert && model.fromFormat == .csv ? "name,active\nBello Box,true\nExample,false" : "{\"name\":\"Bello Box\",\"id\":9007199254740993,\"tools\":[\"clock\",\"screenshot\"]}"
        case .jwt:
            func base64(_ text: String) -> String { Data(text.utf8).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "") }
            return base64("{\"alg\":\"HS256\",\"typ\":\"JWT\"}") + "." + base64("{\"sub\":\"example-user\",\"exp\":1893456000}") + ".example"
        case .regex: model.regexPattern = "[A-Z]+-\\d+"; return "Fixed BOX-123 and API-456. Next: UI-789."
        case .url: return "https://example.com/search?q=Bello%20Box&tag=swift&tag=macOS#results"
        case .time: return "2026-09-06T09:30:00Z"
        case .cron: return "*/15 9-17 * * MON-FRI"
        case .http: return "curl 'https://example.com' -H 'Accept: text/html'"
        default: return ""
        }
    }
}

private struct SnippetLibraryMenu: View {
    @ObservedObject var store: SnippetStore
    var onSelect: (DeveloperSnippet) -> Void
    @State private var query = ""
    var body: some View {
        HStack {
            TextField("Find a saved snippet…", text: $query).textFieldStyle(.roundedBorder)
            Menu("Saved Snippets (\(store.snippets.count))") {
                ForEach(store.snippets.filter { query.isEmpty || $0.name.localizedCaseInsensitiveContains(query) }) { snippet in
                    Button(snippet.name) { onSelect(snippet) }
                }
                if store.snippets.isEmpty { Text("Save your first snippet below") }
            }.fixedSize()
        }
    }
}
