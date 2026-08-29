package com.ttllegacy.services

import android.app.Activity
import androidx.activity.ComponentActivity
import androidx.credentials.*
import androidx.credentials.exceptions.GetCredentialException
import androidx.credentials.exceptions.NoCredentialException
import com.ttllegacy.api.ApiClient
import com.ttllegacy.api.ApiResult
import com.ttllegacy.api.TokenProvider
import com.ttllegacy.models.PasskeyRegisterRequest
import com.ttllegacy.models.PasskeyVerifyRequest
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.suspendCancellableCoroutine
import org.json.JSONArray
import org.json.JSONObject
import java.util.Base64
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** Sentinel token written to [TokenProvider] when the app falls back to biometric auth. */
internal const val BIOMETRIC_FALLBACK_TOKEN = "biometric-fallback-session"

@Singleton
class PasskeyService @Inject constructor(
    private val apiClient: ApiClient,
    private val tokenProvider: TokenProvider
) {
    private val fallbackManager = AuthFallbackManager()

    suspend fun register(activity: Activity, username: String): Result<Unit> = runCatching {
        val challenge = requireSuccess(apiClient.getChallenge()).challenge
        val requestJson = JSONObject().apply {
            put("challenge", challenge)
            put("rp", JSONObject().put("id", "ttl-legacy.app").put("name", "TTL-Legacy"))
            put("user", JSONObject()
                .put("id", Base64.getUrlEncoder().withoutPadding().encodeToString(username.toByteArray()))
                .put("name", username).put("displayName", username))
            put("pubKeyCredParams", JSONArray().put(JSONObject().put("type", "public-key").put("alg", -7)))
            put("authenticatorSelection", JSONObject()
                .put("authenticatorAttachment", "platform")
                .put("requireResidentKey", true)
                .put("userVerification", "required"))
        }.toString()

        val credManager = CredentialManager.create(activity)
        val resp = credManager.createCredential(activity, CreatePublicKeyCredentialRequest(requestJson))
                as CreatePublicKeyCredentialResponse
        val json = JSONObject(resp.registrationResponseJson)
        val regReq = PasskeyRegisterRequest(
            credentialId = json.getString("id"),
            publicKey = json.getJSONObject("response").getString("attestationObject"),
            clientDataJson = json.getJSONObject("response").getString("clientDataJSON")
        )
        requireSuccess(apiClient.registerPasskey(regReq))
    }

    /**
     * Authenticates the user.
     *
     * Primary path: FIDO2 / passkey via [CredentialManager].
     *
     * Fallback path (triggered when FIDO2 is unavailable — no hardware or no
     * enrolled credential): biometric prompt via [BiometricHelper]. On success
     * a sentinel token [BIOMETRIC_FALLBACK_TOKEN] is written to [tokenProvider]
     * so the rest of the app can proceed in an offline/biometric-only mode.
     *
     * User cancellations (from either path) are re-thrown as [CancellationException].
     */
    suspend fun authenticate(activity: Activity): Result<Unit> = runCatching {
        try {
            authenticateWithPasskey(activity)
        } catch (e: Exception) {
            if (isFido2Unavailable(e)) {
                authenticateWithBiometricFallback(activity, originalException = e)
            } else {
                throw e
            }
        }
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    private suspend fun authenticateWithPasskey(activity: Activity) {
        val challenge = requireSuccess(apiClient.getChallenge()).challenge
        val requestJson = JSONObject()
            .put("challenge", challenge).put("rpId", "ttl-legacy.app")
            .put("userVerification", "required").toString()

        val credManager = CredentialManager.create(activity)
        val request = GetCredentialRequest(listOf(GetPublicKeyCredentialOption(requestJson)))
        val credential = credManager.getCredential(activity, request).credential as PublicKeyCredential
        val json = JSONObject(credential.authenticationResponseJson)
        val verifyReq = PasskeyVerifyRequest(
            credentialId = json.getString("id"),
            clientDataJson = json.getJSONObject("response").getString("clientDataJSON"),
            signature = json.getJSONObject("response").getString("signature")
        )
        tokenProvider.token = requireSuccess(apiClient.verifyPasskey(verifyReq)).token
    }

    /**
     * Falls back to [BiometricHelper] when FIDO2 is unavailable.
     *
     * On biometric success: sets [tokenProvider.token] to [BIOMETRIC_FALLBACK_TOKEN].
     * On user cancellation: re-throws as [CancellationException].
     * On other biometric error: re-throws [originalException].
     */
    private suspend fun authenticateWithBiometricFallback(
        activity: Activity,
        originalException: Exception
    ) {
        require(activity is ComponentActivity) {
            "BiometricHelper requires a ComponentActivity; got ${activity::class.simpleName}"
        }

        suspendCancellableCoroutine { cont ->
            BiometricHelper(activity).authenticate(
                title = "Verify your identity",
                subtitle = "Use biometric authentication to access your vault",
                onSuccess = {
                    tokenProvider.token = BIOMETRIC_FALLBACK_TOKEN
                    cont.resume(Unit)
                },
                onErrorWithCode = { errorCode, _ ->
                    if (fallbackManager.isUserCancellation(errorCode)) {
                        cont.resumeWithException(CancellationException("Authentication cancelled by user"))
                    } else {
                        cont.resumeWithException(originalException)
                    }
                }
            )
        }
    }

    /**
     * Returns `true` when [e] indicates that FIDO2 / passkey authentication is
     * not available on this device or for this account (no hardware, no enrolled
     * credential), meaning a biometric fallback is appropriate.
     *
     * Returns `false` for user cancellations or transient errors.
     */
    fun isFido2Unavailable(e: Exception): Boolean = fallbackManager.shouldFallbackToBiometric(e)

    private fun <T> requireSuccess(result: ApiResult<T>): T {
        return when (result) {
            is ApiResult.Success -> result.data
            is ApiResult.Error -> error(result.message)
            ApiResult.NetworkUnavailable -> error("No network connection")
        }
    }
}
