import Foundation
import Security

/// Where provider API keys are kept. Tests substitute an in-memory store.
protocol APIKeyStore {
    func get(account: String) -> String?
    /// Whether a key exists, without reading it: an attribute-only query, so it
    /// never asks for the login password the way reading the secret can.
    func contains(account: String) -> Bool
    @discardableResult func set(_ value: String, account: String) -> Bool
}

struct KeychainAPIKeyStore: APIKeyStore {
    func get(account: String) -> String? { KeychainStore.get(account: account) }
    func contains(account: String) -> Bool { KeychainStore.contains(account: account) }
    func set(_ value: String, account: String) -> Bool { KeychainStore.set(value, account: account) }
}

/// Minimal Keychain wrapper for storing provider API keys as generic passwords.
enum KeychainStore {
    static let service = "com.ainoob.BelloBox"
    private static let queue = DispatchQueue(label: "BelloBox.KeychainStore")

    static func account(for kind: ProviderKind) -> String { "apiKey-\(kind.rawValue)" }

    @discardableResult
    static func set(_ value: String, account: String) -> Bool {
#if DEBUG
        if isDisabledForAutomatedTests { return true }
#endif
        return queue.sync {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: service,
                kSecAttrAccount as String: account,
            ]
            SecItemDelete(query as CFDictionary)

            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { return true } // deletion only

            var attributes = query
            attributes[kSecValueData as String] = Data(trimmed.utf8)
            attributes[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
            return SecItemAdd(attributes as CFDictionary, nil) == errSecSuccess
        }
    }

    static func get(account: String) -> String? {
#if DEBUG
        if isDisabledForAutomatedTests { return nil }
#endif
        return queue.sync {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: service,
                kSecAttrAccount as String: account,
                kSecReturnData as String: true,
                kSecMatchLimit as String: kSecMatchLimitOne,
            ]
            var result: AnyObject?
            guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
                  let data = result as? Data,
                  let string = String(data: data, encoding: .utf8)
            else { return nil }
            return string
        }
    }

    static func contains(account: String) -> Bool {
#if DEBUG
        if isDisabledForAutomatedTests { return false }
#endif
        return queue.sync {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: service,
                kSecAttrAccount as String: account,
                kSecMatchLimit as String: kSecMatchLimitOne,
            ]
            return SecItemCopyMatching(query as CFDictionary, nil) == errSecSuccess
        }
    }

#if DEBUG
    /// Instrumented test hosts have a different code signature and can block on
    /// the user's API-key ACL before XCTest starts. Tests opt into an empty,
    /// non-persistent keychain with this environment variable.
    private static var isDisabledForAutomatedTests: Bool {
        let environment = ProcessInfo.processInfo.environment
        return environment["BELLOBOX_DISABLE_KEYCHAIN"] == "1"
            || environment.keys.contains("XCTestConfigurationFilePath")
    }
#endif
}
