import XCTest
@testable import BelloBox

/// Launch must not read an API key: on a Mac where the Keychain item's ACL does
/// not trust this build, the read waits on a login-password prompt and the app
/// shows nothing until it is answered. The key is read when AI is used or
/// Settings shows it.
final class AppSettingsAPIKeyTests: XCTestCase {
    private final class FakeStore: APIKeyStore {
        var keys: [String: String] = [:]
        var reads: [String] = []
        var presenceChecks: [String] = []
        var writes: [(String, String)] = []
        func get(account: String) -> String? { reads.append(account); return keys[account] }
        func contains(account: String) -> Bool { presenceChecks.append(account); return keys[account] != nil }
        func set(_ value: String, account: String) -> Bool {
            writes.append((account, value))
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            keys[account] = trimmed.isEmpty ? nil : trimmed
            return true
        }
    }

    private var defaults: UserDefaults!
    private var suiteName = ""

    override func setUp() {
        super.setUp()
        suiteName = "BelloBoxTests.api-key-\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suiteName)
        super.tearDown()
    }

    private func settings(provider: ProviderKind, keys: [ProviderKind: String] = [:]) -> (AppSettings, FakeStore) {
        defaults.set(provider.rawValue, forKey: "provider")
        let store = FakeStore()
        for (kind, key) in keys { store.keys[KeychainStore.account(for: kind)] = key }
        return (AppSettings(defaults: defaults, apiKeyStore: store), store)
    }

    func testLaunchAndStatusDisplaysNeverReadTheKey() {
        for provider in ProviderKind.allCases {
            let (settings, store) = settings(provider: provider, keys: [.openAI: "sk-open", .anthropic: "sk-ant"])
            XCTAssertTrue(store.reads.isEmpty, "\(provider): creating settings read \(store.reads)")
            // What Home, the menu bar and the palette show at launch.
            XCTAssertTrue(settings.isConfigured, "\(provider)")
            _ = settings.generationModelName
            XCTAssertTrue(store.reads.isEmpty, "\(provider): showing the status read \(store.reads)")
        }
    }

    func testAnthropicStatusChecksOnlyThatAKeyExists() {
        let (missing, missingStore) = settings(provider: .anthropic)
        XCTAssertFalse(missing.isConfigured)
        XCTAssertEqual(missingStore.presenceChecks, [KeychainStore.account(for: .anthropic)])
        let (present, store) = settings(provider: .anthropic, keys: [.anthropic: "sk-ant"])
        XCTAssertTrue(present.isConfigured)
        XCTAssertTrue(present.isConfigured)
        XCTAssertEqual(store.presenceChecks, [KeychainStore.account(for: .anthropic)], "The answer is kept")
        XCTAssertTrue(store.reads.isEmpty)
    }

    func testUsingAIReadsTheKeyOnceAndKeepsIt() {
        let (settings, store) = settings(provider: .openAI, keys: [.openAI: "sk-open"])
        XCTAssertEqual(settings.currentConfig.apiKey, "sk-open")
        XCTAssertEqual(settings.apiKey, "sk-open")
        XCTAssertEqual(store.reads, [KeychainStore.account(for: .openAI)])
        XCTAssertTrue(store.writes.isEmpty, "Reading never writes")
    }

    func testEditingAndSwitchingProvidersStoreAndReadPerProvider() {
        let (settings, store) = settings(provider: .openAI, keys: [.anthropic: "sk-ant"])
        var changes = 0
        let observer = settings.objectWillChange.sink { changes += 1 }
        defer { observer.cancel() }
        settings.apiKey = "sk-new"
        XCTAssertGreaterThan(changes, 0, "Views showing the key update")
        XCTAssertEqual(store.writes.map(\.0), [KeychainStore.account(for: .openAI)])
        XCTAssertEqual(store.keys[KeychainStore.account(for: .openAI)], "sk-new")
        settings.apiKey = "sk-new"
        XCTAssertEqual(store.writes.count, 1, "An unchanged key is not rewritten")

        settings.providerKind = .anthropic
        XCTAssertTrue(settings.isConfigured)
        XCTAssertEqual(settings.apiKey, "sk-ant")
        XCTAssertEqual(settings.currentConfig.apiKey, "sk-ant")
        settings.apiKey = ""
        XCTAssertNil(store.keys[KeychainStore.account(for: .anthropic)], "Clearing deletes the item")
        XCTAssertFalse(settings.isConfigured)

        settings.providerKind = .openAI
        XCTAssertEqual(settings.apiKey, "sk-new")
        XCTAssertEqual(store.writes.count, 2)
    }
}
