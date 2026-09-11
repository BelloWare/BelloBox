import SwiftUI

/// Used by both Settings and Setup. Editing options only saves a local profile;
/// the explicit connection test is the first point at which it sends a request.
struct AIGenerationSettingsView: View {
    @ObservedObject var settings: AppSettings
    @State private var showsThinking = false

    private var options: AIGenerationPreferences { settings.generationPreferences }
    private var isAnthropic: Bool { settings.providerKind == .anthropic }
    private var temperatureIsInactive: Bool { isAnthropic && options.thinkingMode.isEnabled }
    private var hasOverrides: Bool { options != AIGenerationPreferences() }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Image(systemName: "slider.horizontal.3").foregroundStyle(BoxTheme.accent)
                Text("Model behavior").font(.system(size: 13, weight: .semibold))
                Spacer(minLength: 8)
                if hasOverrides {
                    Button("Reset") { settings.resetModelGenerationPreferences() }
                        .buttonStyle(ToolLinkButtonStyle())
                        .help("Restore the defaults for this model only")
                        .accessibilityLabel("Reset this model’s behavior")
                }
            }
            VStack(alignment: .leading, spacing: 3) {
                Text(settings.generationModelName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                     ? "Choose a model above" : settings.generationModelName)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(BoxTheme.accent).lineLimit(2).textSelection(.enabled)
                helpText(settings.providerKind.isHTTP
                         ? "Saved separately for this model and endpoint. Switching back restores your choices."
                         : "Saved separately for this model and Codex command. Switching back restores your choices.")
            }
            Divider().overlay(BoxTheme.separator)

            if settings.providerKind.isHTTP { temperatureControls }
            reasoningControls

            if isAnthropic {
                Divider().overlay(BoxTheme.separator)
                DisclosureGroup(isExpanded: $showsThinking) {
                    thinkingControls.padding(.top, 10)
                } label: {
                    Text("Thinking & token limits").font(.system(size: 12, weight: .medium))
                }
                .tint(BoxTheme.accent)
            }
        }
        .padding(14)
        .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(BoxTheme.separator, lineWidth: 1))
        .disabled(settings.generationModelName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        .onAppear { updateDisclosure() }
        .onChange(of: settings.generationModelName) { _ in updateDisclosure() }
        .onChange(of: settings.providerKind) { _ in updateDisclosure() }
    }

    private var temperatureControls: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack(spacing: 12) {
                Text("Temperature").font(.system(size: 12, weight: .medium))
                Spacer(minLength: 8)
                if temperatureIsInactive {
                    Label("Not sent", systemImage: "minus.circle")
                        .font(.system(size: 11, weight: .medium)).foregroundStyle(BoxTheme.accent)
                        .padding(.horizontal, 10).padding(.vertical, 6)
                        .background(BoxTheme.accentSoft, in: Capsule())
                } else {
                    ToolChoiceBar(selection: $settings.temperatureMode,
                                  choices: [(.providerDefault, "Model default"), (.custom, "Custom")],
                                  label: "Temperature")
                        .frame(width: 218)
                }
            }
            if options.temperatureMode == .custom && !temperatureIsInactive {
                HStack(spacing: 12) {
                    Slider(value: $settings.temperature, in: 0 ... (isAnthropic ? 1 : 2), step: 0.1)
                        .accessibilityLabel("Custom temperature")
                    Stepper(value: $settings.temperature, in: 0 ... (isAnthropic ? 1 : 2), step: 0.1) {
                        Text(settings.temperature, format: .number.precision(.fractionLength(1)))
                            .monospacedDigit().frame(width: 30, alignment: .trailing)
                    }.fixedSize().accessibilityLabel("Custom temperature")
                }.font(.system(size: 12))
            }
            helpText(temperatureIsInactive
                     ? "Not sent while thinking is enabled. Your temperature choice is kept for when you turn thinking off."
                     : options.temperatureMode == .providerDefault
                     ? "No temperature is sent. Use this for models that don’t support temperature."
                     : "Lower values favor consistency; higher values add variety. Switch to Model default if your model rejects this setting.")
        }
    }

    private var reasoningControls: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack(spacing: 12) {
                Text("Reasoning effort").font(.system(size: 12, weight: .medium))
                Spacer(minLength: 8)
                Picker("Reasoning effort", selection: effortBinding) {
                    ForEach(AIReasoningEffort.choices(for: settings.providerKind)) { effort in
                        Text(effort.label).tag(effort)
                    }
                }.labelsHidden().pickerStyle(.menu).frame(width: 168)
            }
            helpText(effortBinding.wrappedValue == .providerDefault
                     ? "No effort is sent. Choose a level only if your model supports it."
                     : "Higher effort can improve complex answers, but takes longer and may use more tokens. Supported levels vary by model.")
        }
    }

    private var thinkingControls: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Thinking").font(.system(size: 12, weight: .medium))
                Spacer()
                Picker("Thinking mode", selection: optionBinding(\.thinkingMode)) {
                    ForEach(AnthropicThinkingMode.allCases) { mode in Text(mode.label).tag(mode) }
                }.labelsHidden().pickerStyle(.menu).frame(width: 168)
            }
            helpText(thinkingHelp)
            if options.thinkingMode == .budgeted {
                tokenStepper("Thinking budget", value: optionBinding(\.thinkingBudget), range: 1024 ... 32768)
            }
            tokenStepper("Output token limit", value: Binding(
                get: { options.anthropicOutputTokenLimit },
                set: { settings.generationPreferences.outputTokenLimit = $0 }
            ), range: minimumOutputLimit ... 65536)
            helpText(options.thinkingMode == .budgeted
                     ? "Includes thinking and the answer, with at least 2,048 tokens left for the answer. Larger budgets can take longer and cost more."
                     : options.thinkingMode == .adaptive
                     ? "Includes thinking and the answer. Adaptive thinking uses a limit of at least 8,192 tokens; larger limits can take longer and cost more."
                     : "The maximum length of thinking and answer combined. Raise this if a reasoning model stops before answering.")
        }
    }

    private var effortBinding: Binding<AIReasoningEffort> {
        Binding(get: {
            settings.providerKind == .codexCLI
                ? AIReasoningEffort(rawValue: settings.codexReasoningEffort) ?? .medium : settings.reasoningEffort
        }, set: { effort in
            if settings.providerKind == .codexCLI { settings.codexReasoningEffort = effort.rawValue }
            else { settings.reasoningEffort = effort }
        })
    }

    private var minimumOutputLimit: Int {
        switch options.thinkingMode {
        case .adaptive: return 8192
        case .budgeted: return options.thinkingBudget + 2048
        case .providerDefault, .disabled: return 1024
        }
    }

    private var thinkingHelp: String {
        switch options.thinkingMode {
        case .providerDefault: return "Let the model decide. No thinking override is sent."
        case .disabled: return "Requests thinking off. Some models require thinking and cannot disable it."
        case .adaptive: return "The model decides how much to think. Choose this only for models that support adaptive thinking."
        case .budgeted: return "Set a thinking budget for models with manual extended thinking. Newer models may require Adaptive instead."
        }
    }

    private func optionBinding<T>(_ keyPath: WritableKeyPath<AIGenerationPreferences, T>) -> Binding<T> {
        Binding(get: { settings.generationPreferences[keyPath: keyPath] },
                set: { settings.generationPreferences[keyPath: keyPath] = $0 })
    }

    private func tokenStepper(_ title: String, value: Binding<Int>, range: ClosedRange<Int>) -> some View {
        HStack {
            Text(title).font(.system(size: 12, weight: .medium))
            Spacer()
            Stepper(value: value, in: range, step: 1024) {
                Text(value.wrappedValue, format: .number).monospacedDigit()
            }.fixedSize().accessibilityLabel(title)
        }
    }

    private func helpText(_ text: String) -> some View {
        Text(text).font(.system(size: 11)).foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func updateDisclosure() {
        showsThinking = options.thinkingMode != .providerDefault || options.outputTokenLimit != 2048
    }
}
