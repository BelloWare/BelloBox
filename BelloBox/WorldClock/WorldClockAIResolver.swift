import Foundation

/// A validated change the copilot proposes. Nothing here is applied until the
/// user chooses to; hosts turn it into a `WorldClockCopilotPlan` first.
struct WorldClockCopilotSuggestion: Equatable {
    var instant: Date?
    var zoneIDs: [String] = []
    var replacesLocations = false
    var anchorZoneID: String?

    var isEmpty: Bool { instant == nil && zoneIDs.isEmpty && anchorZoneID == nil }
    static let maximumZones = 12
}

/// The concrete edit that applying a suggestion would make to one planner.
struct WorldClockCopilotPlan: Equatable {
    /// Which parts of a suggestion a plan covers, and which parts a user has
    /// already applied. Completion is tracked per part so applying the time
    /// in the palette never hides a location proposal that still waits for
    /// the dedicated window.
    struct Parts: OptionSet, Hashable {
        let rawValue: Int
        static let time = Parts(rawValue: 1)
        static let locations = Parts(rawValue: 2)
    }

    var instant: Date?
    var zoneIDs: [String]?
    var anchorZoneID: String?
    var summary: String
    var parts: Parts = []
}

struct WorldClockCopilotReply: Equatable {
    let answer: String
    let suggestion: WorldClockCopilotSuggestion?
    /// Why part of the model's proposal was dropped, shown beside the answer.
    let suggestionIssue: String?
}

struct WorldClockCopilotTurn: Equatable {
    enum Role: String { case user = "User", assistant = "Assistant" }
    let role: Role
    let text: String
}

/// Everything sent for one question. Built by the host at send time so it
/// reflects the instant and locations the user is looking at.
struct WorldClockCopilotRequest: Equatable {
    let question: String
    let context: WorldClockCopilotContext
    let history: [WorldClockCopilotTurn]

    static let questionLimit = 2_000
    /// Grapheme counts alone let a long combining sequence through; bytes do not.
    static let questionByteLimit = 8_192
    static let historyTurns = 6
    static let historyTurnLimit = 600
}

enum WorldClockCopilotError: LocalizedError, Equatable {
    case emptyQuestion
    case questionTooLong
    case providerNotConfigured
    case emptyAnswer

    var errorDescription: String? {
        switch self {
        case .emptyQuestion: return "Ask a question about the selected time or locations first."
        case .questionTooLong: return "Keep questions under \(WorldClockCopilotRequest.questionLimit.formatted()) characters and 8 KB."
        case .providerNotConfigured: return "Connect an AI provider in Settings to use the copilot."
        case .emptyAnswer: return "The copilot returned an empty answer."
        }
    }
}

extension WorldClockCopilotSuggestion {
    /// Resolves the suggestion against the planner it would change. Returns nil
    /// when nothing would change. Location edits are only planned when the
    /// host allows them; the palette preview keeps saved locations untouched
    /// and hands location proposals to the dedicated window instead.
    func plan(
        currentZoneIDs: [String],
        currentAnchorZoneID: String,
        currentInstant: Date,
        allowsTimeChanges: Bool = true,
        allowsLocationChanges: Bool,
        describeInstant: (Date) -> String
    ) -> WorldClockCopilotPlan? {
        var plan = WorldClockCopilotPlan(summary: "")
        var parts: [String] = []

        if allowsTimeChanges, let instant, abs(instant.timeIntervalSince(currentInstant)) >= 60 {
            plan.instant = instant
            plan.parts.insert(.time)
            parts.append("Set time to \(describeInstant(instant))")
        }

        if allowsLocationChanges {
            var resulting = currentZoneIDs
            let valid = WorldClockZoneCatalog.validIdentifiers(zoneIDs)
            if replacesLocations, !valid.isEmpty {
                resulting = valid
            } else {
                resulting += valid.filter { !currentZoneIDs.contains($0) }
            }
            if resulting != currentZoneIDs {
                plan.zoneIDs = resulting
                let names = { (ids: [String]) in ids.map { WorldClockZoneCatalog.option(for: $0).name }.joined(separator: ", ") }
                if replacesLocations, !valid.isEmpty {
                    parts.append("Replace locations with \(names(valid))")
                } else {
                    parts.append("Add \(names(valid.filter { !currentZoneIDs.contains($0) }))")
                }
            }
            if let anchorZoneID, resulting.contains(anchorZoneID), anchorZoneID != currentAnchorZoneID {
                plan.anchorZoneID = anchorZoneID
                parts.append("Reference: \(WorldClockZoneCatalog.option(for: anchorZoneID).name)")
            } else if plan.zoneIDs != nil, !resulting.contains(currentAnchorZoneID), let first = resulting.first {
                plan.anchorZoneID = first
                parts.append("Reference: \(WorldClockZoneCatalog.option(for: first).name)")
            }
        }

        guard !parts.isEmpty else { return nil }
        if plan.zoneIDs != nil || plan.anchorZoneID != nil { plan.parts.insert(.locations) }
        plan.summary = parts.joined(separator: " · ")
        return plan
    }

