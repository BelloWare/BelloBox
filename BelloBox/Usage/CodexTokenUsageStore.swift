import Darwin
import Foundation

struct CodexTokenUsageStore: Sendable {
    private static let tokenCountMarker = Data("\"token_count\"".utf8)
    private static let turnContextMarker = Data("\"turn_context\"".utf8)

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
        let files = try rolloutFiles(modifiedOnOrAfter: nil)
        var events: [CodexTokenUsageEvent] = []
        var unreadableFiles: [CodexTokenUsageUnreadableFile] = []

        for file in files {
            try Self.checkCancellation()
            do {
                let inheritedBaseline = try Self.inheritedBaseline(for: file)
                events.append(
                    contentsOf: try Self.parseUsageEvents(
                        in: file.url,
                        inheritedBaseline: inheritedBaseline
                    )
                )
            } catch is CancellationError {
                throw CancellationError()
            } catch {
                unreadableFiles.append(
                    CodexTokenUsageUnreadableFile(path: file.url.path, message: error.localizedDescription)
                )
            }
        }

        events.sort {
            if $0.timestamp == $1.timestamp { return $0.id < $1.id }
            return $0.timestamp < $1.timestamp
        }
        let totalBytes = files.reduce(0) { $0 + $1.size }
        return CodexTokenUsageScanResult(
            codexHome: codexHome,
            filesScanned: files.count,
            bytesRead: totalBytes,
            candidateBytes: totalBytes,
            unreadableFiles: unreadableFiles,
            events: events
        )
    }

    func loadEventsConcurrently(in range: DateInterval) async throws -> CodexTokenUsageScanResult {
        let files = try rolloutFiles(modifiedOnOrAfter: range.start)
        var events: [CodexTokenUsageEvent] = []
        var unreadableFiles: [(index: Int, value: CodexTokenUsageUnreadableFile)] = []
        var bytesRead: Int64 = 0
        let concurrencyLimit = min(
            files.count,
            max(1, min(2, ProcessInfo.processInfo.activeProcessorCount))
        )

        try await withThrowingTaskGroup(of: IndexedParseOutcome.self) { group in
            var nextIndex = 0
            while nextIndex < concurrencyLimit {
                let index = nextIndex
                let file = files[index]
                group.addTask {
                    try Self.parseOutcome(file: file, index: index, range: range)
                }
                nextIndex += 1
            }

            while let outcome = try await group.next() {
                switch outcome {
                case let .success(_, parsed):
                    events.append(contentsOf: parsed.events)
                    bytesRead += parsed.bytesRead
                case let .failure(index, unreadable):
                    unreadableFiles.append((index, unreadable))
                }

                if nextIndex < files.count {
                    let index = nextIndex
                    let file = files[index]
                    group.addTask {
                        try Self.parseOutcome(file: file, index: index, range: range)
                    }
                    nextIndex += 1
                }
            }
        }

        events.sort {
            if $0.timestamp == $1.timestamp { return $0.id < $1.id }
            return $0.timestamp < $1.timestamp
        }
        return CodexTokenUsageScanResult(
            codexHome: codexHome,
            filesScanned: files.count,
            bytesRead: bytesRead,
            candidateBytes: files.reduce(0) { $0 + $1.size },
            unreadableFiles: unreadableFiles.sorted { $0.index < $1.index }.map(\.value),
            events: events
        )
    }

    private static func parseOutcome(
        file: RolloutFile,
        index: Int,
        range: DateInterval
    ) throws -> IndexedParseOutcome {
        try Task.checkCancellation()
        do {
            let inheritedBaseline = try inheritedBaseline(for: file)
            let parsed = try parseUsageEventsReverse(
                in: file.url,
                range: range,
                inheritedBaseline: inheritedBaseline
            )
            return .success(index: index, parsed: parsed)
        } catch is CancellationError {
            throw CancellationError()
        } catch {
            return .failure(
                index: index,
                unreadable: CodexTokenUsageUnreadableFile(
                    path: file.url.path,
                    message: error.localizedDescription
                )
            )
        }
    }

    func rolloutFiles() throws -> [URL] {
        try rolloutFiles(modifiedOnOrAfter: nil).map(\.url)
    }

    private func rolloutFiles(modifiedOnOrAfter cutoff: Date?) throws -> [RolloutFile] {
        let roots = [
            codexHome.appendingPathComponent("sessions", isDirectory: true),
            codexHome.appendingPathComponent("archived_sessions", isDirectory: true),
        ]
        var seen = Set<String>()
        var discoveredFiles: [DiscoveredRolloutFile] = []

        for root in roots where FileManager.default.fileExists(atPath: root.path) {
            guard let enumerator = FileManager.default.enumerator(
                at: root,
                includingPropertiesForKeys: [.isRegularFileKey, .contentModificationDateKey, .fileSizeKey],
                options: [.skipsPackageDescendants]
            ) else { continue }

            for case let url as URL in enumerator {
                guard url.pathExtension == "jsonl" else { continue }
                let values = try? url.resourceValues(
                    forKeys: [.isRegularFileKey, .contentModificationDateKey, .fileSizeKey]
                )
                guard values?.isRegularFile == true else { continue }
                let resolved = url.resolvingSymlinksInPath().standardizedFileURL.path
                guard !seen.contains(resolved) else { continue }
                seen.insert(resolved)
                discoveredFiles.append(
                    DiscoveredRolloutFile(
                        url: url,
                        size: Int64(values?.fileSize ?? 0),
                        modificationDate: values?.contentModificationDate
                    )
                )
            }
        }

        let filesByThreadID = Dictionary(
            discoveredFiles.compactMap { file in
                Self.threadID(fromRolloutURL: file.url).map { ($0, file.url) }
            },
            uniquingKeysWith: { first, _ in first }
        )
        return discoveredFiles
            .filter { file in
                guard let cutoff, let modificationDate = file.modificationDate else { return true }
                return modificationDate >= cutoff
            }
            .map { file in
                RolloutFile(
                    url: file.url,
                    size: file.size,
                    history: Self.rolloutHistory(
                        in: file.url,
                        filesByThreadID: filesByThreadID
                    )
                )
            }
            .sorted { $0.url.path < $1.url.path }
    }

    static func parseUsageEvents(in url: URL) throws -> [CodexTokenUsageEvent] {
        try parseUsageEvents(in: url, inheritedBaseline: nil)
    }

    private static func parseUsageEvents(
        in url: URL,
        inheritedBaseline: CodexTokenUsage?
    ) throws -> [CodexTokenUsageEvent] {
        let reader = try UTF8LineReader(url: url)
        defer { reader.close() }

        let timestampParser = CodexTimestampParser()
        var events: [CodexTokenUsageEvent] = []
        var previousUsage: CodexTokenUsage?
        var currentModel: String?
        var currentTurnID: String?
        var isReplayingParentHistory = inheritedBaseline != nil
        var lineNumber = 0
        let sourcePath = url.path
        let sourceIdentity = url.resolvingSymlinksInPath().standardizedFileURL.path

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
            if isReplayingParentHistory {
                if let inheritedBaseline, current == inheritedBaseline {
                    isReplayingParentHistory = false
                    previousUsage = current
                }
                continue
            }
            let delta = current.delta(from: previousUsage)
            previousUsage = current
            guard !delta.isZero else { continue }

            let eventModel = Self.explicitModel(in: payload) ??
                Self.explicitModel(in: info) ??
                currentModel
            let eventTurnID = Self.explicitTurnID(in: payload) ??
                Self.explicitTurnID(in: info) ??
                currentTurnID
            let eventID = "\(sourceIdentity):\(lineNumber)"
            events.append(
                CodexTokenUsageEvent(
                    id: eventID,
                    timestamp: timestamp,
                    model: CodexTokenUsageModel.normalized(eventModel),
                    turnID: eventTurnID,
                    usage: delta,
                    sourcePath: sourcePath,
                    lineNumber: lineNumber
                )
            )
        }

        return events
    }

    private static func parseUsageEventsReverse(
        in url: URL,
        range: DateInterval,
        inheritedBaseline: CodexTokenUsage?
    ) throws -> ParsedUsageFile {
        let reader = try ReverseUTF8LineReader(url: url)
        defer { reader.close() }

        let timestampParser = CodexTimestampParser()
        var recordsNewestFirst: [ParsedUsageRecord] = []
        var foundRelevantToken = false
        var foundBaselineToken = false
        var earliestRelevantTimestamp: Date?
        var oldestContextTimestamp: Date?

        scanLines: while let line = try reader.nextLine() {
            try checkCancellation()
            guard line.data.range(of: tokenCountMarker) != nil ||
                    line.data.range(of: turnContextMarker) != nil,
                  let record = parsedUsageRecord(from: line, timestampParser: timestampParser),
                  record.timestamp <= range.end
            else { continue }

            switch record {
            case let .context(timestamp, _, _):
                guard foundRelevantToken else {
                    if timestamp < range.start { break scanLines }
                    continue
                }
                recordsNewestFirst.append(record)
                oldestContextTimestamp = min(oldestContextTimestamp ?? timestamp, timestamp)
            case let .token(timestamp, current, _, _, _):
                if let inheritedBaseline, current == inheritedBaseline {
                    foundBaselineToken = true
                    break scanLines
                }
                if timestamp < range.start {
                    guard foundRelevantToken else { break scanLines }
                    guard !foundBaselineToken else { continue }
                    foundBaselineToken = true
                } else {
                    foundRelevantToken = true
                    earliestRelevantTimestamp = min(earliestRelevantTimestamp ?? timestamp, timestamp)
                }
                recordsNewestFirst.append(record)
            }

            if foundBaselineToken,
               let earliestRelevantTimestamp,
               let oldestContextTimestamp,
               oldestContextTimestamp <= earliestRelevantTimestamp {
                break
            }
        }

        guard foundRelevantToken else {
            return ParsedUsageFile(events: [], bytesRead: reader.bytesRead)
        }

        var events: [CodexTokenUsageEvent] = []
        var previousUsage = inheritedBaseline
        var currentModel: String?
        var currentTurnID: String?
        let sourcePath = url.path
        let sourceIdentity = url.resolvingSymlinksInPath().standardizedFileURL.path

        for record in recordsNewestFirst.reversed() {
            switch record {
            case let .context(_, model, turnID):
                if let model { currentModel = model }
                if let turnID { currentTurnID = turnID }
            case let .token(timestamp, current, model, turnID, sourceOffset):
                let delta = current.delta(from: previousUsage)
                previousUsage = current
                guard range.contains(timestamp), !delta.isZero else { continue }
                events.append(
                    CodexTokenUsageEvent(
                        id: "\(sourceIdentity):\(sourceOffset)",
                        timestamp: timestamp,
                        model: CodexTokenUsageModel.normalized(model ?? currentModel),
                        turnID: turnID ?? currentTurnID,
                        usage: delta,
                        sourcePath: sourcePath,
                        lineNumber: 0
                    )
                )
            }
        }

        return ParsedUsageFile(events: events, bytesRead: reader.bytesRead)
    }

    private static func rolloutHistory(
        in url: URL,
        filesByThreadID: [String: URL]
    ) -> RolloutHistory {
        do {
            let handle = try FileHandle(forReadingFrom: url)
            defer { try? handle.close() }
            let prefix = try handle.read(upToCount: 64 * 1_024) ?? Data()
            let lines = prefix.split(separator: 0x0a, maxSplits: 2, omittingEmptySubsequences: false)
            guard let firstLine = lines.first,
                  let first = jsonObject(from: Data(firstLine)),
                  first["type"] as? String == "session_meta",
                  let payload = first["payload"] as? [String: Any]
            else { return .independent }

            guard lines.count > 1,
                  let second = jsonObject(from: Data(lines[1])),
                  second["type"] as? String == "session_meta"
            else {
                // Ordinary sessions and older subagents contain only their own metadata.
                return .independent
            }
            guard let inheritedPayload = second["payload"] as? [String: Any],
                  let parentThreadID = string(inheritedPayload["id"]),
                  parentThreadID != string(payload["id"]),
                  let childStartedAt = CodexTimestampParser().parse(first["timestamp"])
            else {
                return .invalid("The inherited parent metadata is inconsistent.")
            }
            if let source = payload["source"] as? [String: Any],
               let subagent = source["subagent"] as? [String: Any],
               let spawn = subagent["thread_spawn"] as? [String: Any],
               let declaredParentID = string(spawn["parent_thread_id"]),
               declaredParentID != parentThreadID {
                return .invalid("The inherited parent does not match the subagent lineage.")
            }
            guard let parentURL = filesByThreadID[parentThreadID] else {
                return .invalid("The inherited parent rollout \(parentThreadID) is unavailable.")
            }
            return .replayedParent(parentURL: parentURL, childStartedAt: childStartedAt)
        } catch {
            return .invalid(error.localizedDescription)
        }
    }

    private static func inheritedBaseline(for file: RolloutFile) throws -> CodexTokenUsage? {
        switch file.history {
        case .independent:
            return nil
        case let .invalid(message):
            throw CodexTokenUsageStoreError.invalidReplayMetadata(path: file.url.path, message: message)
        case let .replayedParent(parentURL, childStartedAt):
            guard let baseline = try findForkBaseline(
                childURL: file.url,
                parentURL: parentURL,
                childStartedAt: childStartedAt
            ) else {
                throw CodexTokenUsageStoreError.inheritedBaselineNotFound(path: file.url.path)
            }
            return baseline
        }
    }

    private static func findForkBaseline(
        childURL: URL,
        parentURL: URL,
        childStartedAt: Date
    ) throws -> CodexTokenUsage? {
        let childReader = try ReverseUTF8LineReader(url: childURL)
        defer { childReader.close() }
        let parentReader = try ReverseUTF8LineReader(url: parentURL)
        defer { parentReader.close() }
        let childTimestampParser = CodexTimestampParser()
        let parentTimestampParser = CodexTimestampParser()
        var child = try nextTokenSnapshot(
            from: childReader,
            timestampParser: childTimestampParser,
            notAfter: nil
        )
        var parent = try nextTokenSnapshot(
            from: parentReader,
            timestampParser: parentTimestampParser,
            notAfter: childStartedAt
        )
        var childUsages = Set<CodexTokenUsage>()
        var parentUsages = Set<CodexTokenUsage>()

        while child != nil || parent != nil {
            if let childSnapshot = child {
                if parentUsages.contains(childSnapshot.usage) {
                    return childSnapshot.usage
                }
                childUsages.insert(childSnapshot.usage)
                child = try nextTokenSnapshot(
                    from: childReader,
                    timestampParser: childTimestampParser,
                    notAfter: nil
                )
            }

            if let parentSnapshot = parent {
                if childUsages.contains(parentSnapshot.usage) {
                    return parentSnapshot.usage
                }
                parentUsages.insert(parentSnapshot.usage)
                parent = try nextTokenSnapshot(
                    from: parentReader,
                    timestampParser: parentTimestampParser,
                    notAfter: childStartedAt
                )
            }
        }
        return nil
    }

    private static func nextTokenSnapshot(
        from reader: ReverseUTF8LineReader,
        timestampParser: CodexTimestampParser,
        notAfter cutoff: Date?
    ) throws -> TokenSnapshot? {
        while let line = try reader.nextLine() {
            try checkCancellation()
            guard line.data.range(of: tokenCountMarker) != nil,
                  let record = parsedUsageRecord(from: line, timestampParser: timestampParser),
                  case let .token(timestamp, usage, _, _, _) = record
            else { continue }
            if let cutoff, timestamp > cutoff { continue }
            return TokenSnapshot(usage: usage)
        }
        return nil
    }

    private static func threadID(fromRolloutURL url: URL) -> String? {
        let stem = url.deletingPathExtension().lastPathComponent
        guard stem.count >= 36 else { return nil }
        let candidate = String(stem.suffix(36))
        return UUID(uuidString: candidate) == nil ? nil : candidate.lowercased()
    }

    private static func parsedUsageRecord(
        from line: ReverseUTF8LineReader.Line,
        timestampParser: CodexTimestampParser
    ) -> ParsedUsageRecord? {
        autoreleasepool {
            guard let record = jsonObject(from: line.data),
                  let timestamp = timestampParser.parse(record["timestamp"]),
                  let type = record["type"] as? String
            else { return nil }

            if type == "turn_context" {
                return .context(
                    timestamp: timestamp,
                    model: explicitModel(in: record),
                    turnID: explicitTurnID(in: record)
                )
            }
            guard type == "event_msg",
                  let payload = record["payload"] as? [String: Any],
                  payload["type"] as? String == "token_count",
                  let info = payload["info"] as? [String: Any]
            else { return nil }
            return .token(
                timestamp: timestamp,
                usage: CodexTokenUsage.fromDictionary(info["total_token_usage"]),
                model: explicitModel(in: payload) ?? explicitModel(in: info),
                turnID: explicitTurnID(in: payload) ?? explicitTurnID(in: info),
                sourceOffset: line.offset
            )
        }
    }

    private static func jsonObject(from line: String) -> [String: Any]? {
        guard let data = line.data(using: .utf8),
              let dictionary = jsonObject(from: data)
        else { return nil }
        return dictionary
    }

    private static func jsonObject(from data: Data) -> [String: Any]? {
        guard let object = try? JSONSerialization.jsonObject(with: data),
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

    private static func checkCancellation() throws {
        if Task<Never, Never>.isCancelled {
            throw CancellationError()
        }
    }
}

