import AppKit
import SwiftUI

struct WorkbenchResult {
    var text: String
    var status: String = "Ready"
    var comparison: ComparisonResult?
    var regex: RegexInspection?
    var table: DataTable?
    var url: URLInspection?
    var request: HTTPRequestDraft?
}

@MainActor
final class UtilityWorkbenchModel: ObservableObject {
    let command: LauncherCommand
    let selection: TextSelection
    let snippets: SnippetStore
    @Published var input: String { didSet { schedule() } }
    @Published var secondInput = "" { didSet { schedule() } }
    @Published var jsonMode = "Pretty-print" { didSet { schedule() } }
    @Published var comparisonMode = ComparisonMode.lines { didSet { schedule() } }
    @Published var ignoreWhitespace = false { didSet { schedule() } }
    @Published var regexPattern = "" { didSet { schedule() } }
    @Published var replacement = "" { didSet { schedule() } }
    @Published var regexIgnoreCase = false { didSet { schedule() } }
    @Published var regexMultiline = true { didSet { schedule() } }
    @Published var regexOutput = "Matches" { didSet { schedule() } }
    @Published var epochUnit = EpochUnit.automatic { didSet { schedule() } }
    @Published var zoneID = TimeZone.current.identifier { didSet { schedule() } }
    @Published var fromFormat = DataFormat.json { didSet { schedule() } }
    @Published var toFormat = DataFormat.yaml { didSet { schedule() } }
    @Published var inferTypes = false { didSet { schedule() } }
    @Published var delimiter = "," { didSet { schedule() } }
    @Published var generatorKind = GeneratorKind.uuid { didSet { schedule() } }
    @Published var generatorFormat = GeneratorFormat.lines { didSet { schedule() } }
    @Published var generatorCount = 5 { didSet { schedule() } }
    @Published var generatorLength = 24 { didSet { schedule() } }
    @Published var snippetName = ""
    @Published var snippetID: UUID?
    @Published var snippetValues: [String: String] = [:] { didSet { schedule() } }
    @Published var urlDraft: URLInspection? {
        didSet { if !busy { result = nil; message = "Choose Build URL to preview your edits."; error = nil } }
    }
    @Published var request = HTTPRequestDraft() {
        didSet { if !busy { result = nil; message = nil; error = nil } }
    }
    @Published private(set) var result: WorkbenchResult?
    @Published private(set) var error: String?
    @Published private(set) var message: String?
    @Published private(set) var busy = false
    @Published private(set) var sending = false
    private var task: Task<Void, Never>?
    private var runID = UUID()
    private let snippetUUID = UUID().uuidString.lowercased()
    var onReplace: (String) -> Void = { _ in }
    var pinnedText: () -> String? = { nil }
    var pinText: (String) -> Void = { _ in }

    init(command: LauncherCommand, selection: TextSelection, snippets: SnippetStore) {
        self.command = command; self.selection = selection; self.snippets = snippets
        input = selection.text
        if command == .snippets && input.isEmpty { input = "Hello {{name}},\n\n{{selection}}" }
        if command == .convert && !input.hasPrefix("{") && !input.hasPrefix("[") {
            fromFormat = input.contains(":") ? .yaml : .csv
            toFormat = .json
        }
    }
    var canReplace: Bool { selection.pid != nil && !output.isEmpty && !busy && error == nil }
    var output: String { result?.text ?? "" }
    var customFields: [String] { SnippetTemplate.placeholders(input).filter { !["selection", "date", "timestamp", "uuid"].contains($0) } }
    var canChain: Bool {
        [.json, .regex, .convert, .url].contains(command) && !output.isEmpty && output != input && !busy && error == nil
            && !(command == .json && jsonMode == "Validate") && !(command == .regex && regexOutput == "Matches")
    }
    func useOutputAsInput() {
        let text = output
        guard canChain else { return }
        if command == .convert { let previous = fromFormat; fromFormat = toFormat; toFormat = previous }
        input = text
    }

