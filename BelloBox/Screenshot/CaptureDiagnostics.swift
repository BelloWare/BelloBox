import Foundation

enum CaptureDiagnostics {
    enum ExportError: LocalizedError {
        case noLogFile

        var errorDescription: String? {
            switch self {
            case .noLogFile:
                return "No diagnostics log has been written yet."
            }
        }
    }

    private static let lock = NSLock()

    static var logURL: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        return base
            .appendingPathComponent("BelloBox", isDirectory: true)
            .appendingPathComponent("Logs", isDirectory: true)
            .appendingPathComponent("capture-diagnostics.log", isDirectory: false)
    }

    static func log(_ event: String, enabled: Bool, details: [String] = []) {
        guard enabled else { return }
        let timestamp = ISO8601DateFormatter().string(from: Date())
        let line = ([timestamp, event] + details).joined(separator: " | ") + "\n"
        append(line, to: logURL)
    }

    static func exportLog(to destination: URL) throws {
        guard FileManager.default.fileExists(atPath: logURL.path) else {
            throw ExportError.noLogFile
        }
        if FileManager.default.fileExists(atPath: destination.path) {
            try FileManager.default.removeItem(at: destination)
        }
        try FileManager.default.copyItem(at: logURL, to: destination)
    }

    static func write(_ text: String, enabled: Bool, to url: URL) {
        guard enabled else { return }
        append(text, to: url)
    }

    private static func append(_ text: String, to url: URL) {
        lock.lock()
        defer { lock.unlock() }

        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            let data = Data(text.utf8)
            if FileManager.default.fileExists(atPath: url.path) {
                let handle = try FileHandle(forWritingTo: url)
                defer { try? handle.close() }
                try handle.seekToEnd()
                try handle.write(contentsOf: data)
            } else {
                try data.write(to: url, options: .atomic)
            }
        } catch {
            NSLog("Bello Box capture diagnostics write failed: \(error.localizedDescription)")
        }
    }
}

struct CaptureTiming {
    let name: String
    private let start = Date()

    init(_ name: String) {
        self.name = name
    }

    func finish(_ details: [String] = [], enabled: Bool) {
        let elapsedMs = Int((Date().timeIntervalSince(start) * 1000).rounded())
        CaptureDiagnostics.log(name, enabled: enabled, details: details + ["elapsedMs=\(elapsedMs)"])
    }
}
