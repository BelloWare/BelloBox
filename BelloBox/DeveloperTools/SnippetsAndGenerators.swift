import Foundation
import Security

struct DeveloperSnippet: Codable, Identifiable, Equatable {
    var id = UUID()
    var name: String
    var template: String
}
@MainActor
final class SnippetStore: ObservableObject {
    @Published private(set) var snippets: [DeveloperSnippet] = []
    private let url: URL
    private var loadFailure: String?
    init(url: URL? = nil) {
        self.url = url ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("BelloBox/Snippets.json")
        if FileManager.default.fileExists(atPath: self.url.path) {
            do {
                let data = try Data(contentsOf: self.url, options: .mappedIfSafe)
                guard data.count <= 5_000_000 else { throw UtilityError("The snippet library exceeds 5 MB.") }
                snippets = try JSONDecoder().decode([DeveloperSnippet].self, from: data)
            } catch { loadFailure = "The saved snippet library could not be read. It has been kept intact at \(self.url.path)." }
        }
    }
    func save(_ snippet: DeveloperSnippet) throws {
        guard !snippet.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, !snippet.template.isEmpty else { throw UtilityError("Give the snippet a name and some text.") }
        try UtilityLimits.check(snippet.template)
        var updated = snippets
        if let index = updated.firstIndex(where: { $0.id == snippet.id }) { updated[index] = snippet }
        else { updated.append(snippet) }
        try persist(updated)
    }
    func remove(_ id: UUID) throws { try persist(snippets.filter { $0.id != id }) }
    private func persist(_ values: [DeveloperSnippet]) throws {
        if let loadFailure { throw UtilityError(loadFailure) }
        let data = try JSONEncoder().encode(values)
        guard data.count <= 5_000_000 else { throw UtilityError("Keep your snippet library under 5 MB.") }
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: url, options: .atomic)
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
        snippets = values.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }
}
enum SnippetTemplate {
    static func placeholders(_ text: String) -> [String] {
        let regex = try! NSRegularExpression(pattern: #"\{\{([A-Za-z][A-Za-z0-9_ -]{0,59})\}\}"#)
        let ns = text as NSString
        var seen = Set<String>()
        return regex.matches(in: text, range: NSRange(location: 0, length: ns.length)).map { ns.substring(with: $0.range(at: 1)) }.filter { seen.insert($0).inserted }
    }
    static func render(_ text: String, selection: String, values: [String: String], now: Date = Date(), uuid: String = UUID().uuidString.lowercased()) -> String {
        let iso = ISO8601DateFormatter().string(from: now)
        var substitutions = values
        substitutions["selection"] = selection
        substitutions["date"] = String(iso.prefix(10))
        substitutions["timestamp"] = String(Int64(now.timeIntervalSince1970))
        substitutions["uuid"] = uuid
        // Replace matches in the original template only; inserted text is never evaluated again.
        let regex = try! NSRegularExpression(pattern: #"\{\{([A-Za-z][A-Za-z0-9_ -]{0,59})\}\}"#)
        let ns = text as NSString
        let result = NSMutableString(string: text)
        for match in regex.matches(in: text, range: NSRange(location: 0, length: ns.length)).reversed() {
            let key = ns.substring(with: match.range(at: 1))
            result.replaceCharacters(in: match.range, with: substitutions[key] ?? "{{\(key)}}")
        }
        return result as String
    }
}

enum GeneratorKind: String, CaseIterable { case uuid = "UUID", random = "Random string", timestamps = "Timestamps", records = "Sample records" }
enum GeneratorFormat: String, CaseIterable { case lines = "Lines", json = "JSON", csv = "CSV" }
enum DeveloperGenerator {
    static func randomString(length: Int) throws -> String {
        let alphabet = Array("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789")
        guard (1...256).contains(length) else { throw UtilityError("Choose a random-string length from 1 to 256.") }
        var output = ""
        while output.count < length {
            var bytes = [UInt8](repeating: 0, count: 64)
            guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else { throw UtilityError("Secure random generation failed. Try again.") }
            for byte in bytes where byte < 248 {
                output.append(alphabet[Int(byte) % alphabet.count])
                if output.count == length { break }
            }
        }
        return output
    }
    static func generate(kind: GeneratorKind, count: Int, length: Int, format: GeneratorFormat, now: Date = Date()) throws -> String {
        guard (1...1_000).contains(count) else { throw UtilityError("Generate between 1 and 1,000 items.") }
        let values: [DeveloperJSON] = try (0..<count).map { index in
            switch kind {
            case .uuid: return .string(UUID().uuidString.lowercased())
            case .random: return .string(try randomString(length: length))
            case .timestamps: return .number(String(Int64(now.timeIntervalSince1970) + Int64(index)))
            case .records: return .object(["id": .string(UUID().uuidString.lowercased()), "name": .string("Example \(index + 1)"), "email": .string("user\(index + 1)@example.com"), "active": .bool(index % 2 == 0)])
            }
        }
        switch format {
        case .lines: return values.map { $0.scalarText }.joined(separator: "\n")
        case .json: return DeveloperJSON.array(values).formatted()
        case .csv: return try CSVCodec.encode(.array(kind == .records ? values : values.map { .object(["value": $0]) }), delimiter: ",")
        }
    }
}
