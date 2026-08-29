package com.ttllegacy.services

import androidx.biometric.BiometricPrompt
import androidx.credentials.exceptions.GetCredentialException
import androidx.credentials.exceptions.NoCredentialException

/**
 * Encapsulates the decision logic for when the passkey/FIDO2 authentication
 * flow should fall back to biometric (fingerprint / face / device-PIN) auth,
 * and when an error code represents a deliberate user cancellation.
 *
 * Keeping this logic in its own class makes it easy to unit-test independently
 * of the full [PasskeyService] and [BiometricHelper] stack.
 */
class AuthFallbackManager {

    /**
     * Returns `true` when [exception] represents a condition where FIDO2 /
     * passkey authentication is genuinely unavailable on this device or for
     * this account — i.e. a scenario where falling back to biometric auth is
     * appropriate rather than surfacing an error to the user.
     *
     * Returns `false` for user-initiated cancellations or other transient
     * errors that should be propagated to the caller unchanged.
     */
    fun shouldFallbackToBiometric(exception: Exception): Boolean {
        // NoCredentialException: no passkey registered for this device / RP.
        if (exception is NoCredentialException) return true

        // GetCredentialException covers a broad range of FIDO2 failures.
        // We inspect the type string for known hardware-unavailability signals.
        if (exception is GetCredentialException) {
            return isFido2UnavailableType(exception.type)
        }

        // Catch-all: inspect the exception message for common indicators.
        return isFido2UnavailableMessage(exception.message)
    }

    /**
     * Returns `true` when [errorCode] represents a deliberate user
     * cancellation of the biometric prompt rather than a hardware or
     * configuration failure.
     *
     * Codes covered:
     *  - [BiometricPrompt.ERROR_USER_CANCELED]  — user dismissed via back/home
     *  - [BiometricPrompt.ERROR_NEGATIVE_BUTTON] — user tapped the negative button
     */
    fun isUserCancellation(errorCode: Int): Boolean =
        errorCode == BiometricPrompt.ERROR_USER_CANCELED ||
        errorCode == BiometricPrompt.ERROR_NEGATIVE_BUTTON

    // ── Internal helpers ─────────────────────────────────────────────────────

    /**
     * Returns `true` when the [GetCredentialException.type] string indicates
     * that FIDO2 hardware or registration is not available.
     *
     * The Credential Manager API uses fully-qualified string constants for
     * exception types (see `androidx.credentials.exceptions`).
     */
    private fun isFido2UnavailableType(type: String): Boolean {
        val unavailableTypes = setOf(
            "android.credentials.GetCredentialException.TYPE_NO_CREDENTIAL",
            "androidx.credentials.TYPE_NO_CREDENTIAL_EXCEPTION",
            "android.credentials.GetCredentialException.TYPE_UNSUPPORTED_PROVIDER",
            "androidx.credentials.TYPE_GET_CREDENTIAL_UNKNOWN_EXCEPTION",
            "android.credentials.GetCredentialException.TYPE_PROVIDER_CONFIGURATION_ISSUE"
        )
        return type in unavailableTypes
    }

    /**
     * Returns `true` when the exception [message] text contains keywords that
     * typically indicate FIDO2 hardware or credential absence, used as a
     * last-resort heuristic for exception types not covered by structured
     * Credential Manager types.
     */
    private fun isFido2UnavailableMessage(message: String?): Boolean {
        if (message == null) return false
        val lowerMsg = message.lowercase()
        val unavailableKeywords = listOf(
            "no credential",
            "no passkey",
            "fido2 not supported",
            "hardware not available",
            "not supported",
            "no fido",
            "credential not found"
        )
        return unavailableKeywords.any { it in lowerMsg }
    }
}
