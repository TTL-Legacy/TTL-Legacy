import Foundation

/// Manages optional iCloud sync for passkey-to-vault association metadata.
/// Only non-sensitive data (vault IDs, credential IDs) is synced — never private keys.
final class ICloudSyncService {
    static let shared = ICloudSyncService()
    private init() {}

    private let store = NSUbiquitousKeyValueStore.default
    private let enabledKey = "com.ttllegacy.icloud_sync_enabled"
    private let associationsKey = "com.ttllegacy.vault_associations"

    // MARK: - Toggle

    var isSyncEnabled: Bool {
        get { store.bool(forKey: enabledKey) }
        set {
            store.set(newValue, forKey: enabledKey)
            store.synchronize()
            if newValue { pushAssociations(loadLocalAssociations()) }
        }
    }

    // MARK: - Associations

    /// Save a vault-to-credential association locally; push to iCloud if sync is on.
    func save(vaultID: String, credentialID: String) {
        var assoc = loadLocalAssociations()
        assoc[vaultID] = credentialID
        persist(assoc)
        if isSyncEnabled { pushAssociations(assoc) }
    }

    /// Return the credential ID associated with a vault, checking iCloud first when sync is on.
    func credentialID(for vaultID: String) -> String? {
        if isSyncEnabled, let remote = remoteAssociations()[vaultID] { return remote }
        return loadLocalAssociations()[vaultID]
    }

    /// Pull associations from iCloud and merge into local storage.
    func restoreFromICloud() {
        guard isSyncEnabled else { return }
        let remote = remoteAssociations()
        var local = loadLocalAssociations()
        for (k, v) in remote { local[k] = v }
        persist(local)
    }

    // MARK: - Private helpers

    private func loadLocalAssociations() -> [String: String] {
        guard let data = UserDefaults.standard.data(forKey: associationsKey),
              let dict = try? JSONDecoder().decode([String: String].self, from: data) else { return [:] }
        return dict
    }

    private func persist(_ associations: [String: String]) {
        guard let data = try? JSONEncoder().encode(associations) else { return }
        UserDefaults.standard.set(data, forKey: associationsKey)
    }

    private func pushAssociations(_ associations: [String: String]) {
        guard let data = try? JSONEncoder().encode(associations) else { return }
        store.set(data, forKey: associationsKey)
        store.synchronize()
    }

    private func remoteAssociations() -> [String: String] {
        guard let data = store.data(forKey: associationsKey),
              let dict = try? JSONDecoder().decode([String: String].self, from: data) else { return [:] }
        return dict
    }
}