private struct RolloutFile: Sendable {
    let url: URL
    let size: Int64
    let history: RolloutHistory
}

private struct DiscoveredRolloutFile {
    let url: URL
    let size: Int64
    let modificationDate: Date?
}

private enum RolloutHistory: Sendable {
    case independent
    case replayedParent(parentURL: URL, childStartedAt: Date)
    case invalid(String)
}

private struct TokenSnapshot {
    let usage: CodexTokenUsage
}

private enum CodexTokenUsageStoreError: LocalizedError {
    case invalidReplayMetadata(path: String, message: String)
    case inheritedBaselineNotFound(path: String)

    var errorDescription: String? {
        switch self {
        case let .invalidReplayMetadata(path, message):
            return "Cannot read inherited history metadata in \(path): \(message)"
        case let .inheritedBaselineNotFound(path):
            return "Cannot identify the inherited token baseline in \(path)."
        }
    }
}

private struct ParsedUsageFile: Sendable {
    let events: [CodexTokenUsageEvent]
    let bytesRead: Int64
}

private enum IndexedParseOutcome: Sendable {
    case success(index: Int, parsed: ParsedUsageFile)
    case failure(index: Int, unreadable: CodexTokenUsageUnreadableFile)

}

private enum ParsedUsageRecord {
    case context(timestamp: Date, model: String?, turnID: String?)
    case token(
        timestamp: Date,
        usage: CodexTokenUsage,
        model: String?,
        turnID: String?,
        sourceOffset: UInt64
    )

