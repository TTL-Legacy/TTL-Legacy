package com.ttllegacy.services

import android.net.Uri

enum class VaultDeepLinkAction(val pathSegment: String) {
    CHECK_IN("check-in"),
    WITHDRAW("withdraw"),
    VIEW_DETAILS("view-details"),
    MANAGE_BENEFICIARY("manage-beneficiary");

    companion object {
        fun fromPathSegment(segment: String): VaultDeepLinkAction? =
            entries.find { it.pathSegment == segment }
    }
}

data class VaultDeepLink(val vaultId: String, val action: VaultDeepLinkAction)

object VaultDeepLinkParser {
    /** Parses ttllegacy://vault/{vault_id}/{action} from a URL string or returns null if unrecognised. */
    fun parseUrl(url: String): VaultDeepLink? {
        val match = URL_PATTERN.matchEntire(url.trim()) ?: return null
        val action = VaultDeepLinkAction.fromPathSegment(match.groupValues[2]) ?: return null
        return VaultDeepLink(vaultId = match.groupValues[1], action = action)
    }

    /** Parses ttllegacy://vault/{vault_id}/{action} from a Uri or returns null if unrecognised. */
    fun parse(uri: Uri): VaultDeepLink? {
        if (uri.scheme != "ttllegacy" || uri.host != "vault") return null
        val segments = uri.pathSegments
        if (segments.size != 2) return null
        val action = VaultDeepLinkAction.fromPathSegment(segments[1]) ?: return null
        return VaultDeepLink(vaultId = segments[0], action = action)
    }

    private val URL_PATTERN = Regex("^ttllegacy://vault/([^/]+)/([^/]+)$")
}
