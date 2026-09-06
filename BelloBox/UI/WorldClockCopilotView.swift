import SwiftUI

/// The World Clock copilot: an ephemeral transcript, explicit Send, and
/// suggestions that only take effect when the user applies them. The same view
/// serves the dedicated window (`.planner`) and the palette preview (`.compact`).
struct WorldClockCopilotView: View {
    enum Style { case planner, compact }

    @ObservedObject var session: WorldClockCopilotSession
    var style: Style
    /// What applying a message would change, or nil when nothing would.
    var plan: (WorldClockCopilotSession.Message) -> WorldClockCopilotPlan?
    /// Parts of a suggestion this host cannot apply but the dedicated window
    /// can; the palette shows them as "press Enter to apply" instead of
    /// pretending they are already in effect.
    var deferredPlan: (WorldClockCopilotSession.Message) -> WorldClockCopilotPlan? = { _ in nil }
    var onApply: (WorldClockCopilotPlan, WorldClockCopilotSession.Message) -> Void
    var onOpenSettings: () -> Void
    var onEscape: () -> Void = {}
    var onFieldReady: (LauncherSearchTextField) -> Void = { _ in }
    var quickPrompts: [String] = []
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    static let compactTranscriptHeight: CGFloat = 98
    private var isCompact: Bool { style == .compact }

    var body: some View {
        VStack(alignment: .leading, spacing: isCompact ? 6 : 8) {
            if !isCompact { header }
            if session.isTranscriptVisible {
                transcript
            } else if !quickPrompts.isEmpty, session.canUseAI {
                prompts
            }
            inputRow
            if !session.canUseAI { providerNotice }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("World Clock copilot")
    }

    private var header: some View {
        HStack(spacing: 8) {
            Image(systemName: "sparkles").foregroundStyle(BoxTheme.purple)
            Text("Copilot").font(.caption.weight(.semibold))
            Text("Knows the selected time and your locations").font(.caption).foregroundStyle(.secondary)
            Spacer()
            if session.hasTranscript {
                Button("Clear", action: session.clear).buttonStyle(.link).font(.caption)
                    .help("Forget this conversation")
            }
        }
    }

    private var prompts: some View {
        HStack(spacing: 6) {
            ForEach(quickPrompts, id: \.self) { prompt in
                Button(prompt) { session.ask(prompt) }
                    .buttonStyle(.plain).font(.system(size: 10, weight: .medium))
                    .padding(.horizontal, 8).padding(.vertical, 4)
                    .background(BoxTheme.well, in: Capsule())
                    .overlay(Capsule().strokeBorder(BoxTheme.border))
                    .help("Ask: \(prompt)")
            }
        }
    }

    private var transcript: some View {
        ScrollViewReader { reader in
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(session.messages) { message in
                        bubble(message).id(message.id)
                    }
                    statusRow.id("status")
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(isCompact ? 6 : 8)
            }
            .frame(height: isCompact ? Self.compactTranscriptHeight : 220)
            .background(BoxTheme.well, in: RoundedRectangle(cornerRadius: 8))
            .accessibilityIdentifier("worldClockCopilotTranscript")
            .onChange(of: session.messages.count) { _ in
                withAnimation(reduceMotion ? nil : .easeOut(duration: 0.12)) { reader.scrollTo("status", anchor: .bottom) }
            }
            .onChange(of: session.isBusy) { _ in reader.scrollTo("status", anchor: .bottom) }
        }
    }

