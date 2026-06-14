import Foundation

/// Helpers for the local Codex CLI (`codex exec`). Codex is invoked as a
/// subprocess; it uses its own stored login, so no API key is needed.
enum CodexCLI {
    /// Models Codex commonly supports. Codex has no list endpoint, so these are
    /// presets; an empty model falls back to the user's codex config default.
    static let presetModels = ["gpt-5.5", "gpt-5-codex", "gpt-5", "o4-mini", "o3", "gpt-4.1"]

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

    /// Best-effort discovery of the codex binary. Runs off the main thread.
    static func detectPath() -> String {
        for path in candidatePaths() where FileManager.default.isExecutableFile(atPath: path) {
            return path
        }
        if let viaShell = resolveViaLoginShell(), FileManager.default.isExecutableFile(atPath: viaShell) {
            return viaShell
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
        process.arguments = ["-lc", "command -v codex"]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        do { try process.run() } catch { return nil }
        process.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines)
        return (path?.isEmpty == false) ? path : nil
    }
}
