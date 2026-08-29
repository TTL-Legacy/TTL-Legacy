package com.ttllegacy.services

import androidx.activity.ComponentActivity
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.biometric.BiometricPrompt

class BiometricHelper(private val activity: ComponentActivity) {

    fun isAvailable(): Boolean {
        val mgr = BiometricManager.from(activity)
        val result = mgr.canAuthenticate(BIOMETRIC_STRONG or DEVICE_CREDENTIAL)
        return result == BiometricManager.BIOMETRIC_SUCCESS
    }

    /**
     * Shows the biometric / device-credential prompt.
     *
     * @param title     Prompt title shown to the user.
     * @param subtitle  Prompt subtitle shown to the user.
     * @param onSuccess Called on successful authentication.
     * @param onError   Called with the error message string on terminal failure.
     */
    fun authenticate(
        title: String,
        subtitle: String,
        onSuccess: () -> Unit,
        onError: (String) -> Unit,
    ) {
        authenticate(
            title = title,
            subtitle = subtitle,
            onSuccess = onSuccess,
            onErrorWithCode = { _, msg -> onError(msg) }
        )
    }

    /**
     * Shows the biometric / device-credential prompt, exposing the raw
     * [BiometricPrompt] error code alongside the error message.
     *
     * Use this overload when you need to distinguish user cancellation
     * (e.g. [BiometricPrompt.ERROR_USER_CANCELED]) from hardware failures.
     *
     * @param title           Prompt title shown to the user.
     * @param subtitle        Prompt subtitle shown to the user.
     * @param onSuccess       Called on successful authentication.
     * @param onErrorWithCode Called with `(errorCode, errorMessage)` on terminal failure.
     */
    fun authenticate(
        title: String,
        subtitle: String,
        onSuccess: () -> Unit,
        onErrorWithCode: (errorCode: Int, errorMessage: String) -> Unit,
    ) {
        val callback = object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                onSuccess()
            }

            override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                onErrorWithCode(errorCode, errString.toString())
            }

            override fun onAuthenticationFailed() {
                // Informational only — the prompt stays open so the user can retry.
            }
        }

        val prompt = BiometricPrompt(activity, callback)

        val promptInfo = BiometricPrompt.PromptInfo.Builder()
            .setTitle(title)
            .setSubtitle(subtitle)
            .setAllowedAuthenticators(BIOMETRIC_STRONG or DEVICE_CREDENTIAL)
            .build()

        prompt.authenticate(promptInfo)
    }
}