    @ViewBuilder private func bubble(_ message: WorldClockCopilotSession.Message) -> some View {
        switch message.role {
        case .user:
            HStack {
                Spacer(minLength: 40)
                Text(message.text).font(.system(size: isCompact ? 11 : 12)).textSelection(.enabled)
                    .padding(.horizontal, 9).padding(.vertical, 5)
                    .background(BoxTheme.accentSoft, in: RoundedRectangle(cornerRadius: 8))
            }
        case .assistant:
            HStack(alignment: .top, spacing: 6) {
                Image(systemName: "sparkles").font(.system(size: 10)).foregroundStyle(BoxTheme.purple).padding(.top, 3)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 5) {
                    Text(message.text).font(.system(size: isCompact ? 11 : 12)).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    if let issue = message.issue {
                        Label(issue, systemImage: "exclamationmark.triangle").font(.system(size: 10))
                            .foregroundStyle(BoxTheme.warning)
                    }
                    if message.suggestion != nil { suggestionRow(message) }
                }
                .padding(.horizontal, 9).padding(.vertical, 6)
                .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(BoxTheme.border))
            }
        }
    }

    /// Remaining and deferred parts always win over completion state, so a
    /// partially applied suggestion keeps offering what is left.
    @ViewBuilder private func suggestionRow(_ message: WorldClockCopilotSession.Message) -> some View {
        let applied = session.appliedParts(for: message)
        let applicable = plan(message)
        let deferred = deferredPlan(message)
        VStack(alignment: .leading, spacing: 5) {
            if applicable == nil, deferred == nil {
                if applied.isEmpty {
                    Text("Already in effect.").font(.system(size: 10)).foregroundStyle(.secondary)
                } else {
                    Label("Applied", systemImage: "checkmark.circle.fill").font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(BoxTheme.success)
                        .accessibilityIdentifier("worldClockCopilotApplied")
                }
            } else {
                if !applied.isEmpty {
                    Label(applied == .time ? "Time applied" : applied == .locations ? "Locations applied" : "Partly applied",
                          systemImage: "checkmark.circle")
                        .font(.system(size: 10, weight: .semibold)).foregroundStyle(BoxTheme.success)
                        .accessibilityIdentifier("worldClockCopilotPartlyApplied")
                }
                if let applicable {
                    HStack(spacing: 8) {
                        Button { onApply(applicable, message) } label: {
                            Label("Apply", systemImage: "arrow.right.circle.fill")
                        }
                        .buttonStyle(PrimaryButtonStyle())
                        .accessibilityIdentifier("worldClockCopilotApply")
                        .help("Apply this suggestion. Nothing changes until you do.")
                        Text(applicable.summary).font(.system(size: 10)).foregroundStyle(.secondary).lineLimit(2)
                    }
                }
                if let deferred {
                    Label("Press ↵ to open World Clock and apply: \(deferred.summary)", systemImage: "arrow.right.square")
                        .font(.system(size: 10)).foregroundStyle(BoxTheme.accent).lineLimit(2)
                        .accessibilityIdentifier("worldClockCopilotDeferred")
                }
            }
        }
    }

    @ViewBuilder private var statusRow: some View {
        if session.isBusy {
            HStack(spacing: 6) {
                ProgressView().controlSize(.small)
                Text("Thinking…").font(.system(size: 10)).foregroundStyle(.secondary)
                Button("Cancel", action: session.cancel).buttonStyle(.link).font(.system(size: 10))
                    .accessibilityIdentifier("worldClockCopilotCancel")
            }
        } else if let error = session.errorMessage {
            HStack(alignment: .top, spacing: 6) {
                Image(systemName: "exclamationmark.circle").foregroundStyle(BoxTheme.danger)
                Text(error).font(.system(size: 10)).foregroundStyle(BoxTheme.danger).textSelection(.enabled)
                if session.canRetry {
                    Button("Retry", action: session.retry).buttonStyle(.link).font(.system(size: 10))
                        .accessibilityIdentifier("worldClockCopilotRetry")
                }
            }
        } else if let status = session.statusMessage {
            HStack(spacing: 6) {
                Text(status).font(.system(size: 10)).foregroundStyle(.secondary)
                if session.canRetry {
                    Button("Ask again", action: session.retry).buttonStyle(.link).font(.system(size: 10))
                        .accessibilityIdentifier("worldClockCopilotRetry")
                }
            }
        }
    }

    private var inputRow: some View {
        HStack(spacing: 8) {
            Image(systemName: "sparkles").font(.system(size: 11)).foregroundStyle(BoxTheme.purple)
                .accessibilityHidden(true)
            LauncherSearchField(text: $session.draft, onMove: { _ in }, onSubmit: session.send, onEscape: onEscape,
                onReady: onFieldReady,
                placeholder: session.canUseAI ? "Ask the copilot, e.g. best time for everyone today…"
                                              : "Connect an AI provider to ask the copilot",
                accessibilityID: "worldClockCopilotQuestion", accessibilityLabel: "Ask the World Clock copilot",
                fontSize: isCompact ? 12 : 13, focusesWhenAttached: !isCompact)
                .frame(height: isCompact ? 20 : 22)
                .disabled(!session.canUseAI)
            if session.isBusy {
                Button("Cancel", action: session.cancel).buttonStyle(SecondaryButtonStyle())
            } else {
                Button { session.send() } label: { Label("Send", systemImage: "arrow.up") }
                    .buttonStyle(PrimaryButtonStyle()).disabled(!session.canSend)
                    .accessibilityIdentifier("worldClockCopilotSend")
                    .help("Send the question to your configured AI provider")
            }
        }
        .padding(.horizontal, 8).padding(.vertical, isCompact ? 4 : 5)
        .background(BoxTheme.surface, in: RoundedRectangle(cornerRadius: 9))
        .overlay(RoundedRectangle(cornerRadius: 9).strokeBorder(BoxTheme.border))
    }

    private var providerNotice: some View {
        HStack(spacing: 5) {
            Text("The copilot needs an AI provider.").foregroundStyle(.secondary)
            Button("Open Settings", action: onOpenSettings).buttonStyle(.link)
                .accessibilityIdentifier("worldClockCopilotOpenSettings")
        }.font(.system(size: 10))
    }
}
