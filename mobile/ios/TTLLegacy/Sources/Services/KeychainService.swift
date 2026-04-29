import Security
import Foundation

final class KeychainService {
    static let shared = KeychainService()
    private init() {}

    private let tokenKey = "com.ttllegacy.auth_token"
    private let credentialKey = "com.ttllegacy.passkey_credential"

    func saveToken(_ token: String) {
        save(token, forKey: tokenKey)
    }

    func loadToken() -> String? {
        load(forKey: tokenKey)
    }

    func deleteToken() {
        delete(forKey: tokenKey)
    }

    func saveCredentialID(_ id: String) {
        save(id, forKey: credentialKey)
    }

    func loadCredentialID() -> String? {
        load(forKey: credentialKey)
    }

    private func save(_ value: String, forKey key: String) {
        let data = Data(value.utf8)
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrAccount: key,
            kSecValueData: data,
            kSecAttrAccessible: kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        ]
        SecItemDelete(query as CFDictionary)
        SecItemAdd(query as CFDictionary, nil)
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
        let query: [CFString: Any] = [kSecClass: kSecClassGenericPassword, kSecAttrAccount: key]
        SecItemDelete(query as CFDictionary)
    }
}
