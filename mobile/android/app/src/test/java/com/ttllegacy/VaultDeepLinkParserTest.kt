package com.ttllegacy

import com.ttllegacy.services.VaultDeepLinkAction
import com.ttllegacy.services.VaultDeepLinkParser
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class VaultDeepLinkParserTest {

    @Test
    fun parseUrl_checkIn_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttllegacy://vault/vault-abc-123/check-in")
        assertEquals("vault-abc-123", result?.vaultId)
        assertEquals(VaultDeepLinkAction.CHECK_IN, result?.action)
    }

    @Test
    fun parseUrl_withdraw_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttllegacy://vault/vault-xyz/withdraw")
        assertEquals("vault-xyz", result?.vaultId)
        assertEquals(VaultDeepLinkAction.WITHDRAW, result?.action)
    }

    @Test
    fun parseUrl_viewDetails_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttllegacy://vault/v1/view-details")
        assertEquals("v1", result?.vaultId)
        assertEquals(VaultDeepLinkAction.VIEW_DETAILS, result?.action)
    }

    @Test
    fun parseUrl_manageBeneficiary_returnsVaultDeepLink() {
        val result = VaultDeepLinkParser.parseUrl("ttllegacy://vault/vault-42/manage-beneficiary")
        assertEquals("vault-42", result?.vaultId)
        assertEquals(VaultDeepLinkAction.MANAGE_BENEFICIARY, result?.action)
    }

    @Test
    fun parseUrl_unknownAction_returnsNull() {
        assertNull(VaultDeepLinkParser.parseUrl("ttllegacy://vault/vault-1/unknown-action"))
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
    fun parseUrl_missingActionSegment_returnsNull() {
        assertNull(VaultDeepLinkParser.parseUrl("ttllegacy://vault/v1"))
    }
}
