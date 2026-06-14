import Foundation

/// Minimal JSON-RPC client for `codex app-server --stdio`.
///
/// Bello Box uses one short-lived app-server session per AI action. The app
/// passes model, reasoning effort, approval, and sandbox parameters on the
/// requests instead of creating or modifying any Codex config files.
final class CodexAppServerClient {
    func stream(config: AIConfig, userText: String, onDelta: @escaping (String) -> Void) async throws {
        let runner = Runner(config: config, userText: userText, onDelta: onDelta)
        try await runner.run()
    }

    static func resolvedModel(_ config: AIConfig) -> String {
        let model = config.model.trimmingCharacters(in: .whitespacesAndNewlines)
        return model.isEmpty ? CodexCLI.defaultModel : model
    }

    static func resolvedReasoningEffort(_ config: AIConfig) -> String {
        let effort = config.codexReasoningEffort.trimmingCharacters(in: .whitespacesAndNewlines)
        return effort.isEmpty ? CodexCLI.defaultReasoningEffort : effort
    }

    static func developerInstructions(system: String) -> String {
        let directive = "Output only the resulting text itself. Do not explain, do not comment, do not ask questions, and do not use code fences."
        let trimmed = system.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? directive : "\(trimmed)\n\n\(directive)"
    }

    static func appServerCommand(_ pathOrCommand: String) -> String {
        "\(AIClient.codexInvocation(pathOrCommand)) app-server --stdio"
    }

    static func initializeParams() -> [String: Any] {
        [
            "clientInfo": [
                "name": "bellobox",
                "title": "Bello Box",
                "version": Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown",
            ],
            "capabilities": [
                "experimentalApi": true,
                "requestAttestation": false,
            ],
        ]
    }

    static func threadStartParams(config: AIConfig, cwd: String) -> [String: Any] {
        [
            "model": resolvedModel(config),
            "cwd": cwd,
            "approvalPolicy": "never",
            "sandbox": "read-only",
            "ephemeral": true,
            "serviceName": "Bello Box",
            "developerInstructions": developerInstructions(system: config.systemPrompt),
        ]
    }

    static func turnStartParams(threadId: String, config: AIConfig, userText: String, cwd: String) -> [String: Any] {
        [
            "threadId": threadId,
            "input": [
                [
                    "type": "text",
                    "text": userText,
                    "text_elements": [],
                ],
            ],
            "cwd": cwd,
            "approvalPolicy": "never",
            "sandboxPolicy": [
                "type": "readOnly",
                "networkAccess": true,
            ],
            "model": resolvedModel(config),
            "effort": resolvedReasoningEffort(config),
        ]
    }

    static func jsonObject(from line: String) -> [String: Any]? {
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix("{"), let data = trimmed.data(using: .utf8) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }

    static func requestID(from value: Any?) -> Int? {
        if let int = value as? Int { return int }
        if let number = value as? NSNumber { return number.intValue }
        return nil
    }

    static func jsonRPCErrorMessage(_ message: [String: Any]) -> String? {
        guard let error = message["error"] as? [String: Any] else { return nil }
        if let text = error["message"] as? String, !text.isEmpty { return text }
        return "Codex app-server returned an error."
    }

    static func threadID(from response: [String: Any]) -> String? {
        guard
            let result = response["result"] as? [String: Any],
            let thread = result["thread"] as? [String: Any],
            let id = thread["id"] as? String
        else { return nil }
        return id
    }

    static func agentMessageDelta(from notification: [String: Any]) -> String? {
        guard
            notification["method"] as? String == "item/agentMessage/delta",
            let params = notification["params"] as? [String: Any],
            let delta = params["delta"] as? String
        else { return nil }
        return delta
    }

    static func completedAgentMessage(from notification: [String: Any]) -> String? {
        guard
            notification["method"] as? String == "item/completed",
            let params = notification["params"] as? [String: Any],
            let item = params["item"] as? [String: Any],
            item["type"] as? String == "agentMessage",
            let text = item["text"] as? String
        else { return nil }
        return text
    }

