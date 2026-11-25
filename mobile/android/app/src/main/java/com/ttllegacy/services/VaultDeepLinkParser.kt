package com.ttllegacy.services

import android.net.Uri

/**
 * Deep-link action types supported by TTL-Legacy vault URIs.
 *
 * Each value maps to a URL path segment used in the navigation graph.
 */
enum class VaultDeepLinkAction(val pathSegment: String) {
    CHECK_IN("check-in"),
    WITHDRAW("withdraw"),
    VIEW_DETAILS("view-details"),
    MANAGE_BENEFICIARY("manage-beneficiary"),
    PREVIEW("preview");

    companion object {
        fun fromPathSegment(segment: String): VaultDeepLinkAction? =
            entries.find { it.pathSegment == segment }
    }
}

/** Parsed representation of an incoming vault deep-link. */
data class VaultDeepLink(val vaultId: String, val action: VaultDeepLinkAction)

/**
 * Parser for TTL-Legacy vault deep-link URIs.
 *
 * Supported schemes (Issue #1146):
 *  - `ttl-legacy://vault/{vaultId}/{action}` — new App Links scheme with
 *    `android:autoVerify="true"` (no browser disambiguation dialog once
 *    the assetlinks.json fingerprint is verified by the OS).
 *  - `ttllegacy://vault/{vaultId}/{action}` — legacy custom scheme kept for
 *    backwards compatibility with existing notifications and shared links.
 *
 * Both schemes use `vault` as the host and encode the vault ID and action
 * as path segments:
 *
 * ```
 * ttl-legacy://vault/abc-123/check-in
 * ttllegacy://vault/abc-123/view-details
 * ```
 *
 * Valid actions: `check-in`, `withdraw`, `view-details`, `manage-beneficiary`.
 */
object VaultDeepLinkParser {

    /**
     * Recognised URI schemes. The first entry is the primary (App Links)
     * scheme; the second is the legacy custom scheme.
     */
    private val SUPPORTED_SCHEMES = setOf("ttl-legacy", "ttllegacy")

    /**
     * Regex that matches both custom schemes and extracts vault ID and action.
     *
     * Pattern: `{scheme}://vault/{vaultId}/{action}`
     */
    private val URL_PATTERN = Regex(
        """^(?:ttl-legacy|ttllegacy)://vault/([^/]+)/([^/]+)$"""
    )

    /**
     * Regex that matches HTTPS App Links of the form:
     * `https://ttl-legacy.app/vaults/{vaultId}/{action}`
     *
     * This covers share-preview links and any future web-hosted deep links.
     */
    private val HTTPS_URL_PATTERN = Regex(
        """^https://ttl-legacy\.app/vaults/([^/]+)/([^/]+)$"""
    )

    /**
     * Parses a [Uri] into a [VaultDeepLink] or returns `null` if the URI
     * does not match a recognised vault deep-link format.
     *
     * Supports:
     *  - Custom schemes: `ttl-legacy://vault/{vaultId}/{action}`
     *  - HTTPS App Links: `https://ttl-legacy.app/vaults/{vaultId}/{action}`
     */
    fun parse(uri: Uri): VaultDeepLink? {
        // Handle HTTPS App Link: https://ttl-legacy.app/vaults/{vaultId}/{action}
        if (uri.scheme == "https" && uri.host == "ttl-legacy.app") {
            val segments = uri.pathSegments
            // segments = ["vaults", vaultId, action]
            if (segments.size == 3 && segments[0] == "vaults") {
                val action = VaultDeepLinkAction.fromPathSegment(segments[2]) ?: return null
                return VaultDeepLink(vaultId = segments[1], action = action)
            }
            return null
        }

        // Handle custom schemes: ttl-legacy:// and ttllegacy://
        if (uri.scheme !in SUPPORTED_SCHEMES) return null
        if (uri.host != "vault") return null

        val segments = uri.pathSegments
        if (segments.size != 2) return null

        val action = VaultDeepLinkAction.fromPathSegment(segments[1]) ?: return null
        return VaultDeepLink(vaultId = segments[0], action = action)
    }

    /**
     * Parses a raw URL string into a [VaultDeepLink] or returns `null` if
     * the URL does not match a recognised vault deep-link format.
     *
     * This overload is useful in unit tests and notification payloads where
     * a plain string is available rather than an [android.net.Uri].
     *
     * Supports:
     *  - Custom schemes: `ttl-legacy://vault/{vaultId}/{action}`
     *  - HTTPS App Links: `https://ttl-legacy.app/vaults/{vaultId}/{action}`
     */
    fun parseUrl(url: String): VaultDeepLink? {
        val trimmed = url.trim()

        // Try HTTPS pattern first (covers share-preview links).
        val httpsMatch = HTTPS_URL_PATTERN.matchEntire(trimmed)
        if (httpsMatch != null) {
            val action = VaultDeepLinkAction.fromPathSegment(httpsMatch.groupValues[2]) ?: return null
            return VaultDeepLink(vaultId = httpsMatch.groupValues[1], action = action)
        }

        // Fall back to custom-scheme pattern.
        val match = URL_PATTERN.matchEntire(trimmed) ?: return null
        val action = VaultDeepLinkAction.fromPathSegment(match.groupValues[2]) ?: return null
        return VaultDeepLink(vaultId = match.groupValues[1], action = action)
    }
}
