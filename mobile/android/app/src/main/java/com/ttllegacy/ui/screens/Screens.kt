package com.ttllegacy.ui.screens

import androidx.activity.ComponentActivity
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
import com.ttllegacy.models.TwoFactorMethod
import com.ttllegacy.models.TwoFactorStatus
import com.ttllegacy.models.Enable2FARequest
import com.ttllegacy.models.Verify2FARequest
import com.ttllegacy.services.BiometricHelper
import com.ttllegacy.services.VaultDeepLinkAction
import com.ttllegacy.ui.AuthViewModel
import com.ttllegacy.ui.VaultViewModel
import com.ttllegacy.ui.TwoFactorViewModel

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
    val context = LocalContext.current
    var showCreate by remember { mutableStateOf(false) }
    var pendingCheckIn by remember { mutableStateOf<Vault?>(null) }
    var biometricError by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) { vm.load() }

    if (showCreate) {
        CreateVaultDialog(
            onCreate = { ben, days -> vm.createVault(ben, days); showCreate = false },
            onDismiss = { showCreate = false }
        )
    }

    pendingCheckIn?.let { vault ->
        CheckInConfirmationDialog(
            vault = vault,
            onConfirm = {
                pendingCheckIn = null
                BiometricHelper(context as ComponentActivity).authenticate(
                    title = "Confirm Check-In",
                    subtitle = "Vault ${vault.id.take(12)}… will extend by ${formatInterval(vault.checkInInterval)}",
                    onSuccess = { vm.checkIn(vault.id) },
                    onError = { err -> biometricError = err },
                )
            },
            onDismiss = { pendingCheckIn = null },
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
                        val errorMsg = biometricError ?: state.error
                        errorMsg?.let { err ->
                            item {
                                Text(err, color = MaterialTheme.colorScheme.error,
                                    modifier = Modifier.padding(16.dp))
                            }
                        }
                        items(state.vaults, key = { it.id }) { vault ->
                            VaultCard(
                                vault = vault,
                                onClick = { onVaultClick(vault.id) },
                                onCheckIn = { pendingCheckIn = vault },
                            )
                        }
                    }
                }
            }
        }
    }
}

private fun formatInterval(seconds: Long): String {
    val days = seconds / 86_400
    return if (days > 0) "$days day${if (days == 1L) "" else "s"}" else "${seconds / 3_600}h"
}

