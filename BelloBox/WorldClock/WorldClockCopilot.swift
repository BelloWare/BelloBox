import Foundation

/// What the copilot knows about the planner when a question is sent. Built
/// from the same presentations the user sees, so the answer matches the screen.
struct WorldClockCopilotContext: Equatable {
    struct Location: Equatable {
        let zoneID: String
        let name: String
        /// Local date, time, and zone text at the selected instant.
        let localDescription: String
        let quality: MeetingTimeQuality
        let isReference: Bool
    }

    let selectedInstant: Date
    let referenceZoneID: String
    let locations: [Location]
    let now: Date
    let localZoneID: String
    let isFollowingNow: Bool

    var promptLines: [String] {
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime]
        var lines = [
            "- Current time: \(iso.string(from: now)) (user's local zone \(localZoneID))",
            "- Selected instant: \(iso.string(from: selectedInstant)) (\(isFollowingNow ? "live time" : "planning time"))",
            "- Reference location: \(WorldClockZoneCatalog.option(for: referenceZoneID).name) (\(referenceZoneID))",
            "- Locations at the selected instant:",
        ]
        for location in locations {
            let suffix = location.isReference ? " [reference]" : ""
            lines.append("  - \(location.name) (\(location.zoneID)): \(location.localDescription) — \(location.quality.label.lowercased())\(suffix)")
        }
        return lines
    }
}

/// An in-memory copy of a copilot conversation that travels from the palette
/// preview to the dedicated window on Enter. It is never written to disk.
struct WorldClockCopilotSnapshot: Equatable {
    var messages: [WorldClockCopilotSession.Message]
    var draft: String
    /// Only the parts that were actually applied travel, so a location
    /// proposal stays actionable in the window after the palette applied time.
    var appliedParts: [UUID: WorldClockCopilotPlan.Parts]
    var pendingQuestion: String?
    var outcome: WorldClockCopilotSession.Outcome