    static func turnCompletion(from notification: [String: Any]) -> (status: String, error: String?)? {
        guard
            notification["method"] as? String == "turn/completed",
            let params = notification["params"] as? [String: Any],
            let turn = params["turn"] as? [String: Any],
            let status = turn["status"] as? String
        else { return nil }
        let error = (turn["error"] as? [String: Any])?["message"] as? String
        return (status, error)
    }
}

private extension CodexAppServerClient {
    final class Runner {
        private let config: AIConfig
        private let userText: String
        private let onDelta: (String) -> Void
        private let process = Process()
        private let stdinPipe = Pipe()
        private let stdoutPipe = Pipe()
        private let stderrPipe = Pipe()
        private let queue = DispatchQueue(label: "com.ainoob.BelloBox.codex-app-server")
        private let cwd = NSTemporaryDirectory()

        private var continuation: CheckedContinuation<Void, Error>?
        private var stdoutBuffer = ""
        private var stderrBuffer = ""
        private var nextRequestID = 1
        private var pending: [Int: String] = [:]
        private var threadID: String?
        private var emittedText = ""
        private var completedText: String?
        private var finished = false

        init(config: AIConfig, userText: String, onDelta: @escaping (String) -> Void) {
            self.config = config
            self.userText = userText
            self.onDelta = onDelta
        }

        func run() async throws {
            try await withTaskCancellationHandler {
                try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
                    queue.async {
                        self.continuation = continuation
                        do {
                            try self.startLocked()
                            try self.sendLocked(method: "initialize", params: CodexAppServerClient.initializeParams())
                        } catch {
                            self.finishLocked(.failure(error))
                        }
                    }
                }
            } onCancel: {
                queue.async {
                    self.finishLocked(.failure(CancellationError()))
                }
            }
        }