@Composable
private fun CheckInConfirmationDialog(vault: Vault, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Confirm Check-In") },
        text = {
            Column {
                Text("Vault: ${vault.id.take(12)}…",
                    style = MaterialTheme.typography.bodyMedium)
                Spacer(Modifier.height(4.dp))
                Text("TTL will be extended by ${formatInterval(vault.checkInInterval)}.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(4.dp))
                Text("Biometric or PIN confirmation is required.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        },
        confirmButton = {
            TextButton(onClick = onConfirm) { Text("Confirm") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
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

// MARK: - Beneficiary Acceptance Screen

@Composable
fun BeneficiaryAcceptanceScreen(
    vaultId: String,
    onAccepted: () -> Unit,
    onDecline: () -> Unit,
    vm: AcceptanceViewModel = hiltViewModel()
) {
    val state by vm.state.collectAsStateWithLifecycle()

    LaunchedEffect(state.isAccepted) {
        if (state.isAccepted) onAccepted()
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(32.dp),
        verticalArrangement = Arrangement.Center
    ) {
        Icon(
            Icons.Default.Lock,
            contentDescription = null,
            modifier = Modifier.size(56.dp),
            tint = MaterialTheme.colorScheme.primary
        )
        Spacer(Modifier.height(16.dp))
        Text("Beneficiary Acceptance", style = MaterialTheme.typography.headlineSmall)
        Spacer(Modifier.height(8.dp))
        Text(
            "You have been named as the beneficiary for the following vault:",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.height(12.dp))
        OutlinedTextField(
            value = vaultId,
            onValueChange = {},
            label = { Text("Vault ID") },
            readOnly = true,
            modifier = Modifier.fillMaxWidth()
        )
        state.error?.let {
            Spacer(Modifier.height(8.dp))
            Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        }
        Spacer(Modifier.height(24.dp))
        Button(
            onClick = { vm.accept(vaultId) },
            modifier = Modifier.fillMaxWidth(),
            enabled = !state.isLoading
        ) {
            if (state.isLoading) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            else Text("Accept")
        }
        Spacer(Modifier.height(8.dp))
        OutlinedButton(
            onClick = onDecline,
            modifier = Modifier.fillMaxWidth(),
            enabled = !state.isLoading
        ) {
            Text("Decline")
        }
    }
}

// MARK: - Vault Deep Link Screen

@Composable
fun VaultDeepLinkScreen(
    vaultId: String,
    actionPath: String,
    onDone: () -> Unit,
    vm: VaultViewModel = hiltViewModel()
) {
    val action = VaultDeepLinkAction.fromPathSegment(actionPath)
    val state by vm.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    var isProcessing by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) { vm.load() }

    val vault = state.vaults.find { it.id == vaultId }
    val (title, description) = when (action) {
        VaultDeepLinkAction.CHECK_IN -> "Check In" to "Confirm check-in for vault ${vaultId.take(12)}…"
        VaultDeepLinkAction.WITHDRAW -> "Withdraw" to "Withdraw funds from vault ${vaultId.take(12)}…"
        VaultDeepLinkAction.VIEW_DETAILS -> "Vault Details" to "View details for vault ${vaultId.take(12)}…"
        VaultDeepLinkAction.MANAGE_BENEFICIARY -> "Manage Beneficiary" to "Update beneficiary for vault ${vaultId.take(12)}…"
        null -> "Vault Link" to "Unrecognised vault action."
    }

    if (action == VaultDeepLinkAction.VIEW_DETAILS && vault != null) {
        Column(Modifier.fillMaxSize().padding(16.dp)) {
            Text(title, style = MaterialTheme.typography.headlineSmall)
            Spacer(Modifier.height(12.dp))
            VaultCard(vault = vault, onClick = {}, onCheckIn = {})
            Spacer(Modifier.height(16.dp))
            OutlinedButton(onClick = onDone, modifier = Modifier.fillMaxWidth()) { Text("Done") }
        }
        return
    }

    val displayError = error ?: if (action == VaultDeepLinkAction.VIEW_DETAILS && vault == null) {
        "Vault not found"
    } else {
        null
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(32.dp),
        verticalArrangement = Arrangement.Center
    ) {
        Text(title, style = MaterialTheme.typography.headlineSmall)
        Spacer(Modifier.height(8.dp))
        Text(description, style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        displayError?.let {
            Spacer(Modifier.height(8.dp))
            Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        }
        Spacer(Modifier.height(24.dp))
        when (action) {
            VaultDeepLinkAction.CHECK_IN -> {
                Button(
                    onClick = {
                        if (vault == null) {
                            error = "Vault not found"
                            return@Button
                        }
                        isProcessing = true
                        error = null
                        BiometricHelper(context as ComponentActivity).authenticate(
                            title = "Confirm Check-In",
                            subtitle = "Vault ${vault.id.take(12)}…",
                            onSuccess = {
                                vm.checkIn(vault.id)
                                isProcessing = false
                                onDone()
                            },
                            onError = { err ->
                                error = err
                                isProcessing = false
                            }
                        )
                    },
                    modifier = Modifier.fillMaxWidth(),
                    enabled = !isProcessing && vault != null
                ) { Text(if (isProcessing) "Processing…" else "Check In") }
            }
            VaultDeepLinkAction.WITHDRAW -> {
                Button(
                    onClick = { error = "Withdrawal is not yet available in the mobile app." },
                    modifier = Modifier.fillMaxWidth()
                ) { Text("Withdraw") }
            }
            VaultDeepLinkAction.MANAGE_BENEFICIARY -> {
                Button(
                    onClick = { error = "Beneficiary management is not yet available in the mobile app." },
                    modifier = Modifier.fillMaxWidth()
                ) { Text("Manage Beneficiary") }
            }
            VaultDeepLinkAction.VIEW_DETAILS, null -> Unit
        }
        Spacer(Modifier.height(8.dp))
        OutlinedButton(onClick = onDone, modifier = Modifier.fillMaxWidth()) { Text("Done") }
    }
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

// MARK: - 2FA Screens

@Composable
fun TwoFactorSetupScreen(
    vaultId: String,
    onComplete: () -> Unit,
    onDismiss: () -> Unit
) {
    val vm: TwoFactorViewModel = hiltViewModel()
    val state by vm.state.collectAsStateWithLifecycle()
    var selectedMethod by remember { mutableStateOf(TwoFactorMethod.totp) }
    var phone by remember { mutableStateOf("") }
    var email by remember { mutableStateOf("") }

    if (state.setupResponse != null) {
        TwoFactorVerifyScreen(
            vaultId = vaultId,
            method = selectedMethod,
            provisioningUri = state.setupResponse?.provisioningUri,
            onVerified = { onComplete() },
            onDismiss = onDismiss,
            vm = vm
        )
        return
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Enable 2FA") },
        text = {
            Column {
                Text("Authentication Method", style = MaterialTheme.typography.labelLarge)
                Spacer(Modifier.height(8.dp))
                listOf(TwoFactorMethod.totp, TwoFactorMethod.sms, TwoFactorMethod.email).forEach { method ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        RadioButton(
                            selected = selectedMethod == method,
                            onClick = { selectedMethod = method }
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(
                            when (method) {
                                TwoFactorMethod.totp -> "Authenticator App (TOTP)"
                                TwoFactorMethod.sms -> "SMS Code"
                                TwoFactorMethod.email -> "Email Code"
                            },
                            style = MaterialTheme.typography.bodyMedium
                        )
                    }
                }
                if (selectedMethod == TwoFactorMethod.sms) {
                    Spacer(Modifier.height(8.dp))
                    OutlinedTextField(value = phone, onValueChange = { phone = it },
                        label = { Text("Phone number") }, singleLine = true,
                        modifier = Modifier.fillMaxWidth())
                }
                if (selectedMethod == TwoFactorMethod.email) {
                    Spacer(Modifier.height(8.dp))
                    OutlinedTextField(value = email, onValueChange = { email = it },
                        label = { Text("Email address") }, singleLine = true,
                        modifier = Modifier.fillMaxWidth())
                }
                state.error?.let {
                    Spacer(Modifier.height(8.dp))
                    Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    vm.enable2FA(vaultId, selectedMethod, phone, email)
                },
                enabled = !state.isLoading && when (selectedMethod) {
                    TwoFactorMethod.totp -> true
                    TwoFactorMethod.sms -> phone.isNotBlank()
                    TwoFactorMethod.email -> email.isNotBlank()
                }
            ) {
                if (state.isLoading) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                else Text("Continue")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } }
    )
}

@Composable
private fun TwoFactorVerifyScreen(
    vaultId: String,
    method: TwoFactorMethod,
    provisioningUri: String?,
    onVerified: () -> Unit,
    onDismiss: () -> Unit,
    vm: TwoFactorViewModel
) {
    val state by vm.state.collectAsStateWithLifecycle()
    var otp by remember { mutableStateOf("") }

    LaunchedEffect(state.verified) {
        if (state.verified) onVerified()
    }

    Column(
        modifier = Modifier.fillMaxSize().padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Icon(
            when (method) {
                TwoFactorMethod.totp -> Icons.Default.Lock
                TwoFactorMethod.sms -> Icons.Default.Email
                TwoFactorMethod.email -> Icons.Default.Email
            },
            contentDescription = null, modifier = Modifier.size(56.dp),
            tint = MaterialTheme.colorScheme.primary
        )
        Spacer(Modifier.height(16.dp))
        Text("Verify Setup", style = MaterialTheme.typography.headlineSmall)
        Spacer(Modifier.height(8.dp))
        when (method) {
            TwoFactorMethod.totp -> {
                Text("Scan the URI in your authenticator app:",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (provisioningUri != null) {
                    Spacer(Modifier.height(8.dp))
                    Text(provisioningUri, style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            TwoFactorMethod.sms -> Text("A verification code has been sent to your phone.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            TwoFactorMethod.email -> Text("A verification code has been sent to your email.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Spacer(Modifier.height(16.dp))
        OutlinedTextField(
            value = otp, onValueChange = { otp = it },
            label = { Text("6-digit code") }, singleLine = true,
            modifier = Modifier.width(200.dp),
            textStyle = MaterialTheme.typography.headlineSmall
        )
        state.error?.let {
            Spacer(Modifier.height(8.dp))
            Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        }
        Spacer(Modifier.height(16.dp))
        Button(
            onClick = { vm.verify2FA(vaultId, otp) },
            modifier = Modifier.fillMaxWidth(),
            enabled = otp.length == 6 && !state.isLoading
        ) {
            if (state.isLoading) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            else Text("Verify")
        }
    }
}