    var timestamp: Date {
        switch self {
        case let .context(timestamp, _, _), let .token(timestamp, _, _, _, _): return timestamp
        }
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
        var eventCount = 0
        var total = CodexTokenUsage.zero
        var bucketsByStart: [Date: CodexTokenUsage] = [:]
        var modelTotals: [String: CodexTokenUsage] = [:]
        var outputMinutes = Set<Date>()
        var outputHours = Set<Date>()
        var outputDays = Set<Date>()

        for event in events where range.contains(event.timestamp) &&
            (modelFilter == nil || event.displayModel == modelFilter) {
            eventCount += 1
            total = total.adding(event.usage)
            let bucketStart = resolvedInterval.bucketStart(for: event.timestamp, calendar: calendar)
            bucketsByStart[bucketStart, default: .zero] = bucketsByStart[bucketStart, default: .zero].adding(event.usage)
            modelTotals[event.displayModel, default: .zero] = modelTotals[event.displayModel, default: .zero].adding(event.usage)
            if event.usage.outputTokens > 0 {
                outputMinutes.insert(CodexTokenUsageInterval.minute.bucketStart(for: event.timestamp, calendar: calendar))
                outputHours.insert(CodexTokenUsageInterval.hour.bucketStart(for: event.timestamp, calendar: calendar))
                outputDays.insert(CodexTokenUsageInterval.day.bucketStart(for: event.timestamp, calendar: calendar))
            }
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
        let output = Double(total.outputTokens)
        let activeOutputAverages = CodexTokenUsageActiveOutputAverages(
            perMinute: outputMinutes.isEmpty ? 0 : output / Double(outputMinutes.count),
            perHour: outputHours.isEmpty ? 0 : output / Double(outputHours.count),
            perDay: outputDays.isEmpty ? 0 : output / Double(outputDays.count)
        )

        return CodexTokenUsageReport(
            range: range,
            resolvedInterval: resolvedInterval,
            eventCount: eventCount,
            buckets: buckets,
            total: total,
            modelTotals: rows,
            activeOutputAverages: activeOutputAverages
        )
    }
}

private final class CodexTimestampParser {
    private lazy var fractionalFormatter: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()
    private lazy var wholeSecondFormatter: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter
    }()

