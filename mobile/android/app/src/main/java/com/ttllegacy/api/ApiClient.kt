package com.ttllegacy.api

import com.ttllegacy.models.*
import io.ktor.client.*
import io.ktor.client.call.*
import io.ktor.client.engine.android.*
import io.ktor.client.plugins.contentnegotiation.*
import io.ktor.client.plugins.logging.*
import io.ktor.client.request.*
import io.ktor.http.*
import io.ktor.serialization.kotlinx.json.*
import kotlinx.serialization.json.Json
import javax.inject.Inject
import javax.inject.Singleton

sealed class ApiResult<out T> {
    data class Success<T>(val data: T) : ApiResult<T>()
    data class Error(val message: String, val code: Int = 0) : ApiResult<Nothing>()
    object NetworkUnavailable : ApiResult<Nothing>()
}

@Singleton
class ApiClient @Inject constructor(
    private val tokenProvider: TokenProvider,
    private val networkMonitor: NetworkMonitor,
    private val offlineCache: OfflineCache,
    private val baseUrl: String
) {
    private val client = HttpClient(Android) {
        install(ContentNegotiation) {
            json(Json { ignoreUnknownKeys = true; isLenient = true })
        }
        install(Logging) { level = LogLevel.INFO }
    }

    // Auth
    suspend fun getChallenge(): ApiResult<AuthChallenge> = get("/auth/challenge")
    suspend fun verifyPasskey(req: PasskeyVerifyRequest): ApiResult<AuthToken> = post("/auth/verify", req)
    suspend fun registerPasskey(req: PasskeyRegisterRequest): ApiResult<Unit> = post("/auth/register", req)

    // Vaults
    suspend fun listVaults(): ApiResult<List<Vault>> = get("/vaults")
    suspend fun getVault(id: String): ApiResult<Vault> = get("/vaults/$id")
    suspend fun createVault(req: CreateVaultRequest): ApiResult<Vault> = post("/vaults", req)
    suspend fun checkIn(vaultId: String): ApiResult<Unit> = post("/vaults/$vaultId/checkin", Unit)
    suspend fun deposit(vaultId: String, amount: Long): ApiResult<Vault> =
        post("/vaults/$vaultId/deposit", mapOf("amount" to amount))
    suspend fun withdraw(vaultId: String, amount: Long): ApiResult<Vault> =
        post("/vaults/$vaultId/withdraw", mapOf("amount" to amount))

    // Push
    suspend fun registerPushToken(token: String): ApiResult<Unit> =
        post("/notifications/register", PushRegistration(token = token))
    suspend fun unregisterPushToken(token: String): ApiResult<Unit> =
        delete("/notifications/register", PushRegistration(token = token))

    // Internals
    private suspend inline fun <reified T> get(path: String): ApiResult<T> {
        if (!networkMonitor.isConnected) {
            val cached = offlineCache.load(path)
            return if (cached != null) ApiResult.Success(Json.decodeFromString(cached))
            else ApiResult.NetworkUnavailable
        }
        return runCatching {
            val response = client.get("$baseUrl$path") { bearerAuth() }
            when (response.status.value) {
                in 200..299 -> {
                    val body: T = response.body()
                    offlineCache.save(path, Json.encodeToString(kotlinx.serialization.serializer(), body))
                    ApiResult.Success(body)
                }
                401 -> ApiResult.Error("Unauthorized", 401)
                404 -> ApiResult.Error("Not found", 404)
                else -> ApiResult.Error("Server error ${response.status.value}", response.status.value)
            }
        }.getOrElse { ApiResult.Error(it.message ?: "Unknown error") }
    }

    private suspend inline fun <reified B, reified T> post(path: String, body: B): ApiResult<T> {
        if (!networkMonitor.isConnected) return ApiResult.NetworkUnavailable
        return runCatching {
            val response = client.post("$baseUrl$path") {
                bearerAuth()
                contentType(ContentType.Application.Json)
                setBody(body)
            }
            when (response.status.value) {
                in 200..299 -> ApiResult.Success(if (T::class == Unit::class) Unit as T else response.body())
                401 -> ApiResult.Error("Unauthorized", 401)
                else -> ApiResult.Error("Server error ${response.status.value}", response.status.value)
            }
        }.getOrElse { ApiResult.Error(it.message ?: "Unknown error") }
    }

    private suspend inline fun <reified B, reified T> delete(path: String, body: B): ApiResult<T> {
        if (!networkMonitor.isConnected) return ApiResult.NetworkUnavailable
        return runCatching {
            val response = client.delete("$baseUrl$path") {
                bearerAuth()
                contentType(ContentType.Application.Json)
                setBody(body)
            }
            ApiResult.Success(if (T::class == Unit::class) Unit as T else response.body())
        }.getOrElse { ApiResult.Error(it.message ?: "Unknown error") }
    }

    private fun HttpRequestBuilder.bearerAuth() {
        tokenProvider.token?.let { header(HttpHeaders.Authorization, "Bearer $it") }
    }
}
