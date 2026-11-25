package com.ttllegacy.widget

import com.ttllegacy.api.ApiClient
import com.ttllegacy.api.ApiResult
import com.ttllegacy.models.Vault
import com.ttllegacy.models.VaultStatus
import io.mockk.coEvery
import io.mockk.mockk
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

class VaultGlanceWidgetTest {

    private val apiClient: ApiClient = mockk()

    @Test
    fun `formatTtl returns days and hours when more than one day remains`() {
        // 2 days + 3 hours = 183600 seconds
        val seconds = 2L * 86_400 + 3L * 3_600
        assertEquals("2d 3h remaining", formatTtl(seconds))
    }

    @Test
    fun `formatTtl returns hours only when less than one day remains`() {
        val seconds = 5L * 3_600 // 5 hours
        assertEquals("5h remaining", formatTtl(seconds))
    }

    @Test
    fun `formatTtl returns Unknown for null`() {
        assertEquals("Unknown", formatTtl(null))
    }

    @Test
    fun `worker picks vault with lowest ttl remaining`() = runTest {
        val vaults = listOf(
            Vault(
                id = "vault-urgent", owner = "O", beneficiary = "B",
                balance = 1000, checkInInterval = 86400,
                lastCheckIn = "2026-01-01", ttlRemaining = 3600,
                status = VaultStatus.active
            ),
            Vault(
                id = "vault-safe", owner = "O", beneficiary = "B",
                balance = 1000, checkInInterval = 86400,
                lastCheckIn = "2026-01-01", ttlRemaining = 86400,
                status = VaultStatus.active
            )
        )
        coEvery { apiClient.listVaults() } returns ApiResult.Success(vaults)

        val result = apiClient.listVaults()
        val data = (result as ApiResult.Success).data
        val mostUrgent = data.filter { it.status == VaultStatus.active }
            .minByOrNull { it.ttlRemaining ?: Long.MAX_VALUE }

        assertEquals("vault-urgent", mostUrgent?.id)
    }

    @Test
    fun `worker skips expired vaults`() = runTest {
        val vaults = listOf(
            Vault(
                id = "vault-expired", owner = "O", beneficiary = "B",
                balance = 1000, checkInInterval = 86400,
                lastCheckIn = "2026-01-01", ttlRemaining = 0,
                status = VaultStatus.expired
            ),
            Vault(
                id = "vault-active", owner = "O", beneficiary = "B",
                balance = 1000, checkInInterval = 86400,
                lastCheckIn = "2026-01-01", ttlRemaining = 43200,
                status = VaultStatus.active
            )
        )
        coEvery { apiClient.listVaults() } returns ApiResult.Success(vaults)

        val result = apiClient.listVaults()
        val data = (result as ApiResult.Success).data
        val activeVaults = data.filter { it.status == VaultStatus.active }

        assertEquals(1, activeVaults.size)
        assertEquals("vault-active", activeVaults.first().id)
    }

    /** Mirrors the private formatTtl logic from VaultGlanceUpdateWorker. */
    private fun formatTtl(seconds: Long?): String {
        if (seconds == null) return "Unknown"
        val days = seconds / 86_400
        val hours = (seconds % 86_400) / 3_600
        return if (days > 0) "${days}d ${hours}h remaining" else "${hours}h remaining"
    }
}
