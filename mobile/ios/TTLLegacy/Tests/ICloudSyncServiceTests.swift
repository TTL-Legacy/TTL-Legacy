import XCTest
@testable import TTLLegacy

final class ICloudSyncServiceTests: XCTestCase {

    override func setUp() {
        super.setUp()
        // Reset state before each test
        ICloudSyncService.shared.isSyncEnabled = false
        UserDefaults.standard.removeObject(forKey: "com.ttllegacy.vault_associations")
    }

    func test_toggle_enablesSync() {
        ICloudSyncService.shared.isSyncEnabled = true
        XCTAssertTrue(ICloudSyncService.shared.isSyncEnabled)
    }

    func test_toggle_disablesSync() {
        ICloudSyncService.shared.isSyncEnabled = true
        ICloudSyncService.shared.isSyncEnabled = false
        XCTAssertFalse(ICloudSyncService.shared.isSyncEnabled)
    }

    func test_saveAndRetrieve_credentialForVault() {
        ICloudSyncService.shared.save(vaultID: "vault-abc", credentialID: "cred-xyz")
        XCTAssertEqual(ICloudSyncService.shared.credentialID(for: "vault-abc"), "cred-xyz")
    }

    func test_missingVault_returnsNil() {
        XCTAssertNil(ICloudSyncService.shared.credentialID(for: "nonexistent-\(UUID())"))
    }

    func test_restoreFromICloud_mergesIntoLocal() {
        // Pre-populate local storage
        ICloudSyncService.shared.save(vaultID: "local-vault", credentialID: "local-cred")
        // Enable sync so restore reads remote (will be empty in unit test, but merge must not wipe local)
        ICloudSyncService.shared.isSyncEnabled = true
        ICloudSyncService.shared.restoreFromICloud()
        // Local entry must survive the merge
        XCTAssertEqual(ICloudSyncService.shared.credentialID(for: "local-vault"), "local-cred")
    }

    func test_multipleAssociations_allRetrievable() {
        ICloudSyncService.shared.save(vaultID: "v1", credentialID: "c1")
        ICloudSyncService.shared.save(vaultID: "v2", credentialID: "c2")
        XCTAssertEqual(ICloudSyncService.shared.credentialID(for: "v1"), "c1")
        XCTAssertEqual(ICloudSyncService.shared.credentialID(for: "v2"), "c2")
    }

    func test_overwrite_updatesCredential() {
        ICloudSyncService.shared.save(vaultID: "v1", credentialID: "old-cred")
        ICloudSyncService.shared.save(vaultID: "v1", credentialID: "new-cred")
        XCTAssertEqual(ICloudSyncService.shared.credentialID(for: "v1"), "new-cred")
    }
}