    init() {
    }

    func parse(_ value: Any?) -> Date? {
        guard let string = value as? String, !string.isEmpty else { return nil }
        if let date = Self.fastUTCDate(from: string) {
            return date
        }
        return fractionalFormatter.date(from: string) ?? wholeSecondFormatter.date(from: string)
    }

    private static func fastUTCDate(from string: String) -> Date? {
        let bytes = Array(string.utf8)
        guard bytes.count >= 20,
              bytes[4] == 0x2d,
              bytes[7] == 0x2d,
              bytes[10] == 0x54,
              bytes[13] == 0x3a,
              bytes[16] == 0x3a,
              bytes.last == 0x5a,
              let year = decimal(bytes, 0, 4),
              let month = decimal(bytes, 5, 2),
              let day = decimal(bytes, 8, 2),
              let hour = decimal(bytes, 11, 2),
              let minute = decimal(bytes, 14, 2),
              let second = decimal(bytes, 17, 2)
        else { return nil }

        var components = tm()
        components.tm_year = Int32(year - 1900)
        components.tm_mon = Int32(month - 1)
        components.tm_mday = Int32(day)
        components.tm_hour = Int32(hour)
        components.tm_min = Int32(minute)
        components.tm_sec = Int32(second)
        let wholeSeconds = timegm(&components)
        guard wholeSeconds >= 0 else { return nil }

        var fraction = 0.0
        if bytes.count > 21, bytes[19] == 0x2e {
            var divisor = 10.0
            for byte in bytes[20..<(bytes.count - 1)] {
                guard byte >= 0x30, byte <= 0x39 else { return nil }
                fraction += Double(byte - 0x30) / divisor
                divisor *= 10
            }
        } else if bytes.count != 20 {
            return nil
        }
        return Date(timeIntervalSince1970: TimeInterval(wholeSeconds) + fraction)
    }

