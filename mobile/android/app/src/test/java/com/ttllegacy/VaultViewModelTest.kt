package com.ttllegacy

import com.ttllegacy.api.ApiResult
import com.ttllegacy.models.Vault
import com.ttllegacy.models.VaultStatus
import com.ttllegacy.ui.VaultUiState
import com.ttllegacy.ui.VaultViewModel
import com.ttllegacy.api.ApiClient
import com.ttllegacy.services.NotificationHelper
import io.mockk.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.*
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class VaultViewModelTest {

    private val testDispatcher = UnconfinedTestDispatcher()
    private val apiClient: ApiClient = mockk()
    private val notificationHelper: NotificationHelper = mockk(relaxed = true)
    private lateinit var vm: VaultViewModel

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
        vm = VaultViewModel(apiClient, notificationHelper)
    }

    @After
    fun teardown() { Dispatchers.resetMain() }

    @Test
    fun `load success updates vaults`() = runTest {
        val vaults = listOf(makeVault("v1"), makeVault("v2"))
        coEvery { apiClient.listVaults() } returns ApiResult.Success(vaults)

        vm.load()

        assertEquals(vaults, vm.state.value.vaults)
        assertFalse(vm.state.value.isLoading)
        assertNull(vm.state.value.error)
    }

    @Test
    fun `load network unavailable sets offline flag`() = runTest {
        coEvery { apiClient.listVaults() } returns ApiResult.NetworkUnavailable

        vm.load()

        assertTrue(vm.state.value.isOffline)
        assertFalse(vm.state.value.isLoading)
    }

    @Test
    fun `load error sets error message`() = runTest {
        coEvery { apiClient.listVaults() } returns ApiResult.Error("Server error", 500)

        vm.load()

        assertEquals("Server error", vm.state.value.error)
        assertFalse(vm.state.value.isLoading)
    }

    @Test
    fun `checkIn success reloads vaults`() = runTest {
        val vaults = listOf(makeVault("v1"))
        coEvery { apiClient.checkIn("v1") } returns ApiResult.Success(Unit)
        coEvery { apiClient.listVaults() } returns ApiResult.Success(vaults)

        vm.checkIn("v1")

        coVerify { apiClient.checkIn("v1") }
        coVerify { apiClient.listVaults() }
    }

    @Test
    fun `checkIn network unavailable sets error`() = runTest {
        coEvery { apiClient.checkIn("v1") } returns ApiResult.NetworkUnavailable

        vm.checkIn("v1")

        assertNotNull(vm.state.value.error)
    }

    private fun makeVault(id: String) = Vault(
        id = id, owner = "GABC", beneficiary = "GXYZ",
        balance = 10_000_000L, checkInInterval = 2_592_000L,
        lastCheckIn = "2026-04-01T00:00:00Z", ttlRemaining = 172_800L,
        status = VaultStatus.active
    )
}
