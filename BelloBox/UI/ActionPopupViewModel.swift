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
    private var runToken: UUID?

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
    var copyableText: String {
        if !resultText.isEmpty { return resultText }
        return errorMessage ?? ""
    }
    var canCopy: Bool { !copyableText.isEmpty && !isStreaming }

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
            runToken = nil
            task = nil
            resultText = ""
            isStreaming = false
            didRun = true
            lastActionReplaces = replaces
            errorMessage = "Add an API key for your provider in Bello Box settings first."
            return
        }
        let token = UUID()
        runToken = token
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
                for try await delta in self.deltaStream(config: config, userText: payload) {
                    try Task.checkCancellation()
                    guard self.runToken == token else { return }
                    self.resultText += delta
                }
            } catch is CancellationError {
            } catch {
                guard self.runToken == token else { return }
                self.errorMessage = (error as? AIError)?.errorDescription ?? error.localizedDescription
            }
            if self.runToken == token {
                self.runToken = nil
                self.task = nil
                self.isStreaming = false
            }
        }
    }

    private func deltaStream(config: AIConfig, userText: String) -> AsyncThrowingStream<String, Error> {
        let client = client
        return AsyncThrowingStream { continuation in
            let producer = Task {
                do {
                    try await client.stream(config: config, userText: userText) { delta in
                        guard !delta.isEmpty else { return }
                        continuation.yield(delta)
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { @Sendable _ in
                producer.cancel()
            }
        }
    }

    func copyResult() {
        let text = copyableText
        guard !text.isEmpty else { return }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
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
        runToken = nil
        task?.cancel()
        task = nil
        isStreaming = false
    }

    func close() {
        runToken = nil
        task?.cancel()
        task = nil
        isStreaming = false
        onClose()
    }
}
