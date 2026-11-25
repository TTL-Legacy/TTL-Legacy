package com.handsoff.vault

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** A single vault entry shown in the vault list. */
data class Vault(
    val id: String,
    val name: String,
    val balance: String,
)

/** Immutable UI state consumed by [VaultListScreen]. */
data class VaultListUiState(
    val isLoading: Boolean = false,
    val vaults: List<Vault> = emptyList(),
    val errorMessage: String? = null,
)

/**
 * Supplies vault list state to [VaultListScreen].
 *
 * The repository is injected so the ViewModel can be exercised with a fake in tests.
 */
class VaultListViewModel(
    private val repository: VaultRepository,
) : ViewModel() {

    private val _uiState = MutableStateFlow(VaultListUiState(isLoading = true))
    val uiState: StateFlow<VaultListUiState> = _uiState.asStateFlow()

    init {
        loadVaults()
    }

    fun loadVaults() {
        _uiState.update { it.copy(isLoading = true, errorMessage = null) }
        viewModelScope.launch {
            runCatching { repository.getVaults() }
                .onSuccess { vaults ->
                    _uiState.update {
                        it.copy(isLoading = false, vaults = vaults, errorMessage = null)
                    }
                }
                .onFailure { error ->
                    _uiState.update {
                        it.copy(
                            isLoading = false,
                            errorMessage = error.message ?: "Failed to load vaults",
                        )
                    }
                }
        }
    }
}

/** Source of vault data for [VaultListViewModel]. */
interface VaultRepository {
    suspend fun getVaults(): List<Vault>
}