    private static func decimal(_ bytes: [UInt8], _ start: Int, _ count: Int) -> Int? {
        var value = 0
        for index in start..<(start + count) {
            let byte = bytes[index]
            guard byte >= 0x30, byte <= 0x39 else { return nil }
            value = value * 10 + Int(byte - 0x30)
        }
        return value
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

private final class ReverseUTF8LineReader {
    struct Line {
        let data: Data
        let offset: UInt64
    }

    private static let chunkSize = 256 * 1024
    private static let maxLinePrefixBytes = 512 * 1024
    private static let newline = Data([0x0a])

    private let handle: FileHandle
    private var cursor: UInt64
    private var chunk = Data()
    private var chunkStart: UInt64 = 0
    private var chunkIndex = 0
    private var fragments: [Data] = []
    private var fragmentBytes = 0
    private(set) var bytesRead: Int64 = 0

    init(url: URL) throws {
        handle = try FileHandle(forReadingFrom: url)
        cursor = try handle.seekToEnd()
    }

    func nextLine() throws -> Line? {
        while true {
            if chunkIndex > 0 {
                let searchRange = chunk.startIndex..<chunk.index(chunk.startIndex, offsetBy: chunkIndex)
                if let newlineRange = chunk.range(of: Self.newline, options: .backwards, in: searchRange) {
                    let newlineIndex = newlineRange.lowerBound
                    let contentStart = newlineRange.upperBound
                    let contentEnd = chunk.index(chunk.startIndex, offsetBy: chunkIndex)
                    let offset = chunkStart + UInt64(chunk.distance(from: chunk.startIndex, to: contentStart))
                    let line = assembleLine(prefix: Data(chunk[contentStart..<contentEnd]))
                    chunkIndex = chunk.distance(from: chunk.startIndex, to: newlineIndex)
                    if line.isEmpty { continue }
                    return Line(data: line, offset: offset)
                }

                appendFragment(Data(chunk[searchRange]))
                chunkIndex = 0
            }

            if cursor == 0 {
                guard !fragments.isEmpty else { return nil }
                let line = assembleLine(prefix: Data())
                if line.isEmpty { return nil }
                return Line(data: line, offset: 0)
            }

            let count = min(Self.chunkSize, Int(cursor))
            cursor -= UInt64(count)
            try handle.seek(toOffset: cursor)
            chunk = try handle.read(upToCount: count) ?? Data()
            chunkStart = cursor
            chunkIndex = chunk.count
            bytesRead += Int64(chunk.count)
        }
    }

    func close() {
        try? handle.close()
    }

    private func assembleLine(prefix: Data) -> Data {
        appendFragment(prefix)
        var line = Data(capacity: fragmentBytes)
        for fragment in fragments.reversed() {
            line.append(fragment)
        }
        fragments.removeAll(keepingCapacity: true)
        fragmentBytes = 0
        return line
    }

    private func appendFragment(_ fragment: Data) {
        guard !fragment.isEmpty else { return }
        fragments.append(fragment)
        fragmentBytes += fragment.count

        var overflow = fragmentBytes - Self.maxLinePrefixBytes
        while overflow > 0, let rightmost = fragments.first {
            if rightmost.count <= overflow {
                overflow -= rightmost.count
                fragmentBytes -= rightmost.count
                fragments.removeFirst()
            } else {
                let retainedCount = rightmost.count - overflow
                fragments[0] = Data(rightmost.prefix(retainedCount))
                fragmentBytes -= overflow
                overflow = 0
            }
        }
    }
}
