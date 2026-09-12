import AppKit
import SwiftUI

/// Shared controls keep the palette and full workbench in sync. Mounting the
/// compact editor never takes the search field's first-responder status.
struct AdditionalUtilityEditor: View {
    @ObservedObject var model: UtilityWorkbenchModel
    let compact: Bool
    var onEscape: () -> Void
    private var kind: AdditionalUtilityKind { model.command.additionalTool! }
    private func option(_ id: String) -> Binding<String> {
        Binding(get: { model.utilityOptions[id] ?? kind.defaults[id] ?? "" }, set: { value in
            if kind == .bitwise && id == "operation" && (value.contains("shift") || value.contains("Rotate")) && !(model.utilityOptions[id]?.contains("shift") == true || model.utilityOptions[id]?.contains("Rotate") == true) { model.utilityOptions["operand"] = "1" }
            model.utilityOptions[id] = value
            if kind == .units && id == "from", let source = UnitTool.units.first(where: { $0.name == value }),
               let target = UnitTool.units.first(where: { $0.name == model.utilityOptions["to"] }), source.dimension != target.dimension {
                model.utilityOptions["to"] = UnitTool.units.first(where: { $0.dimension == source.dimension && $0.name != source.name })?.name ?? source.name
            }
        })
    }
    private var mainLabel: String {
        switch kind {
        case .jsonLines: return model.utilityOptions["mode"] == "Array → Lines" ? "JSON array · one output record per item" : kind.inputLabel
        case .envFile: return model.utilityOptions["mode"] == "JSON → Env" ? "JSON object · string values only" : kind.inputLabel
        case .plist: return model.utilityOptions["mode"] == "JSON → Plist" ? "Typed JSON · each node has type and value" : "XML property list · standard plist declaration is handled locally"
        case .cookies: return model.utilityOptions["mode"] == "Cookie" ? "Cookie request header · name=value; name=value" : "One Set-Cookie field per line"
        default: return kind.inputLabel
        }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: compact ? 6 : 12) {
            if !kind.options.isEmpty {
                HStack(spacing: 8) {
                    ForEach(kind.options) { definition in optionMenu(definition) }
                    if kind == .units {
                        Button {
                            let from = model.utilityOptions["from"] ?? "KiB", to = model.utilityOptions["to"] ?? "MiB"
                            model.utilityOptions["from"] = to; model.utilityOptions["to"] = from
                        } label: { Image(systemName: "arrow.left.arrow.right") }
                            .buttonStyle(ToolIconButtonStyle()).help("Swap units").accessibilityLabel("Swap units")
                    }
                    Spacer(minLength: 0)
                }
            }
            HStack(spacing: 6) {
                Text(kind.secondInputLabel == nil ? mainLabel : "Two inputs · edit either side").font(.system(size: compact ? 10 : 12, weight: .medium)).foregroundStyle(.secondary).lineLimit(1)
                Spacer(minLength: 0)
                Button("Example", action: model.loadUtilityExample).help("Load an example and reset the options")
                if kind.secondInputLabel == nil { Button("Paste") { model.pasteInput() } }
                Button("Clear") { model.input = ""; if kind.secondInputLabel != nil || kind == .hmac { model.secondInput = "" } }
            }.buttonStyle(LauncherChipButtonStyle())
            if let secondLabel = kind.secondInputLabel {
                HStack(spacing: 8) {
                    ForEach(0..<2) { side in
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(side == 0 ? mainLabel : secondLabel).font(.system(size: compact ? 10 : 12)).foregroundStyle(.secondary).lineLimit(1)
                                Spacer(minLength: 2)
                                Button("Paste") { model.pasteInput(second: side == 1) }.buttonStyle(LauncherChipButtonStyle()).accessibilityLabel(side == 0 ? "Paste first input" : "Paste second input")
                            }
                            textEditor(side == 0 ? $model.input : $model.secondInput, label: side == 0 ? mainLabel : secondLabel, secondary: side == 1)
                        }
                    }
                }
            } else if kind.multiline { textEditor($model.input, label: mainLabel) }
            else { field($model.input, label: mainLabel, placeholder: kind.example) }
            if kind == .bezier {
                HStack(spacing: 5) {
                    ForEach(["linear", "ease", "ease-in", "ease-out", "ease-in-out"], id: \.self) { preset in
                        Button(preset) { model.input = preset }.buttonStyle(LauncherChipButtonStyle()).help("Use the \(preset) curve")
                    }
                }
            }
            if !kind.fields.isEmpty {
                HStack(spacing: 8) {
                    ForEach(kind.fields) { definition in
                        VStack(alignment: .leading, spacing: compact ? 2 : 5) {
                            if compact {
                                HStack(spacing: 6) {
                                    Text(compactLabel(definition)).font(.system(size: 10)).foregroundStyle(.secondary).fixedSize()
                                    field(option(definition.id), label: definition.label, placeholder: definition.label)
                                }
                            } else {
                                Text(definition.label).font(.caption).foregroundStyle(.secondary)
                                field(option(definition.id), label: definition.label, placeholder: definition.label)
                            }
                        }.disabled(kind == .bitwise && definition.id == "operand" && model.utilityOptions["operation"] == "NOT")
                    }
                }
            }
            if kind == .hmac {
                HStack(spacing: 8) {
                    Image(systemName: "key.horizontal").foregroundStyle(BoxTheme.accent)
                    SecureField("Secret key · temporary, never saved", text: $model.secondInput)
                        .textFieldStyle(ToolTextFieldStyle()).font(.system(size: compact ? 11 : 13, design: .monospaced))
                        .accessibilityLabel("HMAC secret key").disableAutocorrection(true)
                    Button("Clear key") { model.secondInput = "" }.buttonStyle(LauncherChipButtonStyle()).disabled(model.secondInput.isEmpty)
                }
            }
            if kind == .chmod { permissionGrid }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("additionalUtilityEditor_" + kind.id)
    }
    private func optionMenu(_ definition: AdditionalUtilityKind.Option) -> some View {
        Menu {
            if kind == .units {
                ForEach(["Length", "Mass", "Temperature", "Data", "Duration", "Speed"], id: \.self) { dimension in
                    let units = UnitTool.units.filter { unit in
                        unit.dimension == dimension && (definition.id != "to" || UnitTool.units.first(where: { $0.name == model.utilityOptions["from"] })?.dimension == dimension)
                    }
                    if !units.isEmpty {
                        Section(dimension) { ForEach(units, id: \.name) { unit in choice(unit.name, id: definition.id) } }
                    }
                }
            } else { ForEach(definition.choices, id: \.self) { value in choice(value, id: definition.id) } }
        } label: {
            Text(definition.label + ": " + option(definition.id).wrappedValue).lineLimit(1)
        }.menuStyle(.borderlessButton).font(.system(size: compact ? 11 : 13))
            .controlSize(compact ? .small : .regular).fixedSize().padding(.horizontal, 8).frame(height: compact ? 26 : 32)
            .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(BoxTheme.separator))
            .accessibilityLabel(definition.label).accessibilityValue(option(definition.id).wrappedValue)
    }
    private func choice(_ value: String, id: String) -> some View {
        Button { option(id).wrappedValue = value } label: {
            if option(id).wrappedValue == value { Label(value, systemImage: "checkmark") } else { Text(value) }
        }
    }
    @ViewBuilder private func field(_ text: Binding<String>, label: String, placeholder: String) -> some View {
        if compact { LauncherPreviewField(text: text, placeholder: placeholder, label: label, onEscape: onEscape) }
        else {
            LauncherSearchField(text: text, onMove: { _ in }, onSubmit: {}, onEscape: onEscape, onReady: { _ in },
                                placeholder: placeholder, accessibilityID: "utilityField_" + label,
                                accessibilityLabel: label, fontSize: 13, focusesWhenAttached: label == mainLabel,
                                monospaced: true, consumesVerticalArrows: false)
                .frame(height: 22).padding(8).background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(BoxTheme.separator))
        }
    }
    private func compactLabel(_ field: AdditionalUtilityKind.Field) -> String {
        switch field.id { case "pointer": return "Pointer"; case "keys": return "Fields"; case "columns": return "Columns"; case "operand": return "B / shift"; default: return field.label }
    }
    private func textEditor(_ text: Binding<String>, label: String, secondary: Bool = false) -> some View {
        LiteralTextEditor(text: text, label: label, monospaced: true, focusesWhenAttached: !compact && !secondary, fontSize: compact ? 11 : 13)
            .frame(height: compact ? 46 : fullEditorHeight).padding(compact ? 4 : 8)
            .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: compact ? 7 : 10))
            .overlay(RoundedRectangle(cornerRadius: compact ? 7 : 10).strokeBorder(BoxTheme.separator))
    }
    private var fullEditorHeight: CGFloat {
        // Measure a bounded prefix, including wraps, so short inputs leave room
        // for results. Paired editors keep equal heights and retain native scroll.
        let columns = kind.secondInputLabel == nil ? 76 : 36
        func height(_ input: String) -> CGFloat {
            let sample = String(String.UnicodeScalarView(input.unicodeScalars.prefix(8_192)))
            let lines = DataWorkshop.normalizedLines(sample).reduce(0) { $0 + max(1, ($1.utf16.count + columns - 1) / columns) }
            return CGFloat(min(140, max(64, lines * 17 + 12)))
        }
        return kind.secondInputLabel == nil ? height(model.input) : max(height(model.input), height(model.secondInput))
    }
    private func permission(_ mask: Int) -> Binding<Bool> {
        Binding(get: { ((try? SecurityUtility.permissionBits(model.input)) ?? 0) & mask != 0 }, set: { enabled in
            let old = (try? SecurityUtility.permissionBits(model.input)) ?? 0
            model.input = String(format: "%03o", enabled ? old | mask : old & ~mask)
        })
    }
    private var permissionGrid: some View {
        VStack(spacing: 6) {
            HStack(spacing: 12) {
                ForEach(Array(["Owner", "Group", "Others"].enumerated()), id: \.offset) { group, title in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(title).font(.system(size: compact ? 10 : 12, weight: .semibold)).foregroundStyle(.secondary)
                        HStack(spacing: 8) {
                            ForEach(Array(["Read", "Write", "Exec"].enumerated()), id: \.offset) { bit, name in
                                Toggle(compact ? String(name.prefix(1)) : name, isOn: permission(1 << (8 - group * 3 - bit)))
                                    .toggleStyle(.checkbox).accessibilityLabel(title + " " + name)
                            }
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            HStack(spacing: 14) {
                Toggle("Set UID", isOn: permission(0o4000)).help("Run an executable with its owner's user ID")
                Toggle("Set GID", isOn: permission(0o2000)).help("Set group ID on execution; directories pass their group to new files")
                Toggle("Sticky", isOn: permission(0o1000)).help("On shared directories, restrict removal to the owner")
                Spacer(minLength: 0)
            }.toggleStyle(.checkbox)
        }.font(.system(size: compact ? 10 : 12)).controlSize(compact ? .mini : .small)
    }
}

struct AdditionalUtilityVisualView: View {
    let visual: UtilityVisual
    let compact: Bool
    var onBezierChange: (([Double]) -> Void)? = nil
    private func color(_ c: UtilityColor) -> Color { Color(.sRGB, red: c.red, green: c.green, blue: c.blue, opacity: c.alpha) }
    var body: some View {
        Group {
            switch visual {
            case .table(let table): UtilityTableVisual(table: table)
            case .bezier(let curve): BezierGraphView(curve: curve, onChange: onBezierChange)
            case .shadow(let spec): ShadowPreviewView(value: spec)
            case .statistics(let stats): StatisticsChartView(value: stats)
            case .bits(let bits, let signed):
                GeometryReader { _ in
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Signed result: " + signed).font(.system(size: 12, weight: .semibold, design: .monospaced)).textSelection(.enabled)
                        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 3), count: 16), spacing: 4) {
                            ForEach(Array(bits.enumerated()), id: \.offset) { i, bit in
                                Text(String(bit)).font(.system(size: 10, weight: .medium, design: .monospaced))
                                    .frame(maxWidth: .infinity).padding(.vertical, 3)
                                    .foregroundStyle(bit == "1" ? BoxTheme.accent : .secondary)
                                    .background(bit == "1" ? BoxTheme.accent.opacity(0.12) : BoxTheme.surface, in: RoundedRectangle(cornerRadius: 3)).help("Bit \(bits.count - 1 - i)")
                            }
                        }
                    }.padding(10)
                }.accessibilityLabel("Binary result \(bits), signed \(signed)")
            case .aspect(let w, let h, let targetW, let targetH):
                GeometryReader { proxy in
                    let ratio = Double(w) / Double(h), availableW = max(1, proxy.size.width * 0.4), availableH = max(1, proxy.size.height - 20)
                    let height = min(availableH, availableW / ratio), width = height * ratio
                    HStack(spacing: 20) {
                        RoundedRectangle(cornerRadius: 6).fill(BoxTheme.accent.opacity(0.1)).overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(BoxTheme.accent, lineWidth: 1.5))
                            .frame(width: max(1, width), height: max(1, height))
                        VStack(alignment: .leading, spacing: 5) {
                            Text("\(targetW) × \(targetH)").font(.system(size: compact ? 19 : 26, weight: .semibold, design: .rounded))
                            Text("Target pixels · proportional to \(w) × \(h)").font(.system(size: 10)).foregroundStyle(.secondary)
                        }.textSelection(.enabled)
                    }.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
                }.padding(10)
            case .color(let c):
                HStack(spacing: 14) {
                    RoundedRectangle(cornerRadius: 10).fill(color(c)).frame(width: compact ? 68 : 110)
                        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(BoxTheme.separator))
                    VStack(alignment: .leading, spacing: 5) {
                        Text(c.hex).font(.system(size: compact ? 17 : 23, weight: .semibold, design: .monospaced))
                        Text(c.rgb + "\n" + c.hsl).font(.system(size: compact ? 10 : 12, design: .monospaced)).foregroundStyle(.secondary)
                    }.textSelection(.enabled)
                    Spacer(minLength: 0)
                }.padding(10)
            case .contrast(let foreground, let background, let ratio):
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 5) {
                        Text("The quick brown fox").font(.system(size: compact ? 16 : 24, weight: .semibold))
                        Text("Readable details matter.").font(.system(size: compact ? 11 : 14))
                    }.foregroundStyle(color(foreground)).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                        .padding(12).background(color(background), in: RoundedRectangle(cornerRadius: 8))
                    VStack(alignment: .leading, spacing: 4) {
                        Text(String(format: "%.2f:1", ratio)).font(.system(size: compact ? 18 : 25, weight: .semibold, design: .rounded))
                        Text("AA normal: \(ratio >= 4.5 ? "Pass" : "Fail")\nAA large: \(ratio >= 3 ? "Pass" : "Fail")\nAAA normal: \(ratio >= 7 ? "Pass" : "Fail")")
                            .font(.system(size: compact ? 10 : 12)).foregroundStyle(.secondary)
                    }.frame(width: compact ? 118 : 170, alignment: .leading)
                }.padding(8)
            case .gradient(let a, let b, let angle):
                let radians = angle * .pi / 180, dx = sin(radians), dy = -cos(radians)
                // CSS's gradient line spans the projected rectangle, not the diagonal.
                GeometryReader { proxy in
                    let w = max(1, proxy.size.width), h = max(1, proxy.size.height), length = abs(w * dx) + abs(h * dy)
                    LinearGradient(colors: [color(a), color(b)], startPoint: UnitPoint(x: 0.5 - dx * length / (2 * w), y: 0.5 - dy * length / (2 * h)), endPoint: UnitPoint(x: 0.5 + dx * length / (2 * w), y: 0.5 + dy * length / (2 * h)))
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                        .overlay(alignment: .bottomLeading) {
                            Text("\(a.hex) → \(b.hex) · \(MathTool.display(angle))°")
                                .font(.system(size: 10, weight: .medium, design: .monospaced)).foregroundStyle(.white)
                                .padding(6).background(.black.opacity(0.7), in: RoundedRectangle(cornerRadius: 5)).padding(8)
                        }
                }.padding(6)
            case .markdown(let blocks):
                GeometryReader { _ in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: compact ? 8 : 12) {
                        ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in markdownBlock(block) }
                    }.frame(maxWidth: .infinity, alignment: .leading).padding(compact ? 10 : 16)
                }.environment(\.openURL, OpenURLAction { url in
                    guard ["https", "http", "mailto"].contains(url.scheme?.lowercased() ?? "") else { return .discarded }
                    return .systemAction
                })
                }
            case .permissions(let bits):
                HStack {
                    Image(systemName: "lock.shield").foregroundStyle(BoxTheme.accent).font(.system(size: 25))
                    VStack(alignment: .leading, spacing: 4) {
                        Text(String(format: "%03o", bits) + "  " + SecurityUtility.symbolic(bits)).font(.system(size: compact ? 18 : 25, weight: .medium, design: .monospaced))
                        Text("Changes apply to this preview only. Copy the command when ready.").font(.system(size: compact ? 10 : 12)).foregroundStyle(.secondary)
                    }.textSelection(.enabled)
                    Spacer(minLength: 0)
                }.padding(10)
            }
        }.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(BoxTheme.separator))
    }
    private func inline(_ text: String) -> Text {
        Text((try? AttributedString(markdown: text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(text))
    }
    @ViewBuilder private func markdownBlock(_ block: MarkdownBlock) -> some View {
        switch block.kind {
        case .heading(let level): inline(block.text).font(.system(size: CGFloat((compact ? 22 : 28) - (level - 1) * 2), weight: .semibold))
        case .paragraph: inline(block.text).font(.system(size: compact ? 12 : 14)).textSelection(.enabled)
        case .bullet: HStack(alignment: .top, spacing: 8) { Text("•").foregroundStyle(BoxTheme.accent); inline(block.text) }.font(.system(size: compact ? 12 : 14))
        case .quote: HStack { Rectangle().fill(BoxTheme.accent).frame(width: 3); inline(block.text).foregroundStyle(.secondary) }.fixedSize(horizontal: false, vertical: true)
        case .code: Text(block.text).font(.system(size: compact ? 11 : 12, design: .monospaced)).textSelection(.enabled).padding(8).frame(maxWidth: .infinity, alignment: .leading).background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 6))
        case .rule: Divider()
        }
    }
}
