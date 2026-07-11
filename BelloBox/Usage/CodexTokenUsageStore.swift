import Foundation

struct CodexTokenUsageStore: Sendable {
    let codexHome: URL

    init(codexHome: URL = Self.defaultCodexHome()) {
        self.codexHome = codexHome
    }

    static func defaultCodexHome(environment: [String: String] = ProcessInfo.processInfo.environment) -> URL {
        let rawPath = environment["CODEX_HOME"]?.trimmingCharacters(in: .whitespacesAndNewlines)
        let path = rawPath?.isEmpty == false ? rawPath! : "~/.codex"
        return URL(fileURLWithPath: NSString(string: path).expandingTildeInPath, isDirectory: true)
    }

    func loadEvents() throws -> CodexTokenUsageScanResult {
        let files = try rolloutFiles()
        var events: [CodexTokenUsageEvent] = []
        var unreadableFiles: [CodexTokenUsageUnreadableFile] = []

        for file in files {
            do {
                events.append(contentsOf: try Self.parseUsageEvents(in: file))
            } catch {
                unreadableFiles.append(
                    CodexTokenUsageUnreadableFile(path: file.path, message: error.localizedDescription)
                )
            }
        }

        events.sort {
            if $0.timestamp == $1.timestamp {
                return $0.id < $1.id
            }
            return $0.timestamp < $1.timestamp
        }
        return CodexTokenUsageScanResult(
            codexHome: codexHome,
            filesScanned: files.count,
            unreadableFiles: unreadableFiles,
            events: events
        )
    }

    func rolloutFiles() throws -> [URL] {
        let roots = [
            codexHome.appendingPathComponent("sessions", isDirectory: true),
            codexHome.appendingPathComponent("archived_sessions", isDirectory: true),
        ]
        var seen = Set<String>()
        var urls: [URL] = []

        for root in roots where FileManager.default.fileExists(atPath: root.path) {
            guard let enumerator = FileManager.default.enumerator(
                at: root,
                includingPropertiesForKeys: [.isRegularFileKey],
                options: [.skipsPackageDescendants]
            ) else { continue }

            for case let url as URL in enumerator {
                guard url.pathExtension == "jsonl" else { continue }
                let values = try? url.resourceValues(forKeys: [.isRegularFileKey])
                guard values?.isRegularFile == true else { continue }
                let resolved = url.resolvingSymlinksInPath().standardizedFileURL.path
                guard !seen.contains(resolved) else { continue }
                seen.insert(resolved)
                urls.append(url)
            }
        }

        return urls.sorted { $0.path < $1.path }
    }

    static func parseUsageEvents(in url: URL) throws -> [CodexTokenUsageEvent] {
        let reader = try UTF8LineReader(url: url)
        defer { reader.close() }

        let timestampParser = CodexTimestampParser()
        var events: [CodexTokenUsageEvent] = []
        var previousUsage: CodexTokenUsage?
        var currentModel: String?
        var currentTurnID: String?
        var lineNumber = 0

        while let line = try reader.nextLine() {
            lineNumber += 1
            guard let record = Self.jsonObject(from: line),
                  let type = record["type"] as? String
            else { continue }

            if type == "turn_context" {
                if let model = Self.explicitModel(in: record) {
                    currentModel = model
                }
                if let turnID = Self.explicitTurnID(in: record) {
                    currentTurnID = turnID
                }
                continue
            }

            guard type == "event_msg",
                  let payload = record["payload"] as? [String: Any],
                  payload["type"] as? String == "token_count",
                  let info = payload["info"] as? [String: Any],
                  let timestamp = timestampParser.parse(record["timestamp"])
            else { continue }

            let current = CodexTokenUsage.fromDictionary(info["total_token_usage"])
            let delta = current.delta(from: previousUsage)
            previousUsage = current
            guard !delta.isZero else { continue }

            let eventModel = Self.explicitModel(in: payload) ??
                Self.explicitModel(in: info) ??
                currentModel
            let eventTurnID = Self.explicitTurnID(in: payload) ??
                Self.explicitTurnID(in: info) ??
                currentTurnID
            let eventID = "\(url.resolvingSymlinksInPath().standardizedFileURL.path):\(lineNumber)"
            events.append(
                CodexTokenUsageEvent(
                    id: eventID,
                    timestamp: timestamp,
                    model: CodexTokenUsageModel.normalized(eventModel),
                    turnID: eventTurnID,
                    usage: delta,
                    sourcePath: url.path,
                    lineNumber: lineNumber
                )
            )
        }

        return events
    }

