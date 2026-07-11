import XCTest
@testable import BelloBox

final class CodexTokenUsageTests: XCTestCase {
    @MainActor
    func testDashboardViewModelLoadsLocalCodexHomeAndZoomsRange() async throws {
        let home = try localCodexHomeWithTwoEvents()
        let now = try date("2026-07-03T00:00:00Z")
        let viewModel = CodexTokenUsageViewModel(
            store: CodexTokenUsageStore(codexHome: home),
            nowProvider: { now }
        )

        await viewModel.reload()
        viewModel.selectedInterval = .hour
        viewModel.selectedMetric = .output
        viewModel.selectedModelName = "gpt-5"

        XCTAssertFalse(viewModel.isLoading)
        XCTAssertNil(viewModel.errorMessage)
        XCTAssertEqual(viewModel.scanResult.filesScanned, 1)
        XCTAssertEqual(viewModel.availableModelNames, ["gpt-5"])
        XCTAssertEqual(viewModel.report.total.outputTokens, 70)
        XCTAssertEqual(viewModel.report.buckets.count, 2)

        viewModel.zoom(to: DateInterval(start: try date("2026-07-01T00:00:00Z"), end: try date("2026-07-01T00:30:00Z")))
        XCTAssertEqual(viewModel.zoomStack.count, 1)
        XCTAssertEqual(viewModel.report.total.outputTokens, 20)

        viewModel.goBack()
        XCTAssertTrue(viewModel.zoomStack.isEmpty)
        XCTAssertEqual(viewModel.report.total.outputTokens, 70)
    }

    func testParseUsageEventsUsesCumulativeDeltasAndModelContext() throws {
        let file = try temporaryJSONL(
            name: "session.jsonl",
            lines: [
                #"{"type":"session_meta","timestamp":"2026-07-01T00:00:00Z","payload":{"id":"session-a"}}"#,
                #"{"type":"turn_context","timestamp":"2026-07-01T00:01:00Z","payload":{"model":"gpt-5","turn_id":"turn-a"}}"#,
                tokenCount(timestamp: "2026-07-01T00:02:00Z", input: 100, cached: 40, output: 20, reasoning: 5, total: 120),
                tokenCount(timestamp: "2026-07-01T00:03:00Z", input: 100, cached: 40, output: 20, reasoning: 5, total: 120),
                tokenCount(timestamp: "2026-07-01T00:04:00Z", input: 150, cached: 50, output: 45, reasoning: 8, total: 195),
                #"{"type":"turn_context","timestamp":"2026-07-01T00:05:00Z","payload":{"model":"codex-mini","turn_id":"turn-b"}}"#,
                tokenCount(timestamp: "2026-07-01T00:06:00Z", input: 10, cached: 1, output: 3, reasoning: 0, total: 13),
            ]
        )

        let events = try CodexTokenUsageStore.parseUsageEvents(in: file)

        XCTAssertEqual(events.count, 3)
        XCTAssertEqual(events[0].model, "gpt-5")
        XCTAssertEqual(events[0].turnID, "turn-a")
        XCTAssertEqual(events[0].usage.inputTokens, 100)
        XCTAssertEqual(events[0].usage.cachedInputTokens, 40)
        XCTAssertEqual(events[0].usage.uncachedInputTokens, 60)
        XCTAssertEqual(events[0].usage.outputTokens, 20)
        XCTAssertEqual(events[0].usage.reasoningOutputTokens, 5)
        XCTAssertEqual(events[0].usage.totalTokens, 120)

        XCTAssertEqual(events[1].usage.inputTokens, 50)
        XCTAssertEqual(events[1].usage.cachedInputTokens, 10)
        XCTAssertEqual(events[1].usage.outputTokens, 25)
        XCTAssertEqual(events[1].usage.reasoningOutputTokens, 3)
        XCTAssertEqual(events[1].usage.totalTokens, 75)

        XCTAssertEqual(events[2].model, "codex-mini")
        XCTAssertEqual(events[2].turnID, "turn-b")
        XCTAssertEqual(events[2].usage.inputTokens, 10)
        XCTAssertEqual(events[2].usage.cachedInputTokens, 1)
        XCTAssertEqual(events[2].usage.outputTokens, 3)
        XCTAssertEqual(events[2].usage.totalTokens, 13)
    }

    func testStoreScansSessionsAndArchivedSessions() throws {
        let home = try temporaryDirectory()
        let sessions = home.appendingPathComponent("sessions", isDirectory: true)
        let archived = home.appendingPathComponent("archived_sessions", isDirectory: true)
        try FileManager.default.createDirectory(at: sessions, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: archived, withIntermediateDirectories: true)
        try writeJSONL(
            [tokenCount(timestamp: "2026-07-01T00:00:00Z", input: 10, cached: 2, output: 4, reasoning: 1, total: 14)],
            to: sessions.appendingPathComponent("a.jsonl")
        )
        try writeJSONL(
            [tokenCount(timestamp: "2026-07-02T00:00:00Z", input: 30, cached: 10, output: 6, reasoning: 0, total: 36)],
            to: archived.appendingPathComponent("b.jsonl")
        )

        let result = try CodexTokenUsageStore(codexHome: home).loadEvents()

        XCTAssertEqual(result.filesScanned, 2)
        XCTAssertEqual(result.unreadableFiles, [])
        XCTAssertEqual(result.events.map(\.usage.totalTokens), [14, 36])
    }