    var isEmpty: Bool {
        messages.isEmpty && draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

/// Everything the dedicated World Clock adopts when opened from a selection or
/// the palette: the previewed instant, the reference the user chose, and the
/// ephemeral copilot conversation. Adopting a handoff never saves preferences.
struct WorldClockHandoff: Equatable {
    var instant: Date?
    var anchorZoneID: String?
    var copilot: WorldClockCopilotSnapshot?

    init(instant: Date? = nil, anchorZoneID: String? = nil, copilot: WorldClockCopilotSnapshot? = nil) {
        self.instant = instant
        self.anchorZoneID = anchorZoneID
        self.copilot = copilot
    }
}

/// One ephemeral question-and-answer session. Transcripts live only in memory
/// for the window or palette that created them and are never persisted.
@MainActor
final class WorldClockCopilotSession: ObservableObject {
    struct Message: Identifiable, Equatable {
        enum Role: Equatable { case user, assistant }
        let id: UUID
        let role: Role
        let text: String
        var suggestion: WorldClockCopilotSuggestion?
        var issue: String?
    }

    enum Outcome: Equatable { case none, answered, failed(String), cancelled }

    typealias Responder = (WorldClockCopilotRequest, AIConfig) async throws -> WorldClockCopilotReply

    @Published private(set) var messages: [Message] = []
    @Published var draft = ""
    @Published private(set) var isBusy = false
    @Published private(set) var outcome: Outcome = .none
    @Published private(set) var appliedParts: [UUID: WorldClockCopilotPlan.Parts] = [:]
    @Published private(set) var statusMessage: String?

    var contextProvider: () -> WorldClockCopilotContext? = { nil }
    var configProvider: () -> AIConfig? = { nil }
    var isProviderConfigured: () -> Bool = { false }
    /// Called after every state change has settled, so hosts can read the
    /// new values synchronously (the palette uses it to resize once).
    var onStateChange: () -> Void = {}

    private let responder: Responder
    private var task: Task<Void, Never>?
    private var runID: UUID?
    private var pendingQuestion: String?

    static let transcriptLimit = 40

    init(responder: @escaping Responder) {
        self.responder = responder
    }

    deinit { task?.cancel() }

    var hasTranscript: Bool { !messages.isEmpty }
    /// Whether the transcript area has anything to show: messages, progress,
    /// an error, or a status line.
    var isTranscriptVisible: Bool { hasTranscript || isBusy || errorMessage != nil || statusMessage != nil }
    var canUseAI: Bool { isProviderConfigured() }
    var trimmedDraft: String { draft.trimmingCharacters(in: .whitespacesAndNewlines) }
    var canSend: Bool { !isBusy && canUseAI && !trimmedDraft.isEmpty }
    var canRetry: Bool {
        guard pendingQuestion != nil, !isBusy, canUseAI else { return false }
        switch outcome {
        case .failed, .cancelled: return true
        case .none, .answered: return false
        }
    }
    var errorMessage: String? {
        if case let .failed(message) = outcome { return message }
        return nil
    }

    /// Sends the draft. Sending is always an explicit user action, and the
    /// draft is only cleared once the question was accepted.
    func send() {
        let question = trimmedDraft
        guard !question.isEmpty else { return }
        if ask(question) { draft = "" }
    }

    /// Sends a specific question, such as a suggested prompt the user chose.
    /// Returns false when nothing was sent: busy, too long, or no provider.
    @discardableResult
    func ask(_ question: String) -> Bool {
        let question = question.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !question.isEmpty, !isBusy else { return false }
        guard question.count <= WorldClockCopilotRequest.questionLimit,
              question.utf8.count <= WorldClockCopilotRequest.questionByteLimit else {
            reject(WorldClockCopilotError.questionTooLong)
            return false
        }
        guard canUseAI else {
            reject(WorldClockCopilotError.providerNotConfigured)
            return false
        }
        let history = messages.map { WorldClockCopilotTurn(role: $0.role == .user ? .user : .assistant, text: $0.text) }
        append(Message(id: UUID(), role: .user, text: question))
        pendingQuestion = question
        return start(question: question, history: history)
    }

    /// Re-sends the last question after a failure or cancellation without
    /// duplicating it in the transcript.
    func retry() {
        guard canRetry, let question = pendingQuestion else { return }
        let history = messages.dropLast().map { WorldClockCopilotTurn(role: $0.role == .user ? .user : .assistant, text: $0.text) }
        start(question: question, history: Array(history))
    }

    func cancel() {
        guard isBusy else { return }
        runID = nil
        task?.cancel()
        task = nil
        isBusy = false
        outcome = .cancelled
        statusMessage = "Cancelled. Nothing was changed."
        onStateChange()
    }

    /// Forgets the conversation but keeps whatever is typed in the field.
    func clear() {
        cancel()
        messages = []
        appliedParts = [:]
        pendingQuestion = nil
        outcome = .none
        statusMessage = nil
        onStateChange()
    }

    /// Forgets everything, including the draft; used when the planner moves
    /// to a different selection so nothing from the old context lingers.
    func reset() {
        clear()
        draft = ""
    }

    /// Records which parts of a message's suggestion the user applied.
    func markApplied(_ messageID: UUID, parts: WorldClockCopilotPlan.Parts, summary: String) {
        appliedParts[messageID, default: []].formUnion(parts)
        statusMessage = "Applied: \(summary)"
        onStateChange()
    }

    func appliedParts(for message: Message) -> WorldClockCopilotPlan.Parts { appliedParts[message.id] ?? [] }

    /// Whether any part of the message's suggestion has been applied.
    func isApplied(_ message: Message) -> Bool { !appliedParts(for: message).isEmpty }

    /// Whether every part the message proposed has been applied.
    func isFullyApplied(_ message: Message) -> Bool {
        guard let suggestion = message.suggestion, !suggestion.parts.isEmpty else { return false }
        return appliedParts(for: message).isSuperset(of: suggestion.parts)
    }

    /// Copies the conversation for a handoff. A request still in flight cannot
    /// travel, so it is recorded as unanswered and can be asked again.
    func snapshot() -> WorldClockCopilotSnapshot {
        WorldClockCopilotSnapshot(
            messages: messages,
            draft: draft,
            appliedParts: appliedParts,
            pendingQuestion: pendingQuestion,
            outcome: isBusy ? .cancelled : outcome
        )
    }

    /// Replaces this session's conversation with a handoff. Any request in
    /// flight here is cancelled first.
    func restore(_ snapshot: WorldClockCopilotSnapshot) {
        reset()
        messages = Array(snapshot.messages.suffix(Self.transcriptLimit))
        draft = snapshot.draft
        appliedParts = snapshot.appliedParts.filter { entry in messages.contains { $0.id == entry.key } }
        pendingQuestion = snapshot.pendingQuestion
        outcome = snapshot.outcome
        switch snapshot.outcome {
        case .cancelled:
            statusMessage = snapshot.pendingQuestion == nil ? nil : "Not answered before the palette closed. Ask again to continue."
        case .none, .answered, .failed:
            statusMessage = nil
        }
        onStateChange()
    }

    @discardableResult
    private func start(question: String, history: [WorldClockCopilotTurn]) -> Bool {
        guard let context = contextProvider(), let config = configProvider() else {
            reject(WorldClockCopilotError.providerNotConfigured)
            return false
        }
        task?.cancel()
        let id = UUID()
        runID = id
        isBusy = true
        outcome = .none
        statusMessage = nil
        let request = WorldClockCopilotRequest(question: question, context: context, history: history)
        let responder = responder
        task = Task { [weak self] in
            do {
                let reply = try await responder(request, config)
                try Task.checkCancellation()
                guard let self, self.runID == id else { return }
                self.append(Message(id: UUID(), role: .assistant, text: reply.answer, suggestion: reply.suggestion, issue: reply.suggestionIssue))
                self.outcome = .answered
                self.pendingQuestion = nil
            } catch is CancellationError {
                // Superseded by cancel(), a newer question, or teardown.
            } catch {
                guard let self, self.runID == id else { return }
                self.outcome = .failed((error as? LocalizedError)?.errorDescription ?? error.localizedDescription)
            }
            guard let self, self.runID == id else { return }
            self.runID = nil
            self.task = nil
            self.isBusy = false
            self.onStateChange()
        }
        onStateChange()
        return true
    }

    /// A rejected question replaces any earlier retry intent, so Retry can
    /// never quietly resend an older, unrelated question.
    private func reject(_ error: WorldClockCopilotError) {
        pendingQuestion = nil
        outcome = .failed(error.localizedDescription)
        statusMessage = nil
        onStateChange()
    }

    private func append(_ message: Message) {
        messages.append(message)
        if messages.count > Self.transcriptLimit {
            messages.removeFirst(messages.count - Self.transcriptLimit)
        }
    }
}

#if DEBUG
/// Offline responders so reviewers can exercise the copilot without a provider
/// or network. Selected by BELLOBOX_E2E_WORLD_CLOCK_COPILOT=scripted|error|slow.
enum WorldClockCopilotFixture {
    static func responder(environment: [String: String] = ProcessInfo.processInfo.environment) -> WorldClockCopilotSession.Responder? {
        guard let mode = environment["BELLOBOX_E2E_WORLD_CLOCK_COPILOT"]?.lowercased() else { return nil }
        switch mode {
        case "scripted":
            return { request, _ in
                try await Task.sleep(nanoseconds: 700_000_000)
                return scriptedReply(for: request)
            }
        case "error":
            return { _, _ in
                try await Task.sleep(nanoseconds: 400_000_000)
                throw AIError.http(status: 503, message: "Scripted failure for review.")
            }
        case "slow":
            return { request, _ in
                try await Task.sleep(nanoseconds: 30_000_000_000)
                return scriptedReply(for: request)
            }
        default:
            return nil
        }
    }

    /// Canned answers only restate the context they were given; they never
    /// assert conclusions the fixture did not compute. Mentioning "Tokyo" or
    /// "add" proposes the location, "hour" or "later" proposes one hour later,
    /// and both together produce a mixed suggestion; anything else proposes
    /// one hour later.
    static func scriptedReply(for request: WorldClockCopilotRequest) -> WorldClockCopilotReply {
        let context = request.context
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime]
        let listing = context.locations
            .map { "\($0.name): \($0.localDescription), \($0.quality.label.lowercased())" }
            .joined(separator: "; ")
        let question = request.question.lowercased()
        let wantsTokyo = question.contains("tokyo") || question.contains("add")
        let wantsLater = question.contains("hour") || question.contains("later") || !wantsTokyo
        let proposed = context.selectedInstant.addingTimeInterval(3_600)
        let alreadyListed = context.locations.contains { $0.zoneID == "Asia/Tokyo" }

        var sentences: [String] = ["Offline fixture reply:"]
        if wantsLater {
            sentences.append("applying the time part moves the selected time one hour later, to \(iso.string(from: proposed)); every location shifts by the same hour, so check the cards for the new local times.")
        }
        if wantsTokyo {
            sentences.append(alreadyListed ? "Tokyo (Asia/Tokyo) is already one of your locations."
                                           : "Tokyo (Asia/Tokyo) can be added as a location; the palette hands that part to the World Clock window.")
        }
        sentences.append("Current locations at the selected time — \(listing).")
        return WorldClockCopilotReply(
            answer: sentences.joined(separator: " "),
            suggestion: WorldClockCopilotSuggestion(
                instant: wantsLater ? proposed : nil,
                zoneIDs: wantsTokyo ? ["Asia/Tokyo"] : [],
                replacesLocations: false,
                anchorZoneID: nil
            ),
            suggestionIssue: question.contains("mars") ? "Unsupported time zones were ignored: Mars/Olympus." : nil
        )
    }
}
#endif
