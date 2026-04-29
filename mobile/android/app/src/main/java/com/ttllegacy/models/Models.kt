package com.ttllegacy.models

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class Vault(
    val id: String,
    val owner: String,
    val beneficiary: String,
    val balance: Long,
    @SerialName("check_in_interval") val checkInInterval: Long,
    @SerialName("last_check_in") val lastCheckIn: String,
    @SerialName("ttl_remaining") val ttlRemaining: Long? = null,
    val status: VaultStatus
) {
    val isExpiringSoon: Boolean get() = (ttlRemaining ?: Long.MAX_VALUE) < 86_400L
    val formattedBalance: String get() = "%.7f XLM".format(balance / 10_000_000.0)
}

@Serializable
enum class VaultStatus { active, expired, released, paused }

@Serializable
data class AuthChallenge(
    val challenge: String,
    @SerialName("expires_at") val expiresAt: String
)

@Serializable
data class AuthToken(
    val token: String,
    @SerialName("expires_at") val expiresAt: String
)

@Serializable
data class CreateVaultRequest(
    val beneficiary: String,
    @SerialName("check_in_interval") val checkInInterval: Long
)

@Serializable
data class PushRegistration(
    val token: String,
    val platform: String = "android"
)

@Serializable
data class PasskeyVerifyRequest(
    @SerialName("credential_id") val credentialId: String,
    @SerialName("client_data_json") val clientDataJson: String,
    val signature: String
)

@Serializable
data class PasskeyRegisterRequest(
    @SerialName("credential_id") val credentialId: String,
    @SerialName("public_key") val publicKey: String,
    @SerialName("client_data_json") val clientDataJson: String
)
