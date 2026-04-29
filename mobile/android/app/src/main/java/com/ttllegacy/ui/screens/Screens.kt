package com.ttllegacy.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.ttllegacy.models.Vault
import com.ttllegacy.ui.AuthViewModel
import com.ttllegacy.ui.VaultViewModel

// MARK: - Auth Screen

@Composable
fun AuthScreen(vm: AuthViewModel = hiltViewModel()) {
    val state by vm.state.collectAsStateWithLifecycle()
    val activity = LocalContext.current as android.app.Activity
    var showRegister by remember { mutableStateOf(false) }

    if (showRegister) {
        RegisterSheet(
            onRegister = { username -> vm.register(activity, username); showRegister = false },
            onDismiss = { showRegister = false }
        )
    }

    Column(
        modifier = Modifier.fillMaxSize().padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Icon(Icons.Default.Lock, contentDescription = null,
            modifier = Modifier.size(72.dp), tint = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.height(16.dp))
        Text("TTL-Legacy", style = MaterialTheme.typography.headlineLarge)
        Text("Secure digital inheritance", style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(32.dp))

        state.error?.let {
            Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            Spacer(Modifier.height(8.dp))
        }

        Button(
            onClick = { vm.signIn(activity) },
            modifier = Modifier.fillMaxWidth(),
            enabled = !state.isLoading
        ) {
            if (state.isLoading) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            else { Icon(Icons.Default.Key, null); Spacer(Modifier.width(8.dp)); Text("Sign in with Passkey") }
        }
        Spacer(Modifier.height(8.dp))
        TextButton(onClick = { showRegister = true }) { Text("Create account") }
    }
}

@Composable
private fun RegisterSheet(onRegister: (String) -> Unit, onDismiss: () -> Unit) {
    var username by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Create Account") },
        text = {
            OutlinedTextField(value = username, onValueChange = { username = it },
                label = { Text("Username") }, singleLine = true)
        },
        confirmButton = {
            TextButton(onClick = { onRegister(username) }, enabled = username.isNotBlank()) { Text("Register") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

// MARK: - Vault List Screen

@Composable
fun VaultListScreen(
    onVaultClick: (String) -> Unit,
    vm: VaultViewModel = hiltViewModel()
) {
    val state by vm.state.collectAsStateWithLifecycle()
    var showCreate by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) { vm.load() }

    if (showCreate) {
        CreateVaultDialog(
            onCreate = { ben, days -> vm.createVault(ben, days); showCreate = false },
            onDismiss = { showCreate = false }
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(title = { Text("My Vaults") }, actions = {
                IconButton(onClick = { showCreate = true }) { Icon(Icons.Default.Add, "Create vault") }
            })
        }
    ) { padding ->
        Box(Modifier.padding(padding).fillMaxSize()) {
            when {
                state.isLoading && state.vaults.isEmpty() ->
                    CircularProgressIndicator(Modifier.align(Alignment.Center))
                state.vaults.isEmpty() ->
                    Text("No vaults yet. Tap + to create one.",
                        Modifier.align(Alignment.Center),
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                else -> {
                    LazyColumn {
                        if (state.isOffline) item {
                            OfflineBanner()
                        }
                        state.error?.let { err -> item {
                            Text(err, color = MaterialTheme.colorScheme.error,
                                modifier = Modifier.padding(16.dp))
                        }}
                        items(state.vaults, key = { it.id }) { vault ->
                            VaultCard(vault = vault, onClick = { onVaultClick(vault.id) },
                                onCheckIn = { vm.checkIn(vault.id) })
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun OfflineBanner() {
    Surface(color = MaterialTheme.colorScheme.tertiaryContainer) {
        Row(Modifier.fillMaxWidth().padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Default.WifiOff, null, tint = MaterialTheme.colorScheme.onTertiaryContainer)
            Spacer(Modifier.width(8.dp))
            Text("Offline — showing cached data", color = MaterialTheme.colorScheme.onTertiaryContainer,
                style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
private fun VaultCard(vault: Vault, onClick: () -> Unit, onCheckIn: () -> Unit) {
    Card(onClick = onClick, modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp)) {
        Column(Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(vault.id.take(12) + "…", style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.weight(1f))
                StatusChip(vault.status)
            }
            Spacer(Modifier.height(4.dp))
            Text(vault.formattedBalance, style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            if (vault.isExpiringSoon) {
                Spacer(Modifier.height(4.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Default.Warning, null, tint = MaterialTheme.colorScheme.error,
                        modifier = Modifier.size(14.dp))
                    Spacer(Modifier.width(4.dp))
                    Text("Expiring soon!", color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.labelSmall)
                }
            }
            if (vault.status == com.ttllegacy.models.VaultStatus.active) {
                Spacer(Modifier.height(8.dp))
                OutlinedButton(onClick = onCheckIn, modifier = Modifier.fillMaxWidth()) {
                    Text("Check In")
                }
            }
        }
    }
}

@Composable
private fun StatusChip(status: com.ttllegacy.models.VaultStatus) {
    val (label, color) = when (status) {
        com.ttllegacy.models.VaultStatus.active -> "Active" to MaterialTheme.colorScheme.primary
        com.ttllegacy.models.VaultStatus.expired -> "Expired" to MaterialTheme.colorScheme.error
        com.ttllegacy.models.VaultStatus.released -> "Released" to MaterialTheme.colorScheme.secondary
        com.ttllegacy.models.VaultStatus.paused -> "Paused" to MaterialTheme.colorScheme.outline
    }
    SuggestionChip(onClick = {}, label = { Text(label, style = MaterialTheme.typography.labelSmall) },
        colors = SuggestionChipDefaults.suggestionChipColors(labelColor = color))
}

@Composable
private fun CreateVaultDialog(onCreate: (String, Int) -> Unit, onDismiss: () -> Unit) {
    var beneficiary by remember { mutableStateOf("") }
    var days by remember { mutableStateOf(30f) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("New Vault") },
        text = {
            Column {
                OutlinedTextField(value = beneficiary, onValueChange = { beneficiary = it },
                    label = { Text("Beneficiary address") }, singleLine = true,
                    modifier = Modifier.fillMaxWidth())
                Spacer(Modifier.height(12.dp))
                Text("Check-in interval: ${days.toInt()} days",
                    style = MaterialTheme.typography.bodySmall)
                Slider(value = days, onValueChange = { days = it }, valueRange = 1f..365f, steps = 363)
            }
        },
        confirmButton = {
            TextButton(onClick = { onCreate(beneficiary, days.toInt()) },
                enabled = beneficiary.isNotBlank()) { Text("Create") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}
