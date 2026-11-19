package com.handsoff.vault

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle

/**
 * Compose screen that renders the list of vaults supplied by [VaultListViewModel].
 *
 * The screen is intentionally stateless: all state is owned by the ViewModel and
 * observed here, which keeps the composable easy to preview and test.
 */
@Composable
fun VaultListScreen(
    viewModel: VaultListViewModel,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    VaultListScreen(state = state, modifier = modifier)
}

@Composable
fun VaultListScreen(
    state: VaultListUiState,
    modifier: Modifier = Modifier,
) {
    when {
        state.isLoading -> {
            Column(
                modifier = modifier
                    .fillMaxSize()
                    .testTag(VaultListTestTags.LOADING),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                CircularProgressIndicator()
            }
        }

        state.errorMessage != null -> {
            Column(
                modifier = modifier
                    .fillMaxSize()
                    .padding(16.dp)
                    .testTag(VaultListTestTags.ERROR),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Text(
                    text = state.errorMessage,
                    style = MaterialTheme.typography.bodyLarge,
                )
            }
        }

        state.vaults.isEmpty() -> {
            Column(
                modifier = modifier
                    .fillMaxSize()
                    .padding(16.dp)
                    .testTag(VaultListTestTags.EMPTY),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Text(
                    text = "No vaults yet",
                    style = MaterialTheme.typography.bodyLarge,
                )
            }
        }

        else -> {
            LazyColumn(
                modifier = modifier
                    .fillMaxSize()
                    .testTag(VaultListTestTags.LIST),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(state.vaults, key = { it.id }) { vault ->
                    VaultListItem(vault = vault)
                }
            }
        }
    }
}

@Composable
private fun VaultListItem(
    vault: Vault,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .testTag(VaultListTestTags.item(vault.id)),
    ) {
        Text(
            text = vault.name,
            style = MaterialTheme.typography.titleMedium,
        )
        Text(
            text = vault.balance,
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

/** Test tags shared between the screen and its instrumentation/unit tests. */
object VaultListTestTags {
    const val LOADING = "vault_list_loading"
    const val ERROR = "vault_list_error"
    const val EMPTY = "vault_list_empty"
    const val LIST = "vault_list"

    fun item(id: String): String = "vault_list_item_$id"
}