        private func startLocked() throws {
            let shell = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"
            process.executableURL = URL(fileURLWithPath: shell)
            process.arguments = ["-l", "-i", "-c", CodexAppServerClient.appServerCommand(config.baseURL)]
            process.currentDirectoryURL = URL(fileURLWithPath: cwd)
            process.environment = ProcessInfo.processInfo.environment
            process.standardInput = stdinPipe
            process.standardOutput = stdoutPipe
            process.standardError = stderrPipe

            stdoutPipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
                let data = handle.availableData
                guard !data.isEmpty else { return }
                self?.queue.async {
                    self?.handleStdoutLocked(data)
                }
            }
            stderrPipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
                let data = handle.availableData
                guard !data.isEmpty else { return }
                self?.queue.async {
                    self?.appendStderrLocked(data)
                }
            }
            process.terminationHandler = { [weak self] process in
                self?.queue.async {
                    guard let self, !self.finished else { return }
                    let details = self.stderrTailLocked()
                    let message = details.isEmpty
                        ? "Codex app-server exited with status \(process.terminationStatus). Is the Codex CLI installed and logged in?"
                        : String(details.suffix(900))
                    self.finishLocked(.failure(AIError.transport(message)))
                }
            }

            do {
                try process.run()
            } catch {
                throw AIError.transport("Couldn't launch Codex app-server: \(error.localizedDescription)")
            }
        }

        private func sendLocked(method: String, params: [String: Any]) throws {
            let id = nextRequestID
            nextRequestID += 1
            pending[id] = method
            let message: [String: Any] = [
                "method": method,
                "id": id,
                "params": params,
            ]
            var data = try JSONSerialization.data(withJSONObject: message, options: [])
            data.append(contentsOf: [0x0A])
            stdinPipe.fileHandleForWriting.write(data)
        }

        private func handleStdoutLocked(_ data: Data) {
            stdoutBuffer += String(data: data, encoding: .utf8) ?? ""
            let parts = stdoutBuffer.split(separator: "\n", omittingEmptySubsequences: false)
            guard stdoutBuffer.hasSuffix("\n") else {
                stdoutBuffer = parts.last.map(String.init) ?? ""
                for line in parts.dropLast() { handleLineLocked(String(line)) }
                return
            }
            stdoutBuffer = ""
            for line in parts.dropLast() { handleLineLocked(String(line)) }
        }

        private func appendStderrLocked(_ data: Data) {
            stderrBuffer += String(data: data, encoding: .utf8) ?? ""
            if stderrBuffer.count > 4_000 {
                stderrBuffer = String(stderrBuffer.suffix(4_000))
            }
        }

        private func handleLineLocked(_ line: String) {
            guard let message = CodexAppServerClient.jsonObject(from: line) else { return }

            if let id = CodexAppServerClient.requestID(from: message["id"]),
               let method = pending.removeValue(forKey: id) {
                handleResponseLocked(message, for: method)
                return
            }

            handleNotificationLocked(message)
        }

        private func handleResponseLocked(_ message: [String: Any], for method: String) {
            if let error = CodexAppServerClient.jsonRPCErrorMessage(message) {
                finishLocked(.failure(AIError.transport(error)))
                return
            }

            do {
                switch method {
                case "initialize":
                    try sendLocked(method: "thread/start", params: CodexAppServerClient.threadStartParams(config: config, cwd: cwd))
                case "thread/start":
                    guard let threadID = CodexAppServerClient.threadID(from: message) else {
                        finishLocked(.failure(AIError.transport("Codex app-server did not return a thread id.")))
                        return
                    }
                    self.threadID = threadID
                    try sendLocked(method: "turn/start", params: CodexAppServerClient.turnStartParams(threadId: threadID, config: config, userText: userText, cwd: cwd))
                case "turn/start":
                    break
                default:
                    break
                }
            } catch {
                finishLocked(.failure(error))
            }
        }

        private func handleNotificationLocked(_ message: [String: Any]) {
            if let delta = CodexAppServerClient.agentMessageDelta(from: message), !delta.isEmpty {
                emitLocked(delta)
                return
            }

            if let text = CodexAppServerClient.completedAgentMessage(from: message) {
                completedText = text
                reconcileCompletedTextLocked(text)
                return
            }

            if let completion = CodexAppServerClient.turnCompletion(from: message) {
                switch completion.status {
                case "completed":
                    if emittedText.isEmpty, let completedText, !completedText.isEmpty {
                        emitLocked(completedText)
                    }
                    finishLocked(.success(()))
                case "failed":
                    finishLocked(.failure(AIError.transport(completion.error ?? "Codex turn failed.")))
                default:
                    finishLocked(.failure(AIError.transport("Codex turn ended with status \(completion.status).")))
                }
                return
            }

            if message["method"] as? String == "error" {
                let params = message["params"] as? [String: Any]
                let text = params?["message"] as? String
                finishLocked(.failure(AIError.transport(text ?? "Codex app-server reported an error.")))
            }
        }

        private func reconcileCompletedTextLocked(_ text: String) {
            guard !text.isEmpty else { return }
            if emittedText.isEmpty {
                emitLocked(text)
            } else if text.hasPrefix(emittedText) {
                let suffix = String(text.dropFirst(emittedText.count))
                if !suffix.isEmpty { emitLocked(suffix) }
            }
        }

        private func emitLocked(_ delta: String) {
            emittedText += delta
            onDelta(delta)
        }

        private func stderrTailLocked() -> String {
            stderrBuffer.trimmingCharacters(in: .whitespacesAndNewlines)
        }

        private func finishLocked(_ result: Result<Void, Error>) {
            guard !finished else { return }
            finished = true
            stdoutPipe.fileHandleForReading.readabilityHandler = nil
            stderrPipe.fileHandleForReading.readabilityHandler = nil
            try? stdinPipe.fileHandleForWriting.close()
            if process.isRunning { process.terminate() }
            let continuation = continuation
            self.continuation = nil
            continuation?.resume(with: result)
        }
    }
}