    func testAggregatorBucketsAndKeepsUnknownModelSeparate() throws {
        let known = CodexTokenUsageEvent(
            id: "known",
            timestamp: try date("2026-07-01T00:15:00Z"),
            model: "gpt-5",
            turnID: nil,
            usage: CodexTokenUsage(inputTokens: 100, cachedInputTokens: 25, outputTokens: 40, reasoningOutputTokens: 10, totalTokens: 140),
            sourcePath: "/tmp/a.jsonl",
            lineNumber: 1
        )
        let unknown = CodexTokenUsageEvent(
            id: "unknown",
            timestamp: try date("2026-07-01T00:45:00Z"),
            model: nil,
            turnID: nil,
            usage: CodexTokenUsage(inputTokens: 20, cachedInputTokens: 0, outputTokens: 5, reasoningOutputTokens: 0, totalTokens: 25),
            sourcePath: "/tmp/b.jsonl",
            lineNumber: 1
        )
        let range = DateInterval(start: try date("2026-07-01T00:00:00Z"), end: try date("2026-07-01T02:00:00Z"))

        let report = CodexTokenUsageAggregator.report(
            events: [unknown, known],
            range: range,
            intervalPreference: .hour,
            modelFilter: nil,
            calendar: utcCalendar()
        )

        XCTAssertEqual(report.buckets.count, 1)
        XCTAssertEqual(report.total.inputTokens, 120)
        XCTAssertEqual(report.total.cachedInputTokens, 25)
        XCTAssertEqual(report.total.uncachedInputTokens, 95)
        XCTAssertEqual(report.total.outputTokens, 45)
        XCTAssertEqual(report.total.reasoningOutputTokens, 10)
        XCTAssertEqual(report.total.totalTokens, 165)
        XCTAssertEqual(report.modelTotals.map(\.modelName).sorted(), ["Unknown model", "gpt-5"])

        let unknownOnly = CodexTokenUsageAggregator.report(
            events: [unknown, known],
            range: range,
            intervalPreference: .hour,
            modelFilter: "Unknown model",
            calendar: utcCalendar()
        )
        XCTAssertEqual(unknownOnly.total.totalTokens, 25)
        XCTAssertEqual(unknownOnly.modelTotals.map(\.modelName), ["Unknown model"])
    }

    func testEndToEndLocalCodexHomeReport() throws {
        let home = try localCodexHomeWithTwoEvents()

        let result = try CodexTokenUsageStore(codexHome: home).loadEvents()
        let report = CodexTokenUsageAggregator.report(
            events: result.events,
            range: DateInterval(start: try date("2026-07-01T00:00:00Z"), end: try date("2026-07-01T03:00:00Z")),
            intervalPreference: .hour,
            modelFilter: "gpt-5",
            calendar: utcCalendar()
        )

        XCTAssertEqual(result.filesScanned, 1)
        XCTAssertEqual(result.events.count, 2)
        XCTAssertEqual(report.buckets.count, 2)
        XCTAssertEqual(report.total.inputTokens, 180)
        XCTAssertEqual(report.total.cachedInputTokens, 80)
        XCTAssertEqual(report.total.uncachedInputTokens, 100)
        XCTAssertEqual(report.total.outputTokens, 70)
        XCTAssertEqual(report.total.totalTokens, 250)
    }

    private func localCodexHomeWithTwoEvents() throws -> URL {
        let home = try temporaryDirectory()
        let nested = home
            .appendingPathComponent("sessions", isDirectory: true)
            .appendingPathComponent("2026", isDirectory: true)
            .appendingPathComponent("07", isDirectory: true)
        try FileManager.default.createDirectory(at: nested, withIntermediateDirectories: true)
        try writeJSONL(
            [
                #"{"type":"turn_context","timestamp":"2026-07-01T00:01:00Z","payload":{"model":"gpt-5","turn_id":"turn-a"}}"#,
                tokenCount(timestamp: "2026-07-01T00:02:00Z", input: 100, cached: 50, output: 20, reasoning: 4, total: 120),
                tokenCount(timestamp: "2026-07-01T01:02:00Z", input: 180, cached: 80, output: 70, reasoning: 12, total: 250),
            ],
            to: nested.appendingPathComponent("rollout.jsonl")
        )
        return home
    }

    private func tokenCount(
        timestamp: String,
        input: Int,
        cached: Int,
        output: Int,
        reasoning: Int,
        total: Int
    ) -> String {
        """
        {"type":"event_msg","timestamp":"\(timestamp)","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":\(input),"cached_input_tokens":\(cached),"output_tokens":\(output),"reasoning_output_tokens":\(reasoning),"total_tokens":\(total)}}}}
        """
    }

    private func temporaryJSONL(name: String, lines: [String]) throws -> URL {
        let directory = try temporaryDirectory()
        let file = directory.appendingPathComponent(name)
        try writeJSONL(lines, to: file)
        return file
    }

    private func writeJSONL(_ lines: [String], to url: URL) throws {
        let payload = lines.joined(separator: "\n") + "\n"
        try payload.write(to: url, atomically: true, encoding: .utf8)
    }

    private func temporaryDirectory() throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("BelloBoxCodexUsageTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func date(_ string: String) throws -> Date {
        let formatter = ISO8601DateFormatter()
        guard let date = formatter.date(from: string) else {
            throw NSError(domain: "CodexTokenUsageTests", code: 1)
        }
        return date
    }

    private func utcCalendar() -> Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }
}