    private static func jsonObject(from line: String) -> [String: Any]? {
        guard let data = line.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data),
              let dictionary = object as? [String: Any]
        else { return nil }
        return dictionary
    }

    private static func explicitModel(in dictionary: [String: Any]) -> String? {
        let direct = string(dictionary["model"]) ?? string(dictionary["model_name"])
        if let direct {
            return direct
        }
        if let payload = dictionary["payload"] as? [String: Any] {
            return explicitModel(in: payload)
        }
        return nil
    }

    private static func explicitTurnID(in dictionary: [String: Any]) -> String? {
        let direct = string(dictionary["turn_id"]) ?? string(dictionary["turnID"])
        if let direct {
            return direct
        }
        if let payload = dictionary["payload"] as? [String: Any] {
            return explicitTurnID(in: payload)
        }
        return nil
    }

    private static func string(_ value: Any?) -> String? {
        guard let string = value as? String else { return nil }
        let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

enum CodexTokenUsageAggregator {
    static func report(
        events: [CodexTokenUsageEvent],
        range: DateInterval,
        intervalPreference: CodexTokenUsageInterval,
        modelFilter: String?,
        calendar: Calendar = .autoupdatingCurrent
    ) -> CodexTokenUsageReport {
        let resolvedInterval = CodexTokenUsageInterval.resolved(preference: intervalPreference, range: range)
        let filteredEvents = events.filter { event in
            range.contains(event.timestamp) &&
                (modelFilter == nil || event.displayModel == modelFilter)
        }
        var total = CodexTokenUsage.zero
        var bucketsByStart: [Date: CodexTokenUsage] = [:]
        var modelTotals: [String: CodexTokenUsage] = [:]

        for event in filteredEvents {
            total = total.adding(event.usage)
            let bucketStart = resolvedInterval.bucketStart(for: event.timestamp, calendar: calendar)
            bucketsByStart[bucketStart, default: .zero] = bucketsByStart[bucketStart, default: .zero].adding(event.usage)
            modelTotals[event.displayModel, default: .zero] = modelTotals[event.displayModel, default: .zero].adding(event.usage)
        }

        let buckets = bucketsByStart
            .map { start, usage in
                CodexTokenUsageBucket(
                    start: start,
                    end: resolvedInterval.bucketEnd(after: start, calendar: calendar),
                    usage: usage
                )
            }
            .sorted { $0.start < $1.start }

        let rows = modelTotals
            .map { modelName, usage in CodexTokenUsageModelTotal(modelName: modelName, usage: usage) }
            .sorted {
                if $0.usage.totalTokens == $1.usage.totalTokens {
                    return $0.modelName < $1.modelName
                }
                return $0.usage.totalTokens > $1.usage.totalTokens
            }

        return CodexTokenUsageReport(
            range: range,
            resolvedInterval: resolvedInterval,
            events: filteredEvents,
            buckets: buckets,
            total: total,
            modelTotals: rows
        )
    }
}

private final class CodexTimestampParser {
    private let fractionalFormatter: ISO8601DateFormatter
    private let wholeSecondFormatter: ISO8601DateFormatter

    init() {
        fractionalFormatter = ISO8601DateFormatter()
        fractionalFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        wholeSecondFormatter = ISO8601DateFormatter()
        wholeSecondFormatter.formatOptions = [.withInternetDateTime]
    }

    func parse(_ value: Any?) -> Date? {
        guard let string = value as? String, !string.isEmpty else { return nil }
        return fractionalFormatter.date(from: string) ?? wholeSecondFormatter.date(from: string)
    }
}

private final class UTF8LineReader {
    private let handle: FileHandle
    private var buffer = Data()
    private let newline = Data([0x0a])

    init(url: URL) throws {
        handle = try FileHandle(forReadingFrom: url)
    }

    func nextLine() throws -> String? {
        while true {
            if let range = buffer.firstRange(of: newline) {
                let lineData = buffer.subdata(in: buffer.startIndex..<range.lowerBound)
                buffer.removeSubrange(buffer.startIndex..<range.upperBound)
                return String(data: lineData, encoding: .utf8) ?? String(decoding: lineData, as: UTF8.self)
            }

            let chunk = try handle.read(upToCount: 64 * 1024) ?? Data()
            if chunk.isEmpty {
                guard !buffer.isEmpty else { return nil }
                let lineData = buffer
                buffer.removeAll(keepingCapacity: false)
                return String(data: lineData, encoding: .utf8) ?? String(decoding: lineData, as: UTF8.self)
            }

            buffer.append(chunk)
        }
    }

    func close() {
        try? handle.close()
    }
}
