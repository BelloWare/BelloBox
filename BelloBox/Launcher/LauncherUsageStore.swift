import Foundation

/// A coarse content category, never the selection, app, URL, or a fingerprint of it.
enum LauncherContentKind: String, CaseIterable {
    case empty, text, json, url, jwt, timestamp, cron, data, curl

    init(hasText: Bool, suggestions: [LauncherCommand]) {
        guard hasText else { self = .empty; return }
        switch suggestions.first {
        case .json: self = .json
        case .url: self = .url
        case .jwt: self = .jwt
        case .worldClock: self = .timestamp
        case .cron: self = .cron
        case .convert: self = .data
        case .http: self = .curl
        default: self = .text
        }
    }
}

/// Bounded, local learning from explicit tool opens. A few repeated choices can
/// outweigh the default interpretation, but never an explicit title search.
/// Thirty-day half-life lets an old preference gradually give way to a new one.
struct LauncherUsageStore {
    static let defaultsKey = "launcherContextUsageV1"
    static let maximumBonus = 1_400
    static let halfLife: TimeInterval = 30 * 24 * 60 * 60
    private static let maximumWeight: Double = 4
    private struct Use: Codable {
        var weight: Double
        var lastUsed: TimeInterval

        func decayed(at now: TimeInterval) -> Double {
            weight * pow(0.5, max(0, now - lastUsed) / LauncherUsageStore.halfLife)
        }
    }
    private typealias History = [String: [String: Use]]
    let defaults: UserDefaults

#if DEBUG
    // Native UI fixtures/tests sometimes construct a real launcher controller.
    // Keep their learning in a volatile domain, never in the person's history.
    private static let reviewDefaults = UserDefaults(suiteName: "BelloBox.UsageReview.\(UUID().uuidString)")!
    private var isReview: Bool {
        defaults === UserDefaults.standard && (
            ProcessInfo.processInfo.environment.keys.contains { $0.hasPrefix("BELLOBOX_E2E_") || $0 == "XCTestConfigurationFilePath" }
            || NSClassFromString("XCTestCase") != nil)
    }
#endif

    private var activeDefaults: UserDefaults {
#if DEBUG
        if isReview { return Self.reviewDefaults }
#endif
        return defaults
    }

    private func write(_ data: Data?) {
#if DEBUG
        if isReview {
            Self.reviewDefaults.setVolatileDomain(data.map { [Self.defaultsKey: $0] } ?? [:], forName: UserDefaults.argumentDomain)
            return
        }
#endif
        if let data { defaults.set(data, forKey: Self.defaultsKey) }
        else { defaults.removeObject(forKey: Self.defaultsKey) }
    }

    init(defaults: UserDefaults = .standard) { self.defaults = defaults }

    func scores(for kind: LauncherContentKind?, now: Date = Date()) -> [String: Int] {
        guard let kind, now.timeIntervalSince1970.isFinite else { return [:] }
        return load()[kind.rawValue, default: [:]].mapValues {
            min(Self.maximumBonus, Int(($0.decayed(at: now.timeIntervalSince1970) * 350).rounded()))
        }
    }

    func record(_ command: LauncherCommand, kind: LauncherContentKind?, now: Date = Date()) {
        guard let kind, Self.learns(command), now.timeIntervalSince1970.isFinite else { return }
        let time = now.timeIntervalSince1970
        var history = load()
        // Prune stale counters; the storage also has a fixed category × tool bound.
        for category in Array(history.keys) {
            history[category] = history[category]?.filter { $0.value.decayed(at: time) >= 0.05 }
            if history[category]?.isEmpty == true { history.removeValue(forKey: category) }
        }
        let old = history[kind.rawValue]?[command.id]?.decayed(at: time) ?? 0
        history[kind.rawValue, default: [:]][command.id] = Use(weight: min(Self.maximumWeight, old + 1), lastUsed: time)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        if let data = try? encoder.encode(history) { write(data) }
    }

    func reset() { write(nil) }

    private static func learns(_ command: LauncherCommand) -> Bool {
        command != .home && command != .settings
    }

    private func load() -> History {
        guard let data = activeDefaults.data(forKey: Self.defaultsKey), data.count <= 32_768,
              let history = try? JSONDecoder().decode(History.self, from: data) else { return [:] }
        var result: History = [:]
        for kind in LauncherContentKind.allCases {
            for command in LauncherCommand.allCases where Self.learns(command) {
                guard let use = history[kind.rawValue]?[command.id], use.weight.isFinite,
                      use.weight > 0, use.lastUsed.isFinite else { continue }
                result[kind.rawValue, default: [:]][command.id] = Use(
                    weight: min(Self.maximumWeight, use.weight), lastUsed: use.lastUsed)
            }
        }
        return result
    }
}
