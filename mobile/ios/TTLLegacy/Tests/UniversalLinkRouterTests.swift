import XCTest
@testable import TTLLegacy

final class UniversalLinkRouterTests: XCTestCase {

    let router = UniversalLinkRouter.shared

    // MARK: - Valid Vault Invitation Links

    func test_parse_vaultInvitationLink_valid_returnsVaultInvitation() {
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-abc-123/invite")!
        let result = router.parse(url: url)
        guard case .vaultInvitation(let vaultID) = result else {
            XCTFail("Expected vaultInvitation, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-abc-123")
    }

    func test_parse_vaultInvitationLink_hyphenatedVaultID_returnsVaultInvitation() {
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-with-many-dashes/invite")!
        let result = router.parse(url: url)
        guard case .vaultInvitation(let vaultID) = result else {
            XCTFail("Expected vaultInvitation, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-with-many-dashes")
    }

    func test_parse_vaultInvitationLink_UUID_returnsVaultInvitation() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000"
        let url = URL(string: "https://ttl-legacy.app/vaults/\(uuid)/invite")!
        let result = router.parse(url: url)
        guard case .vaultInvitation(let vaultID) = result else {
            XCTFail("Expected vaultInvitation, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, uuid)
    }

    // MARK: - Valid Beneficiary Acceptance Links

    func test_parse_beneficiaryAcceptanceLink_withToken_returnsBeneficiaryAcceptance() {
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-xyz/accept?token=abc-token-123")!
        let result = router.parse(url: url)
        guard case .beneficiaryAcceptance(let vaultID, let token) = result else {
            XCTFail("Expected beneficiaryAcceptance, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-xyz")
        XCTAssertEqual(token, "abc-token-123")
    }

    func test_parse_beneficiaryAcceptanceLink_missingToken_returnsBeneficiaryAcceptanceWithEmptyToken() {
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-123/accept")!
        let result = router.parse(url: url)
        guard case .beneficiaryAcceptance(let vaultID, let token) = result else {
            XCTFail("Expected beneficiaryAcceptance, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-123")
        XCTAssertEqual(token, "")
    }

    func test_parse_beneficiaryAcceptanceLink_tokenWithSpecialChars_returnsBeneficiaryAcceptance() {
        let token = "token-with-special_chars.123"
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-abc/accept?token=\(token)")!
        let result = router.parse(url: url)
        guard case .beneficiaryAcceptance(let vaultID, let parsedToken) = result else {
            XCTFail("Expected beneficiaryAcceptance, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-abc")
        XCTAssertEqual(parsedToken, token)
    }

    // MARK: - Valid Preview Links

    func test_parse_previewLink_valid_returnsVaultPreview() {
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-123/preview")!
        let result = router.parse(url: url)
        guard case .vaultPreview(let vaultID) = result else {
            XCTFail("Expected vaultPreview, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-123")
    }

    func test_parse_previewLink_hyphenatedVaultID_returnsVaultPreview() {
        let url = URL(string: "https://ttl-legacy.app/vaults/preview-vault-789/preview")!
        let result = router.parse(url: url)
        guard case .vaultPreview(let vaultID) = result else {
            XCTFail("Expected vaultPreview, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "preview-vault-789")
    }

    // MARK: - Valid Vault Action Links

    func test_parse_vaultActionLink_checkIn_returnsVaultAction() {
        let url = URL(string: "ttllegacy://vault/vault-abc/check-in")!
        let result = router.parse(url: url)
        guard case .vaultAction(let vaultID, let action) = result else {
            XCTFail("Expected vaultAction, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-abc")
        XCTAssertEqual(action, .checkIn)
    }

    func test_parse_vaultActionLink_withdraw_returnsVaultAction() {
        let url = URL(string: "ttllegacy://vault/vault-123/withdraw")!
        let result = router.parse(url: url)
        guard case .vaultAction(let vaultID, let action) = result else {
            XCTFail("Expected vaultAction, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-123")
        XCTAssertEqual(action, .withdraw)
    }

    func test_parse_vaultActionLink_viewDetails_returnsVaultAction() {
        let url = URL(string: "ttllegacy://vault/v1/view-details")!
        let result = router.parse(url: url)
        guard case .vaultAction(let vaultID, let action) = result else {
            XCTFail("Expected vaultAction, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "v1")
        XCTAssertEqual(action, .viewDetails)
    }

    func test_parse_vaultActionLink_manageBeneficiary_returnsVaultAction() {
        let url = URL(string: "ttllegacy://vault/vault-42/manage-beneficiary")!
        let result = router.parse(url: url)
        guard case .vaultAction(let vaultID, let action) = result else {
            XCTFail("Expected vaultAction, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-42")
        XCTAssertEqual(action, .manageBeneficiary)
    }

    func test_parse_vaultActionLink_hyphenatedVaultID_returnsVaultAction() {
        let url = URL(string: "ttllegacy://vault/vault-with-dashes/check-in")!
        let result = router.parse(url: url)
        guard case .vaultAction(let vaultID, let action) = result else {
            XCTFail("Expected vaultAction, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-with-dashes")
        XCTAssertEqual(action, .checkIn)
    }

    // MARK: - Malformed URLs - Missing Components

    func test_parse_malformedURL_missingVaultID_returnsNil() {
        let url = URL(string: "https://ttl-legacy.app/vaults//invite")!
        let result = router.parse(url: url)
        XCTAssertNil(result)
    }

    func test_parse_malformedURL_missingAction_returnsNil() {
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-1")!
        let result = router.parse(url: url)
        XCTAssertNil(result)
    }

    func test_parse_malformedURL_tooManyPathComponents_returnsNil() {
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-1/extra/invite")!
        let result = router.parse(url: url)
        XCTAssertNil(result)
    }

    func test_parse_malformedURL_invalidAction_returnsNil() {
        let url = URL(string: "ttllegacy://vault/vault-1/invalid-action")!
        let result = router.parse(url: url)
        XCTAssertNil(result)
    }

    func test_parse_malformedURL_emptyString_returnsNil() {
        let url = URL(string: "")!
        let result = router.parse(url: url)
        XCTAssertNil(result)
    }

    // MARK: - Wrong Host

    func test_parse_wrongHost_https_returnsNil() {
        let url = URL(string: "https://wrong-host.com/vaults/vault-1/invite")!
        let result = router.parse(url: url)
        XCTAssertNil(result)
    }

    func test_parse_wrongScheme_customScheme_withWrongHost_returnsNil() {
        let url = URL(string: "ttllegacy://other/vault-1/check-in")!
        let result = router.parse(url: url)
        XCTAssertNil(result)
    }

    func test_parse_unknownScheme_returnsNil() {
        let url = URL(string: "unknown://vault/vault-1/check-in")!
        let result = router.parse(url: url)
        XCTAssertNil(result)
    }

    // MARK: - Edge Cases

    func test_parse_vaultActionLink_caseInsensitive_actionNotRecognized() {
        let url = URL(string: "ttllegacy://vault/v1/CHECK-IN")!
        let result = router.parse(url: url)
        XCTAssertNil(result)
    }

    func test_parse_httpURL_ttlLegacyHost_returnsNil() {
        let url = URL(string: "http://ttl-legacy.app/vaults/vault-1/invite")!
        let result = router.parse(url: url)
        XCTAssertNil(result)
    }

    func test_parse_beneficiaryAcceptanceLink_multipleTokenParams_usesFirst() {
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-1/accept?token=first&token=second")!
        let result = router.parse(url: url)
        guard case .beneficiaryAcceptance(_, let token) = result else {
            XCTFail("Expected beneficiaryAcceptance, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(token, "first")
    }

    func test_parse_vaultActionLink_withQueryParams_ignoresParams() {
        let url = URL(string: "ttllegacy://vault/vault-123/check-in?unused=param")!
        let result = router.parse(url: url)
        guard case .vaultAction(let vaultID, let action) = result else {
            XCTFail("Expected vaultAction, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-123")
        XCTAssertEqual(action, .checkIn)
    }

    func test_parse_universalLink_withFragment_valid() {
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-abc/invite#section")!
        let result = router.parse(url: url)
        guard case .vaultInvitation(let vaultID) = result else {
            XCTFail("Expected vaultInvitation, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault-abc")
    }

    func test_parse_vaultActionLink_specialCharsInVaultID_valid() {
        let url = URL(string: "ttllegacy://vault/vault_abc_123/check-in")!
        let result = router.parse(url: url)
        guard case .vaultAction(let vaultID, let action) = result else {
            XCTFail("Expected vaultAction, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(vaultID, "vault_abc_123")
        XCTAssertEqual(action, .checkIn)
    }

    func test_parse_beneficiaryAcceptanceLink_encodedToken_valid() {
        let encodedToken = "token%20with%20spaces"
        let url = URL(string: "https://ttl-legacy.app/vaults/vault-1/accept?token=\(encodedToken)")!
        let result = router.parse(url: url)
        guard case .beneficiaryAcceptance(_, let token) = result else {
            XCTFail("Expected beneficiaryAcceptance, got \(String(describing: result))")
            return
        }
        // URLComponents should handle decoding
        XCTAssertTrue(!token.isEmpty)
    }
}
