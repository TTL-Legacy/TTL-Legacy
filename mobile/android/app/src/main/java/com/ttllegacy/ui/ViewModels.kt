package com.ttllegacy.ui

import android.app.Activity
import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.ttllegacy.api.ApiClient
import com.ttllegacy.api.ApiResult
import com.ttllegacy.api.TokenProvider
import com.ttllegacy.models.CreateVaultRequest
import com.ttllegacy.models.Vault
import com.ttllegacy.services.CheckInSyncWorker
import com.ttllegacy.services.NotificationHelper
import com.ttllegacy.services.PasskeyService
import com.ttllegacy.services.PendingCheckIn
import com.ttllegacy.services.PendingCheckInDao
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

// --- Auth ViewModel ---

data class AuthUiState(
    val isAuthenticated: Boolean = false,
    val isLoading: Boolean = false,
    val error: String? = null
)

@HiltViewModel
class AuthViewModel @Inject constructor(
    private val passkeyService: PasskeyService,
    private val tokenProvider: TokenProvider
) : ViewModel() {

    private val _state = MutableStateFlow(AuthUiState(isAuthenticated = tokenProvider.token != null))
    val state = _state.asStateFlow()

    fun signIn(activity: Activity) = viewModelScope.launch {
        _state.update { it.copy(isLoading = true, error = null) }
        passkeyService.authenticate(activity)
            .onSuccess { _state.update { it.copy(isAuthenticated = true, isLoading = false) } }
            .onFailure { e -> _state.update { it.copy(isLoading = false, error = e.message) } }
    }

    fun register(activity: Activity, username: String) = viewModelScope.launch {
        _state.update { it.copy(isLoading = true, error = null) }
        passkeyService.register(activity, username)
            .onSuccess { signIn(activity) }
            .onFailure { e -> _state.update { it.copy(isLoading = false, error = e.message) } }
    }

    fun signOut() {
        tokenProvider.clear()
        _state.update { it.copy(isAuthenticated = false) }
    }
}

// --- Vault ViewModel ---

data class VaultUiState(
    val vaults: List<Vault> = emptyList(),
    val isLoading: Boolean = false,
    val error: String? = null,
    val isOffline: Boolean = false
)

@HiltViewModel
class VaultViewModel @Inject constructor(
    private val apiClient: ApiClient,
    private val notificationHelper: NotificationHelper,
    private val pendingCheckInDao: PendingCheckInDao,
    @ApplicationContext private val context: Context
) : ViewModel() {

    private val _state = MutableStateFlow(VaultUiState())
    val state = _state.asStateFlow()

    fun load() = viewModelScope.launch {
        _state.update { it.copy(isLoading = true, error = null) }
        when (val result = apiClient.listVaults()) {
            is ApiResult.Success -> {
                _state.update { it.copy(vaults = result.data, isLoading = false, isOffline = false) }
            }
            ApiResult.NetworkUnavailable -> {
                _state.update { it.copy(isLoading = false, isOffline = true) }
            }
            is ApiResult.Error -> {
                _state.update { it.copy(isLoading = false, error = result.message) }
            }
        }
    }

    fun checkIn(vaultId: String) = viewModelScope.launch {
        when (val result = apiClient.checkIn(vaultId)) {
            is ApiResult.Success -> load()
            is ApiResult.Error -> _state.update { it.copy(error = result.message) }
            ApiResult.NetworkUnavailable -> {
                pendingCheckInDao.insert(PendingCheckIn(vaultId = vaultId, queuedAt = System.currentTimeMillis()))
                val queued = pendingCheckInDao.getAll()
                notificationHelper.showQueuedCheckIn(queued.size)
                CheckInSyncWorker.schedule(context)
                _state.update { it.copy(error = "Offline — check-in queued and will retry automatically") }
            }
        }
    }

    fun createVault(beneficiary: String, intervalDays: Int) = viewModelScope.launch {
        val req = CreateVaultRequest(beneficiary, intervalDays * 86_400L)
        when (val result = apiClient.createVault(req)) {
            is ApiResult.Success -> load()
            is ApiResult.Error -> _state.update { it.copy(error = result.message) }
            ApiResult.NetworkUnavailable -> _state.update { it.copy(error = "No network") }
        }
    }
}

// --- Acceptance ViewModel ---

data class AcceptanceUiState(
    val isLoading: Boolean = false,
    val isAccepted: Boolean = false,
    val error: String? = null
)

@HiltViewModel
class AcceptanceViewModel @Inject constructor(
    private val apiClient: ApiClient
) : ViewModel() {

    private val _state = MutableStateFlow(AcceptanceUiState())
    val state = _state.asStateFlow()

    fun accept(vaultId: String) = viewModelScope.launch {
        _state.update { it.copy(isLoading = true, error = null) }
        when (val result = apiClient.acceptBeneficiary(vaultId)) {
            is ApiResult.Success -> _state.update { it.copy(isLoading = false, isAccepted = true) }
            is ApiResult.Error -> _state.update { it.copy(isLoading = false, error = result.message) }
            ApiResult.NetworkUnavailable -> _state.update { it.copy(isLoading = false, error = "No network. Please try again.") }
        }
    }
}
