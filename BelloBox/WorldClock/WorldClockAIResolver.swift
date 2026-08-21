import Foundation

struct WorldClockAIResult: Equatable {
    let timeZoneIDs: [String]
    let referenceDate: Date?
    let anchorTimeZoneID: String
}

enum WorldClockAIError: LocalizedError, Equatable {
    case emptyRequest
    case malformedResponse
    case noTimeZones
    case invalidTimeZones([String])
    case invalidReferenceDate(String)

    var errorDescription: String? {
        switch self {
        case .emptyRequest:
            return "Describe the locations or meeting time first."
        case .malformedResponse:
            return "The AI response did not contain valid world-clock data."
        case .noTimeZones:
            return "The AI response did not include any time zones."
        case let .invalidTimeZones(identifiers):
            return "The AI returned unsupported time zones: \(identifiers.joined(separator: ", "))."
        case let .invalidReferenceDate(value):
            return "The AI returned an invalid date: \(value)."
        }
    }
}

struct WorldClockAIResponseParser {
    private struct Payload: Decodable {
        let timeZoneIDs: [String]
        let referenceDate: String?
        let anchorTimeZoneID: String?
    }

    func parse(_ response: String) throws -> WorldClockAIResult {
        guard let objectData = jsonObjectData(in: response) else {
            throw WorldClockAIError.malformedResponse
        }

        let payload: Payload
        do {
            payload = try JSONDecoder().decode(Payload.self, from: objectData)
        } catch {
            throw WorldClockAIError.malformedResponse
        }

        guard !payload.timeZoneIDs.isEmpty else { throw WorldClockAIError.noTimeZones }
        let validIDs = WorldClockZoneCatalog.validIdentifiers(payload.timeZoneIDs)
        let invalidIDs = payload.timeZoneIDs.filter { TimeZone(identifier: $0) == nil }
        guard invalidIDs.isEmpty else { throw WorldClockAIError.invalidTimeZones(invalidIDs) }
        guard !validIDs.isEmpty else { throw WorldClockAIError.noTimeZones }

        let referenceDate: Date?
        if let value = payload.referenceDate?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty {
            guard let date = parseISO8601(value) else {
                throw WorldClockAIError.invalidReferenceDate(value)
            }
            referenceDate = date
        } else {
            referenceDate = nil
        }

        let requestedAnchor = payload.anchorTimeZoneID ?? validIDs[0]
        let anchor = validIDs.contains(requestedAnchor) ? requestedAnchor : validIDs[0]
        return WorldClockAIResult(
            timeZoneIDs: validIDs,
            referenceDate: referenceDate,
            anchorTimeZoneID: anchor
        )
    }

    private func jsonObjectData(in response: String) -> Data? {
        guard let start = response.firstIndex(of: "{"), let end = response.lastIndex(of: "}"), start <= end else {
            return nil
        }
        return String(response[start...end]).data(using: .utf8)
    }

    private func parseISO8601(_ value: String) -> Date? {
        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = fractional.date(from: value) { return date }

        let standard = ISO8601DateFormatter()
        standard.formatOptions = [.withInternetDateTime]
        return standard.date(from: value)
    }
}

final class WorldClockAIResolver: @unchecked Sendable {
    private let client: AIClient
    private let parser: WorldClockAIResponseParser

    init(client: AIClient = AIClient(), parser: WorldClockAIResponseParser = WorldClockAIResponseParser()) {
        self.client = client
        self.parser = parser
    }

    func resolve(
        request: String,
        config: AIConfig,
        now: Date = Date(),
        localTimeZone: TimeZone = .current
    ) async throws -> WorldClockAIResult {
        let trimmed = request.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { throw WorldClockAIError.emptyRequest }

        var requestConfig = config
        requestConfig.systemPrompt = Self.systemPrompt
        let response = try await client.complete(
            config: requestConfig,
            userText: Self.userPrompt(
                request: trimmed,
                now: now,
                localTimeZone: localTimeZone
            )
        )
        return try parser.parse(response)
    }

    private static let systemPrompt = """
    Convert a meeting-planning request into strict JSON. Return one JSON object and nothing else, with this schema:
    {"timeZoneIDs":["IANA/Zone"],"referenceDate":"RFC3339 timestamp or null","anchorTimeZoneID":"IANA/Zone"}

    Use canonical IANA time-zone identifiers. Include each location once, in the user's order. Resolve common city and area names to their actual IANA zones. The anchor must be one of timeZoneIDs. If the request has no date or time, set referenceDate to null. Never include Markdown.
    """

    private static func userPrompt(request: String, now: Date, localTimeZone: TimeZone) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        formatter.timeZone = localTimeZone
        return """
        Current date and time: \(formatter.string(from: now))
        User's local time zone: \(localTimeZone.identifier)
        Request: \(request)
        """
    }
}
