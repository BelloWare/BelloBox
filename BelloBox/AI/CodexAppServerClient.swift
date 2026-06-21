import Foundation

enum CodexJSONRPCRequestID: Hashable, Equatable {
    case int(Int64)
    case string(String)

    var jsonValue: Any {
        switch self {
        case let .int(value):
            return value
        case let .string(value):
            return value
        }
    }
}

/// Minimal JSON-RPC client for `codex app-server` over its default stdio transport.
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

    static func resolvedApprovalPolicy(_ config: AIConfig) -> CodexApprovalPolicy {
        config.codexApprovalPolicy
    }

    static func resolvedSandboxMode(_ config: AIConfig) -> CodexSandboxMode {
        config.codexSandboxMode
    }

    static func sandboxPolicy(mode: CodexSandboxMode, writableRoot: String) -> [String: Any] {
        switch mode {
        case .readOnly:
            return [
                "type": "readOnly",
                "networkAccess": true,
            ]
        case .workspaceWrite:
            return [
                "type": "workspaceWrite",
                "writableRoots": [writableRoot],
                "networkAccess": true,
                "excludeTmpdirEnvVar": false,
                "excludeSlashTmp": false,
            ]
        case .dangerFullAccess:
            return [
                "type": "dangerFullAccess",
            ]
        }
    }

    static func developerInstructions(system: String) -> String {
        let directive = "Output only the resulting text itself. Do not explain, do not comment, do not ask questions, and do not use code fences."
        let trimmed = system.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? directive : "\(trimmed)\n\n\(directive)"
    }

    static func appServerCommand(_ pathOrCommand: String) -> String {
        "\(AIClient.codexInvocation(pathOrCommand)) app-server"
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
            "approvalPolicy": resolvedApprovalPolicy(config).rawValue,
            "sandbox": resolvedSandboxMode(config).rawValue,
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
            "approvalPolicy": resolvedApprovalPolicy(config).rawValue,
            "sandboxPolicy": sandboxPolicy(mode: resolvedSandboxMode(config), writableRoot: cwd),
            "model": resolvedModel(config),
            "effort": resolvedReasoningEffort(config),
        ]
    }

    static func jsonObject(from line: String) -> [String: Any]? {
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix("{"), let data = trimmed.data(using: .utf8) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }

    static func requestID(from value: Any?) -> CodexJSONRPCRequestID? {
        if value is Bool { return nil }
        if let string = value as? String { return .string(string) }
        if let int = value as? Int { return .int(Int64(int)) }
        if let int64 = value as? Int64 { return .int(int64) }
        if let number = value as? NSNumber { return .int(number.int64Value) }
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

    static func completionError(status: String, error: String?, emittedText: String, completedText: String?) -> AIError? {
        switch status {
        case "completed":
            let fallbackText = completedText?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            return emittedText.isEmpty && fallbackText.isEmpty ? .emptyResponse : nil
        case "failed":
            return .transport(error ?? "Codex turn failed.")
        default:
            return .transport("Codex turn ended with status \(status).")
        }
    }

    static func approvalDenialResult(forServerRequestMethod method: String) -> [String: Any]? {
        switch method {
        case "item/commandExecution/requestApproval",
             "item/fileChange/requestApproval":
            return ["decision": "denied"]
        case "execCommandApproval",
             "applyPatchApproval":
            return ["decision": "abort"]
        default:
            return nil
        }
    }

    static func unsupportedServerRequestMessage(method: String) -> String {
        switch method {
        case "item/commandExecution/requestApproval",
             "item/fileChange/requestApproval",
             "item/permissions/requestApproval",
             "execCommandApproval",
             "applyPatchApproval":
            return "Codex requested interactive approval, but Bello Box cannot show approval prompts during text actions. Set Codex approvals to Never, then try again."
        case "item/tool/requestUserInput",
             "mcpServer/elicitation/request":
            return "Codex requested interactive input, but Bello Box text actions cannot continue an interactive Codex session."
        case "item/tool/call":
            return "Codex requested a client-side tool call, but Bello Box does not expose Codex app-server tools."
        case "account/chatgptAuthTokens/refresh":
            return "Codex requested a ChatGPT token refresh. Open Codex directly to refresh your login, then try Bello Box again."
        case "attestation/generate":
            return "Codex requested attestation, but Bello Box does not provide app-server attestation."
        default:
            return "Codex app-server requested unsupported method \(method)."
        }
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
        private var nextRequestID: Int64 = 1
        private var pending: [CodexJSONRPCRequestID: String] = [:]
        private var threadID: String?
        private var emittedText = ""
        private var completedText: String?
        private var finished = false
        private var timeoutWorkItem: DispatchWorkItem?

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
                            self.startTimeoutLocked()
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

        private func startTimeoutLocked() {
            timeoutWorkItem?.cancel()
            let workItem = DispatchWorkItem { [weak self] in
                self?.finishLocked(.failure(AIError.transport("Codex app-server timed out after 300 seconds.")))
            }
            timeoutWorkItem = workItem
            queue.asyncAfter(deadline: .now() + 300, execute: workItem)
        }

        private func sendLocked(method: String, params: [String: Any]) throws {
            let id = CodexJSONRPCRequestID.int(nextRequestID)
            nextRequestID += 1
            pending[id] = method
            let message: [String: Any] = [
                "method": method,
                "id": id.jsonValue,
                "params": params,
            ]
            try writeJSONLocked(message)
        }

        private func sendResponseLocked(id: CodexJSONRPCRequestID, result: [String: Any]) throws {
            try writeJSONLocked([
                "id": id.jsonValue,
                "result": result,
            ])
        }

        private func sendErrorResponseLocked(id: CodexJSONRPCRequestID, message: String) throws {
            try writeJSONLocked([
                "id": id.jsonValue,
                "error": [
                    "code": -32000,
                    "message": message,
                ],
            ])
        }

        private func writeJSONLocked(_ message: [String: Any]) throws {
            var data = try JSONSerialization.data(withJSONObject: message, options: [])
            data.append(contentsOf: [0x0A])
            try stdinPipe.fileHandleForWriting.write(contentsOf: data)
        }

        private func handleStdoutLocked(_ data: Data) {
            stdoutBuffer += String(data: data, encoding: .utf8) ?? ""
            let parts = stdoutBuffer.split(separator: "\n", omittingEmptySubsequences: false)
            guard stdoutBuffer.hasSuffix("\n") else {
                stdoutBuffer = parts.last.map(String.init) ?? ""
                for line in parts.dropLast() {
                    handleLineLocked(String(line))
                }
                return
            }
            stdoutBuffer = ""
            for line in parts.dropLast() {
                handleLineLocked(String(line))
            }
        }

        private func appendStderrLocked(_ data: Data) {
            stderrBuffer += String(data: data, encoding: .utf8) ?? ""
            if stderrBuffer.count > 4000 {
                stderrBuffer = String(stderrBuffer.suffix(4000))
            }
        }

        private func handleLineLocked(_ line: String) {
            guard !finished else { return }
            guard let message = CodexAppServerClient.jsonObject(from: line) else { return }

            if let id = CodexAppServerClient.requestID(from: message["id"]) {
                if let method = pending.removeValue(forKey: id) {
                    handleResponseLocked(message, for: method)
                    return
                }
                if let method = message["method"] as? String {
                    handleServerRequestLocked(id: id, method: method)
                }
                return
            }

            handleNotificationLocked(message)
        }

        private func handleServerRequestLocked(id: CodexJSONRPCRequestID, method: String) {
            let message = CodexAppServerClient.unsupportedServerRequestMessage(method: method)
            do {
                if let result = CodexAppServerClient.approvalDenialResult(forServerRequestMethod: method) {
                    try sendResponseLocked(id: id, result: result)
                } else {
                    try sendErrorResponseLocked(id: id, message: message)
                }
            } catch {
                finishLocked(.failure(error))
                return
            }
            finishLocked(.failure(AIError.transport(message)))
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
                if let error = CodexAppServerClient.completionError(
                    status: completion.status,
                    error: completion.error,
                    emittedText: emittedText,
                    completedText: completedText
                ) {
                    finishLocked(.failure(error))
                } else {
                    if emittedText.isEmpty, let completedText, !completedText.isEmpty {
                        emitLocked(completedText)
                    }
                    finishLocked(.success(()))
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
            timeoutWorkItem?.cancel()
            timeoutWorkItem = nil
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