    func schedule() {
        task?.cancel()
        busy = true
        if command == .url { urlDraft = nil }
        if command == .http { request = HTTPRequestDraft() }
        runID = UUID()
        let id = runID
        result = nil; error = nil; message = nil; sending = false
        guard !input.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || command == .generate || (command == .compare && !secondInput.isEmpty) else { busy = false; return }
        let operation = makeOperation()
        busy = true
        task = Task { [weak self] in
            do {
                try await Task.sleep(nanoseconds: 220_000_000)
                let worker = Task.detached(priority: .userInitiated) { try operation() }
                let value = try await withTaskCancellationHandler { try await worker.value } onCancel: { worker.cancel() }
                guard let self, self.runID == id, !Task.isCancelled else { return }
                self.result = value
                if let url = value.url { self.urlDraft = url }
                if let request = value.request { self.request = request }
                self.busy = false
            } catch {
                guard let self, self.runID == id, !Task.isCancelled else { return }
                self.error = error.localizedDescription; self.busy = false
            }
        }
    }
    func cancel() {
        task?.cancel(); runID = UUID(); busy = false; sending = false
        message = "Cancelled."
    }
    func copyOutput() {
        guard !output.isEmpty, !busy, error == nil else { return }
        NSPasteboard.general.clearContents()
        message = NSPasteboard.general.setString(output, forType: .string) ? "Copied." : "Could not copy."
    }
    func pasteInput(second: Bool = false) {
        guard let text = NSPasteboard.general.string(forType: .string) else { message = "The clipboard has no text."; return }
        if second { secondInput = text } else { input = text }
    }
    func usePinned() {
        if let text = pinnedText() { secondInput = text } else { message = "Pin a selection first, then open Compare with another selection." }
    }
    func pin() { pinText(input); message = "Pinned for comparison until Bello Box quits." }
    func buildURL() {
        guard let draft = urlDraft else { return }
        do { result = WorkbenchResult(text: try draft.rebuilt(), status: "URL rebuilt"); error = nil; message = nil }
        catch { self.error = error.localizedDescription }
    }
    func loadSnippet(_ snippet: DeveloperSnippet) { snippetID = snippet.id; snippetName = snippet.name; snippetValues = [:]; input = snippet.template }
    func newSnippet() { snippetID = nil; snippetName = ""; snippetValues = [:]; input = "" }
    func saveSnippet() {
        do {
            let snippet = DeveloperSnippet(id: snippetID ?? UUID(), name: snippetName, template: input)
            try snippets.save(snippet)
            snippetID = snippet.id; message = "Snippet saved on this Mac."; error = nil
        } catch { self.error = error.localizedDescription }
    }
    func deleteSnippet() {
        guard let snippetID else { return }
        do { try snippets.remove(snippetID); newSnippet(); message = "Snippet deleted." }
        catch { self.error = error.localizedDescription }
    }
    func sendRequest() {
        task?.cancel(); runID = UUID()
        let id = runID, draft = request
        result = nil; error = nil; message = nil; busy = true; sending = true
        task = Task { [weak self] in
            do {
                let response = try await HTTPRequestTool.send(draft)
                guard let self, self.runID == id, !Task.isCancelled else { return }
                self.result = WorkbenchResult(text: response.text, status: response.status)
                self.busy = false; self.sending = false
            } catch {
                guard let self, self.runID == id, !Task.isCancelled else { return }
                self.error = error.localizedDescription; self.busy = false; self.sending = false
            }
        }
    }
    private func makeOperation() -> @Sendable () throws -> WorkbenchResult {
        let command = command, input = input, second = secondInput, jsonMode = jsonMode, mode = comparisonMode, ignore = ignoreWhitespace
        let pattern = regexPattern, replacement = replacement, insensitive = regexIgnoreCase, multiline = regexMultiline, regexOutput = regexOutput
        let unit = epochUnit, zoneID = zoneID, from = fromFormat, to = toFormat, infer = inferTypes, delimiter = delimiter
        let kind = generatorKind, count = generatorCount, length = generatorLength, format = generatorFormat
        let fields = snippetValues, selectionText = selection.text, uuid = snippetUUID
        return {
            try UtilityLimits.check(input)
            switch command {
            case .json:
                let value = try DeveloperJSON.parse(input)
                if jsonMode == "Validate" { return WorkbenchResult(text: "Valid JSON.\n\(input.utf8.count.formatted()) UTF-8 bytes · \(input.count.formatted()) characters", status: "Valid JSON") }
                return WorkbenchResult(text: value.formatted(pretty: jsonMode == "Pretty-print"), status: "Valid JSON · keys sorted · numbers preserved")
            case .compare:
                let result = try TextComparison.compare(input, second, mode: mode, ignoreWhitespace: ignore)
                return WorkbenchResult(text: result.text, status: "\(result.added) added · \(result.removed) removed", comparison: result)
            case .jwt: return WorkbenchResult(text: try JWTInspector.inspect(input), status: "Decoded locally · signature not verified")
            case .regex:
                guard !pattern.isEmpty else { return WorkbenchResult(text: "", status: "Enter a regular expression to see live matches.") }
                let match = try RegexTester.inspect(text: input, pattern: pattern, replacement: replacement, caseInsensitive: insensitive, multiline: multiline)
                let output = regexOutput == "Extract" ? match.extracted : regexOutput == "Replace" ? match.replaced : match.details
                return WorkbenchResult(text: output, status: "\(match.ranges.count) matches · ICU regular expressions", regex: match)
            case .url:
                let url = try URLInspection(input)
                return WorkbenchResult(text: try url.rebuilt(), status: "URL parsed · + stays a literal plus", url: url)
            case .time:
                guard let zone = TimeZone(identifier: zoneID) else { throw UtilityError("Choose a valid IANA time zone, for example Asia/Singapore.") }
                return WorkbenchResult(text: try DeveloperTime.summary(input, unit: unit, zone: zone, comparedWith: second), status: "Converted time")
            case .cron:
                guard let zone = TimeZone(identifier: zoneID) else { throw UtilityError("Choose a valid IANA time zone.") }
                let cron = try CronSchedule(input), dates = try cron.next(after: Date(), zone: zone)
                let formatter = DateFormatter(); formatter.dateFormat = "EEE, yyyy-MM-dd HH:mm:ss XXX"; formatter.timeZone = zone
                let runs = dates.isEmpty ? "No matching run in the next 8 years." : dates.map { formatter.string(from: $0) }.joined(separator: "\n")
                return WorkbenchResult(text: cron.explanation + "\n\nNEXT RUNS · \(zone.identifier)\n" + runs, status: "Standard 5-field cron · DST-aware")
            case .convert:
                let (text, table) = try DataConversion.convert(input, from: from, to: to, delimiter: delimiter.first ?? ",", inferTypes: infer)
                return WorkbenchResult(text: text, status: to == .csv ? "Null/missing values become empty cells; nested values use JSON" : "Converted \(from.rawValue) → \(to.rawValue)", table: table)
            case .snippets:
                return WorkbenchResult(text: SnippetTemplate.render(input, selection: selectionText, values: fields, uuid: uuid), status: "Template preview · date and timestamp use UTC")
            case .generate:
                return WorkbenchResult(text: try DeveloperGenerator.generate(kind: kind, count: count, length: length, format: format), status: "Generated \(count) items")
            case .http:
                var request = HTTPRequestDraft()
                if input.trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix("curl") { request = try CurlImporter.parse(input) }
                else { request.url = input.trimmingCharacters(in: .whitespacesAndNewlines); _ = try request.request() }
                return WorkbenchResult(text: "", status: "Request imported. Review the fields and choose Send.", request: request)
            default: return WorkbenchResult(text: "")
            }
        }
    }
}