    /// The parts this suggestion proposes at all, regardless of the planner.
    var parts: WorldClockCopilotPlan.Parts {
        var parts: WorldClockCopilotPlan.Parts = []
        if instant != nil { parts.insert(.time) }
        if !zoneIDs.isEmpty || anchorZoneID != nil { parts.insert(.locations) }
        return parts
    }
}

/// Reads the copilot's JSON envelope. Models that ignore the format still
/// produce a usable plain-text answer; only the suggestion requires JSON.
struct WorldClockCopilotResponseParser {
    private struct Payload: Decodable {
        struct Suggestion: Decodable {
            let referenceDate: String?
            let timeZoneIDs: [String]?
            let replaceLocations: Bool?
            let anchorTimeZoneID: String?
        }
        let answer: String?
        let suggestion: Suggestion?
    }

    static let answerLimit = 4_000
    /// The finite calendar range the planner supports: Gregorian years 1 through
    /// 9999 in UTC, matching what ISO 8601 timestamps can express. Historical
    /// and far-future instants inside it are legitimate planning inputs.
    static let supportedInstantRange: ClosedRange<Date> = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        let start = calendar.date(from: DateComponents(year: 1, month: 1, day: 1))!
        let end = calendar.date(from: DateComponents(year: 9999, month: 12, day: 31, hour: 23, minute: 59, second: 59))!
        return start...end
    }()

    static func isSupportedInstant(_ date: Date) -> Bool {
        date.timeIntervalSince1970.isFinite && supportedInstantRange.contains(date)
    }

    func parse(_ response: String) throws -> WorldClockCopilotReply {
        let trimmed = response.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { throw WorldClockCopilotError.emptyAnswer }

        guard let data = Self.jsonObjectData(in: trimmed),
              let payload = try? JSONDecoder().decode(Payload.self, from: data),
              payload.answer != nil || payload.suggestion != nil else {
            return WorldClockCopilotReply(answer: Self.bounded(trimmed), suggestion: nil, suggestionIssue: nil)
        }

        var issues: [String] = []
        var suggestion: WorldClockCopilotSuggestion?
        if let proposal = payload.suggestion {
            var candidate = WorldClockCopilotSuggestion()
            if let value = proposal.referenceDate?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty {
                if let date = Self.parseISO8601(value) {
                    if Self.isSupportedInstant(date) {
                        candidate.instant = date
                    } else {
                        issues.append("The suggested time \(Self.bounded(value, limit: 40)) is outside the supported calendar range (years 1–9999) and was ignored.")
                    }
                } else {
                    issues.append("The suggested time \"\(Self.bounded(value, limit: 40))\" could not be read.")
                }
            }
            let requested = (proposal.timeZoneIDs ?? []).map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.filter { !$0.isEmpty }
            let invalid = requested.filter { TimeZone(identifier: $0) == nil }
            if !invalid.isEmpty {
                issues.append("Unsupported time zones were ignored: \(invalid.prefix(4).map { Self.bounded($0, limit: 40) }.joined(separator: ", ")).")
            }
            candidate.zoneIDs = Array(WorldClockZoneCatalog.validIdentifiers(requested).prefix(WorldClockCopilotSuggestion.maximumZones))
            candidate.replacesLocations = proposal.replaceLocations ?? false
            if let anchor = proposal.anchorTimeZoneID?.trimmingCharacters(in: .whitespacesAndNewlines), !anchor.isEmpty {
                if TimeZone(identifier: anchor) != nil {
                    candidate.anchorZoneID = anchor
                } else {
                    issues.append("The suggested reference zone \"\(Self.bounded(anchor, limit: 40))\" is not supported.")
                }
            }
            if !candidate.isEmpty { suggestion = candidate }
        }

        let answer = Self.bounded(payload.answer?.trimmingCharacters(in: .whitespacesAndNewlines) ?? "")
        guard !answer.isEmpty || suggestion != nil else { throw WorldClockCopilotError.emptyAnswer }
        return WorldClockCopilotReply(
            answer: answer.isEmpty ? "Here is a change you can apply." : answer,
            suggestion: suggestion,
            suggestionIssue: issues.isEmpty ? nil : issues.joined(separator: " ")
        )
    }

    private static func jsonObjectData(in response: String) -> Data? {
        guard let start = response.firstIndex(of: "{"), let end = response.lastIndex(of: "}"), start <= end else {
            return nil
        }
        return String(response[start...end]).data(using: .utf8)
    }

    private static func bounded(_ text: String, limit: Int = answerLimit) -> String {
        let scalars = text.unicodeScalars
        guard scalars.count > limit else { return text }
        return String(String.UnicodeScalarView(scalars.prefix(limit))) + "…"
    }

    static func parseISO8601(_ value: String) -> Date? {
        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = fractional.date(from: value) { return date }

        let standard = ISO8601DateFormatter()
        standard.formatOptions = [.withInternetDateTime]
        return standard.date(from: value)
    }
}

