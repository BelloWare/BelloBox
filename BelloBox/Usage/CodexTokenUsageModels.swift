import Foundation

struct CodexTokenUsage: Equatable, Hashable, Sendable {
    var inputTokens: Int
    var cachedInputTokens: Int
    var outputTokens: Int
    var reasoningOutputTokens: Int
    var totalTokens: Int

    static let zero = CodexTokenUsage()

    init(
        inputTokens: Int = 0,
        cachedInputTokens: Int = 0,
        outputTokens: Int = 0,
        reasoningOutputTokens: Int = 0,
        totalTokens: Int = 0
    ) {
        self.inputTokens = max(0, inputTokens)
        self.cachedInputTokens = max(0, cachedInputTokens)
        self.outputTokens = max(0, outputTokens)
        self.reasoningOutputTokens = max(0, reasoningOutputTokens)
        self.totalTokens = max(0, totalTokens)
    }

    var uncachedInputTokens: Int {
        max(0, inputTokens - cachedInputTokens)
    }

    var isZero: Bool {
        inputTokens == 0 &&
            cachedInputTokens == 0 &&
            outputTokens == 0 &&
            reasoningOutputTokens == 0 &&
            totalTokens == 0
    }

    static func fromDictionary(_ value: Any?) -> CodexTokenUsage {
        guard let dictionary = value as? [String: Any] else { return .zero }
        let input = integer(dictionary["input_tokens"])
        let output = integer(dictionary["output_tokens"])
        let total = integer(dictionary["total_tokens"])
        return CodexTokenUsage(
            inputTokens: input,
            cachedInputTokens: integer(dictionary["cached_input_tokens"]),
            outputTokens: output,
            reasoningOutputTokens: integer(dictionary["reasoning_output_tokens"]),
            totalTokens: total > 0 ? total : input + output
        )
    }

    func delta(from previous: CodexTokenUsage?) -> CodexTokenUsage {
        guard let previous else { return self }
        return CodexTokenUsage(
            inputTokens: delta(inputTokens, previous.inputTokens),
            cachedInputTokens: delta(cachedInputTokens, previous.cachedInputTokens),
            outputTokens: delta(outputTokens, previous.outputTokens),
            reasoningOutputTokens: delta(reasoningOutputTokens, previous.reasoningOutputTokens),
            totalTokens: delta(totalTokens, previous.totalTokens)
        )
    }

    func adding(_ other: CodexTokenUsage) -> CodexTokenUsage {
        CodexTokenUsage(
            inputTokens: inputTokens + other.inputTokens,
            cachedInputTokens: cachedInputTokens + other.cachedInputTokens,
            outputTokens: outputTokens + other.outputTokens,
            reasoningOutputTokens: reasoningOutputTokens + other.reasoningOutputTokens,
            totalTokens: totalTokens + other.totalTokens
        )
    }

    private func delta(_ current: Int, _ previous: Int) -> Int {
        current >= previous ? current - previous : current
    }

    private static func integer(_ value: Any?) -> Int {
        if let number = value as? NSNumber {
            return max(0, number.intValue)
        }
        if let string = value as? String, let integer = Int(string) {
            return max(0, integer)
        }
        return 0
    }
}

struct CodexTokenUsageEvent: Identifiable, Equatable, Sendable {
    let id: String
    let timestamp: Date
    let model: String?
    let turnID: String?
    let usage: CodexTokenUsage
    let sourcePath: String
    let lineNumber: Int

    var displayModel: String {
        CodexTokenUsageModel.unknownDisplayName(for: model)
    }
}

enum CodexTokenUsageModel {
    static let unknown = "Unknown model"

    static func normalized(_ value: String?) -> String? {
        guard let value = value?.trimmingCharacters(in: .whitespacesAndNewlines),
              !value.isEmpty
        else { return nil }
        return value
    }

    static func unknownDisplayName(for model: String?) -> String {
        normalized(model) ?? unknown
    }
}

enum CodexTokenUsageMetric: String, CaseIterable, Identifiable, Sendable {
    case total
    case output
    case input
    case uncachedInput
    case cachedInput
    case reasoningOutput

    var id: String { rawValue }

    var title: String {
        switch self {
        case .total: return "Total"
        case .output: return "Output"
        case .input: return "Input"
        case .uncachedInput: return "Uncached input"
        case .cachedInput: return "Cached input"
        case .reasoningOutput: return "Reasoning output"
        }
    }

