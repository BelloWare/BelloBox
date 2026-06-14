import AppKit
import Combine

/// Drives one popup session: holds the captured selection, runs AI actions, and
/// streams the result for display.
@MainActor
final class ActionPopupViewModel: ObservableObject {
    @Published var instruction: String = ""
    @Published var resultText: String = ""
    @Published var isStreaming: Bool = false
    @Published var errorMessage: String?
    @Published private(set) var lastActionReplaces: Bool = true
    @Published private(set) var didRun: Bool = false

    let selection: TextSelection
    let quickActions: [QuickAction] = QuickAction.library

    private let settings: AppSettings
    private let client: AIClient
    private let accessibility: AccessibilityService
    private var task: Task<Void, Never>?

    /// Invoked when the popup wants to dismiss itself.
    var onClose: () -> Void = {}
    /// Invoked to open the settings window.
    var onOpenSettings: () -> Void = {}

    init(
        selection: TextSelection,
        settings: AppSettings,
        client: AIClient,
        accessibility: AccessibilityService
    ) {
        self.selection = selection
        self.settings = settings
        self.client = client
        self.accessibility = accessibility
    }

    var isConfigured: Bool { settings.isConfigured }
    var providerSummary: String {
        let config = settings.currentConfig
        return "\(settings.providerKind.displayName) · \(config.model)"
    }
    var canReplace: Bool { lastActionReplaces && !resultText.isEmpty && !isStreaming }
    var canCopy: Bool { !resultText.isEmpty && !isStreaming }

    func run(_ action: QuickAction) {
        runInstruction(action.instruction, replaces: action.replacesSelection)
    }

    func runCustom() {
        let trimmed = instruction.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        runInstruction(trimmed, replaces: true)
    }

    private func runInstruction(_ instruction: String, replaces: Bool) {
        task?.cancel()
        guard settings.isConfigured else {
            errorMessage = "Add an API key for your provider in BelloBox settings first."
            return
        }
        resultText = ""
        errorMessage = nil
        isStreaming = true
        didRun = true
        lastActionReplaces = replaces

        let config = settings.currentConfig
        let payload = QuickAction.userMessage(instruction: instruction, selectedText: selection.text)

        task = Task { [weak self] in
            guard let self else { return }
            do {
                try await self.client.stream(config: config, userText: payload) { delta in
                    Task { @MainActor [weak self] in
                        self?.resultText += delta
                    }
                }
            } catch {
                await MainActor.run { [weak self] in
                    self?.errorMessage = (error as? AIError)?.errorDescription ?? error.localizedDescription
                }
            }
            await MainActor.run { [weak self] in
                self?.isStreaming = false
            }
        }
    }

    func copyResult() {
        guard !resultText.isEmpty else { return }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(resultText, forType: .string)
    }

    func replaceSelection() {
        guard !resultText.isEmpty else { return }
        let pid = selection.pid
        let text = resultText
        onClose()
        // Paste after the popup has dismissed so focus returns to the target app.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [accessibility] in
            accessibility.replaceSelection(with: text, pid: pid)
        }
    }

    func openSettings() {
        onOpenSettings()
    }

    func cancel() {
        task?.cancel()
        isStreaming = false
    }

    func close() {
        task?.cancel()
        onClose()
    }
}
