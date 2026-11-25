package com.ttllegacy

import com.ttllegacy.services.VaultDeepLinkAction
import com.ttllegacy.services.VaultDeepLinkParser
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Unit tests for [VaultDeepLinkParser].
 *
 * Covers both URI schemes:
 *  - `ttllegacy://` — legacy custom scheme (backwards compatibility)
 *  - `ttl-legacy://` — new App Links scheme with android:autoVerify="true" (Issue #1146)
 */
class VaultDeepLinkParserTest {

    // -----------------------------------------------------------------------
    // Legacy scheme: ttllegacy://
    // -----------------------------------------------------------------------

    @Test
    fun parseUrl_legacy_checkIn_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttllegacy://vault/vault-abc-123/check-in")
        assertEquals("vault-abc-123", result?.vaultId)
        assertEquals(VaultDeepLinkAction.CHECK_IN, result?.action)
    }

    @Test
    fun parseUrl_legacy_withdraw_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttllegacy://vault/vault-xyz/withdraw")
        assertEquals("vault-xyz", result?.vaultId)
        assertEquals(VaultDeepLinkAction.WITHDRAW, result?.action)
    }

    @Test
    fun parseUrl_legacy_viewDetails_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttllegacy://vault/v1/view-details")
        assertEquals("v1", result?.vaultId)
        assertEquals(VaultDeepLinkAction.VIEW_DETAILS, result?.action)
    }

    @Test
    fun parseUrl_legacy_manageBeneficiary_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttllegacy://vault/vault-42/manage-beneficiary")
        assertEquals("vault-42", result?.vaultId)
        assertEquals(VaultDeepLinkAction.MANAGE_BENEFICIARY, result?.action)
    }

    // -----------------------------------------------------------------------
    // New App Links scheme: ttl-legacy:// (Issue #1146)
    // Registered with android:autoVerify="true" in AndroidManifest.xml.
    // -----------------------------------------------------------------------

    @Test
    fun parseUrl_appLinks_checkIn_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttl-legacy://vault/vault-abc-123/check-in")
        assertEquals("vault-abc-123", result?.vaultId)
        assertEquals(VaultDeepLinkAction.CHECK_IN, result?.action)
    }

    @Test
    fun parseUrl_appLinks_withdraw_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttl-legacy://vault/vault-xyz/withdraw")
        assertEquals("vault-xyz", result?.vaultId)
        assertEquals(VaultDeepLinkAction.WITHDRAW, result?.action)
    }

    @Test
    fun parseUrl_appLinks_viewDetails_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttl-legacy://vault/v1/view-details")
        assertEquals("v1", result?.vaultId)
        assertEquals(VaultDeepLinkAction.VIEW_DETAILS, result?.action)
    }

    @Test
    fun parseUrl_appLinks_manageBeneficiary_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttl-legacy://vault/vault-42/manage-beneficiary")
        assertEquals("vault-42", result?.vaultId)
        assertEquals(VaultDeepLinkAction.MANAGE_BENEFICIARY, result?.action)
    }

    @Test
    fun parseUrl_appLinks_hyphenatedVaultId_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttl-legacy://vault/vault-with-dashes-99/check-in")
        assertEquals("vault-with-dashes-99", result?.vaultId)
        assertEquals(VaultDeepLinkAction.CHECK_IN, result?.action)
    }

    // -----------------------------------------------------------------------
    // Negative cases (apply to both schemes)
    // -----------------------------------------------------------------------

    @Test
    fun parseUrl_unknownAction_returnsNull() {
        assertNull(VaultDeepLinkParser.parseUrl("ttllegacy://vault/vault-1/unknown-action"))
    }

    @Test
    fun parseUrl_appLinks_unknownAction_returnsNull() {
        assertNull(VaultDeepLinkParser.parseUrl("ttl-legacy://vault/vault-1/unknown-action"))
    }

    @Test
    fun parseUrl_wrongScheme_returnsNull() {
        assertNull(VaultDeepLinkParser.parseUrl("https://ttl-legacy.app/vault/v1/check-in"))
    }

    @Test
    fun parseUrl_wrongHost_returnsNull() {
        assertNull(VaultDeepLinkParser.parseUrl("ttllegacy://other/v1/check-in"))
    }

    @Test
    fun parseUrl_appLinks_wrongHost_returnsNull() {
        assertNull(VaultDeepLinkParser.parseUrl("ttl-legacy://other/v1/check-in"))
    }

    @Test
    fun parseUrl_missingActionSegment_returnsNull() {
        assertNull(VaultDeepLinkParser.parseUrl("ttllegacy://vault/v1"))
    }

    @Test
    fun parseUrl_appLinks_missingActionSegment_returnsNull() {
        assertNull(VaultDeepLinkParser.parseUrl("ttl-legacy://vault/v1"))
    }

    @Test
    fun parseUrl_emptyString_returnsNull() {
        assertNull(VaultDeepLinkParser.parseUrl(""))
    }
}