    func value(in usage: CodexTokenUsage) -> Int {
        switch self {
        case .total: return usage.totalTokens
        case .output: return usage.outputTokens
        case .input: return usage.inputTokens
        case .uncachedInput: return usage.uncachedInputTokens
        case .cachedInput: return usage.cachedInputTokens
        case .reasoningOutput: return usage.reasoningOutputTokens
        }
    }
}

enum CodexTokenUsageRangePreset: String, CaseIterable, Identifiable, Sendable {
    case threeDays
    case sevenDays
    case thirtyDays

    var id: String { rawValue }

    var title: String {
        switch self {
        case .threeDays: return "3 days"
        case .sevenDays: return "7 days"
        case .thirtyDays: return "30 days"
        }
    }

    var duration: TimeInterval {
        switch self {
        case .threeDays: return 3 * 24 * 60 * 60
        case .sevenDays: return 7 * 24 * 60 * 60
        case .thirtyDays: return 30 * 24 * 60 * 60
        }
    }

    func dateInterval(endingAt end: Date) -> DateInterval {
        DateInterval(start: end.addingTimeInterval(-duration), end: end)
    }
}

enum CodexTokenUsageInterval: String, CaseIterable, Identifiable, Sendable {
    case automatic
    case minute
    case hour
    case day

    var id: String { rawValue }

    var title: String {
        switch self {
        case .automatic: return "Auto"
        case .minute: return "Minute"
        case .hour: return "Hour"
        case .day: return "Day"
        }
    }

    static func resolved(preference: CodexTokenUsageInterval, range: DateInterval) -> CodexTokenUsageInterval {
        guard preference == .automatic else { return preference }
        let duration = range.duration
        if duration <= 6 * 60 * 60 {
            return .minute
        }
        if duration <= 7 * 24 * 60 * 60 {
            return .hour
        }
        return .day
    }

    func bucketStart(for date: Date, calendar: Calendar) -> Date {
        switch self {
        case .automatic:
            return CodexTokenUsageInterval.resolved(
                preference: self,
                range: DateInterval(start: date, end: date)
            ).bucketStart(for: date, calendar: calendar)
        case .minute:
            return calendar.dateInterval(of: .minute, for: date)?.start ?? date
        case .hour:
            return calendar.dateInterval(of: .hour, for: date)?.start ?? date
        case .day:
            return calendar.startOfDay(for: date)
        }
    }

    func bucketEnd(after start: Date, calendar: Calendar) -> Date {
        switch self {
        case .automatic:
            return start
        case .minute:
            return calendar.date(byAdding: .minute, value: 1, to: start) ?? start.addingTimeInterval(60)
        case .hour:
            return calendar.date(byAdding: .hour, value: 1, to: start) ?? start.addingTimeInterval(60 * 60)
        case .day:
            return calendar.date(byAdding: .day, value: 1, to: start) ?? start.addingTimeInterval(24 * 60 * 60)
        }
    }
}

struct CodexTokenUsageBucket: Identifiable, Equatable, Sendable {
    let start: Date
    let end: Date
    let usage: CodexTokenUsage

    var id: Date { start }
}

struct CodexTokenUsageModelTotal: Identifiable, Equatable, Sendable {
    let modelName: String
    let usage: CodexTokenUsage

    var id: String { modelName }
}

struct CodexTokenUsageActiveOutputAverages: Equatable, Sendable {
    let perMinute: Double
    let perHour: Double
    let perDay: Double

    static let zero = CodexTokenUsageActiveOutputAverages(perMinute: 0, perHour: 0, perDay: 0)
}

struct CodexTokenUsageReport: Equatable, Sendable {
    let range: DateInterval
    let resolvedInterval: CodexTokenUsageInterval
    let eventCount: Int
    let buckets: [CodexTokenUsageBucket]
    let total: CodexTokenUsage
    let modelTotals: [CodexTokenUsageModelTotal]
    let activeOutputAverages: CodexTokenUsageActiveOutputAverages
}

struct CodexTokenUsageUnreadableFile: Equatable, Sendable {
    let path: String
    let message: String
}

struct CodexTokenUsageScanResult: Equatable, Sendable {
    let codexHome: URL
    let filesScanned: Int
    let bytesRead: Int64
    let candidateBytes: Int64
    let unreadableFiles: [CodexTokenUsageUnreadableFile]
    let events: [CodexTokenUsageEvent]

    static func empty(codexHome: URL = CodexTokenUsageStore.defaultCodexHome()) -> CodexTokenUsageScanResult {
        CodexTokenUsageScanResult(
            codexHome: codexHome,
            filesScanned: 0,
            bytesRead: 0,
            candidateBytes: 0,
            unreadableFiles: [],
            events: []
        )
    }
}