/// Sends one copilot question through the configured provider. Prompts carry
/// only planner context and the conversation; nothing is logged or persisted.
final class WorldClockAIResolver: @unchecked Sendable {
    private let client: AIClient
    private let parser: WorldClockCopilotResponseParser

    init(client: AIClient = AIClient(), parser: WorldClockCopilotResponseParser = WorldClockCopilotResponseParser()) {
        self.client = client
        self.parser = parser
    }

    func ask(_ request: WorldClockCopilotRequest, config: AIConfig) async throws -> WorldClockCopilotReply {
        let question = request.question.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !question.isEmpty else { throw WorldClockCopilotError.emptyQuestion }
        guard question.count <= WorldClockCopilotRequest.questionLimit,
              question.utf8.count <= WorldClockCopilotRequest.questionByteLimit else { throw WorldClockCopilotError.questionTooLong }

        var requestConfig = config
        requestConfig.systemPrompt = Self.systemPrompt
        let response = try await client.complete(config: requestConfig, userText: Self.userPrompt(for: request))
        try Task.checkCancellation()
        return try parser.parse(response)
    }

    static let systemPrompt = """
    You are the World Clock copilot inside Bello Box, a macOS utility for comparing times and planning meetings across time zones. Treat the planner context as ground truth and do time-zone arithmetic carefully, including daylight-saving offsets and date changes. Working hours are 09:00-17:00 local time, fringe hours are 07:00-09:00 and 17:00-21:00, and everything else is night. Be concise: at most three short sentences or a short list. Never invent locations that are not in the context or the question.

    Reply with exactly one JSON object and nothing else, using this schema:
    {"answer":"plain-text answer","suggestion":{"referenceDate":"RFC3339 timestamp or null","timeZoneIDs":["IANA/Zone"],"replaceLocations":false,"anchorTimeZoneID":"IANA/Zone or null"}}
    Set "suggestion" to null unless the user asks to move the meeting time, pick a slot, or add, remove, or replace locations. Use canonical IANA time-zone identifiers and put referenceDate at the proposed instant. To add locations, list only the new zones with replaceLocations false. To remove a location or replace the list, set replaceLocations to true and list every zone that should remain, keeping all of the user's other locations. Do not use Markdown or code fences.
    """

    static func userPrompt(for request: WorldClockCopilotRequest) -> String {
        var lines = ["Planner context (ground truth):"]
        lines += request.context.promptLines
        let history = request.history.suffix(WorldClockCopilotRequest.historyTurns)
        if !history.isEmpty {
            lines.append("")
            lines.append("Conversation so far:")
            for turn in history {
                let text = turn.text.replacingOccurrences(of: "\n", with: " ")
                let scalars = text.unicodeScalars
                let bounded = scalars.count > WorldClockCopilotRequest.historyTurnLimit
                    ? String(String.UnicodeScalarView(scalars.prefix(WorldClockCopilotRequest.historyTurnLimit))) + "…"
                    : text
                lines.append("\(turn.role.rawValue): \(bounded)")
            }
        }
        lines.append("")
        lines.append("Question: \(request.question.trimmingCharacters(in: .whitespacesAndNewlines))")
        return lines.joined(separator: "\n")
    }
}
