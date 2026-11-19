import Security
import Foundation

final class KeychainService {
    static let shared = KeychainService()
    private init() {}

    /// Secrets must never leave this device: no iCloud Keychain sync and no
    /// restore from backups onto another device.
    static let accessibility = kSecAttrAccessibleWhenUnlockedThisDeviceOnly

    enum Key {
        static let token = "com.ttllegacy.auth_token"
        static let credentialID = "com.ttllegacy.passkey_credential"
    }

    func saveToken(_ token: String) {
        save(token, forKey: Key.token)
    }

    func loadToken() -> String? {
        load(forKey: Key.token)
    }

    func deleteToken() {
        delete(forKey: Key.token)
    }

    func saveCredentialID(_ id: String) {
        save(id, forKey: Key.credentialID)
    }

    func loadCredentialID() -> String? {
        load(forKey: Key.credentialID)
    }

    // MARK: - Queries

    /// Identifies an item regardless of its value, accessibility, or sync state,
    /// so deletes also purge legacy items stored with weaker attributes.
    static func identityQuery(forKey key: String) -> [CFString: Any] {
        [
            kSecClass: kSecClassGenericPassword,
            kSecAttrAccount: key,
            kSecAttrSynchronizable: kSecAttrSynchronizableAny
        ]
    }

    static func addQuery(forKey key: String, data: Data) -> [CFString: Any] {
        [
            kSecClass: kSecClassGenericPassword,
            kSecAttrAccount: key,
            kSecValueData: data,
            kSecAttrAccessible: accessibility,
            kSecAttrSynchronizable: false
        ]
    }

    /// Stored attributes for `key`, or nil if absent. Used to verify accessibility.
    func attributes(forKey key: String) -> [String: Any]? {
        var query = Self.identityQuery(forKey: key)
        query[kSecReturnAttributes] = true
        query[kSecMatchLimit] = kSecMatchLimitOne
        var result: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess else { return nil }
        return result as? [String: Any]
    }

    // MARK: - Storage

    private func save(_ value: String, forKey key: String) {
        // Delete by identity first so an existing item with different
        // accessibility is replaced rather than causing errSecDuplicateItem.
        SecItemDelete(Self.identityQuery(forKey: key) as CFDictionary)
        SecItemAdd(Self.addQuery(forKey: key, data: Data(value.utf8)) as CFDictionary, nil)
    }

    private func load(forKey key: String) -> String? {
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrAccount: key,
            kSecReturnData: true,
            kSecMatchLimit: kSecMatchLimitOne
        ]
        var result: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
              let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private func delete(forKey key: String) {
        SecItemDelete(Self.identityQuery(forKey: key) as CFDictionary)
    }
}
