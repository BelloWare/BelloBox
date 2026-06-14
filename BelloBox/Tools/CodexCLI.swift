import Foundation

/// Helpers for the local Codex command. BelloBox talks to `codex app-server`
/// over stdio; Codex uses its own stored login, so no API key is needed.
enum CodexCLI {
    /// Models and reasoning efforts Codex commonly supports. App-server has a
    /// model list endpoint, but these presets keep settings useful offline.
    static let defaultModel = "gpt-5.5"
    static let presetModels = [defaultModel, "gpt-5-codex", "gpt-5", "o4-mini", "o3", "gpt-4.1"]
    static let defaultReasoningEffort = "medium"
    static let reasoningEfforts = ["low", "medium", "high", "xhigh"]

    static func candidatePaths() -> [String] {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        var paths = [
            "/opt/homebrew/bin/codex",
            "/usr/local/bin/codex",
            "\(home)/.local/bin/codex",
            "\(home)/.bun/bin/codex",
            "\(home)/.volta/bin/codex",
        ]
        // nvm installs: ~/.nvm/versions/node/<version>/bin/codex (newest first).
        let nvmBase = "\(home)/.nvm/versions/node"
        if let versions = try? FileManager.default.contentsOfDirectory(atPath: nvmBase) {
            for version in versions.sorted().reversed() {
                paths.append("\(nvmBase)/\(version)/bin/codex")
            }
        }
        return paths
    }

    /// Best-effort discovery of the codex binary, for the optional Detect button.
    /// The user's shell is asked first, so it matches their terminal's codex
    /// (the default node/version), then well-known locations as a fallback.
    static func detectPath() -> String {
        if let viaShell = resolveViaLoginShell(), FileManager.default.isExecutableFile(atPath: viaShell) {
            return viaShell
        }
        for path in candidatePaths() where FileManager.default.isExecutableFile(atPath: path) {
            return path
        }
        return ""
    }

    static func isInstalled(at path: String) -> Bool {
        !path.trimmingCharacters(in: .whitespaces).isEmpty
            && FileManager.default.isExecutableFile(atPath: path)
    }

    private static func resolveViaLoginShell() -> String? {
        let shell = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"
        let process = Process()
        process.executableURL = URL(fileURLWithPath: shell)
        // Login + interactive so version managers (nvm) load and `codex`
        // resolves to the user's default — the same one their terminal uses.
        process.arguments = ["-l", "-i", "-c", "command -v codex"]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        do { try process.run() } catch { return nil }
        process.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        // An interactive shell may print rc noise; take the last non-empty line.
        let output = String(data: data, encoding: .utf8) ?? ""
        let path = output.split(separator: "\n").map { $0.trimmingCharacters(in: .whitespaces) }.last { !$0.isEmpty }
        return (path?.isEmpty == false) ? path : nil
    }
}
