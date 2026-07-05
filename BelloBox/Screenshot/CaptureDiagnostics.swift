import Foundation

enum CaptureDiagnostics {
    private static let maximumLogBytes = 4 * 1024 * 1024
    private static let retainedLogBytes = 2 * 1024 * 1024

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
        append(line, to: logURL, maximumLogBytes: maximumLogBytes, retainedLogBytes: retainedLogBytes)
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

    static func readLogTail(maxBytes: Int = 64 * 1024, from url: URL = logURL) -> String? {
        lock.lock()
        defer { lock.unlock() }

        guard maxBytes > 0,
              FileManager.default.fileExists(atPath: url.path),
              let handle = try? FileHandle(forReadingFrom: url)
        else { return nil }
        defer { try? handle.close() }

        let fileSize = (try? handle.seekToEnd()) ?? 0
        let start = fileSize > UInt64(maxBytes) ? fileSize - UInt64(maxBytes) : 0
        do {
            try handle.seek(toOffset: start)
            let data = try handle.readToEnd() ?? Data()
            guard var text = String(data: data, encoding: .utf8) else { return nil }
            if start > 0 {
                text = "[last \(maxBytes) bytes of \(fileSize) byte log]\n" + text
            }
            return text
        } catch {
            return nil
        }
    }

    static func write(
        _ text: String,
        enabled: Bool,
        to url: URL,
        maximumLogBytes: Int? = nil,
        retainedLogBytes: Int? = nil
    ) {
        guard enabled else { return }
        append(
            text,
            to: url,
            maximumLogBytes: maximumLogBytes ?? Self.maximumLogBytes,
            retainedLogBytes: retainedLogBytes ?? Self.retainedLogBytes
        )
    }

    private static func append(
        _ text: String,
        to url: URL,
        maximumLogBytes: Int,
        retainedLogBytes: Int
    ) {
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
            try trimIfNeeded(url, maximumLogBytes: maximumLogBytes, retainedLogBytes: retainedLogBytes)
        } catch {
            NSLog("Bello Box capture diagnostics write failed: \(error.localizedDescription)")
        }
    }

    private static func trimIfNeeded(_ url: URL, maximumLogBytes: Int, retainedLogBytes: Int) throws {
        guard maximumLogBytes > 0,
              retainedLogBytes > 0,
              retainedLogBytes < maximumLogBytes
        else { return }

        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        guard let fileSize = attributes[.size] as? NSNumber,
              fileSize.intValue > maximumLogBytes,
              let handle = try? FileHandle(forReadingFrom: url)
        else { return }
        defer { try? handle.close() }

        let offset = max(0, fileSize.intValue - retainedLogBytes)
        try handle.seek(toOffset: UInt64(offset))
        var data = try handle.readToEnd() ?? Data()
        if offset > 0 {
            if let newlineIndex = data.firstIndex(of: UInt8(ascii: "\n")) {
                data.removeSubrange(...newlineIndex)
            } else {
                data.removeAll()
            }
        }
        try data.write(to: url, options: .atomic)
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
