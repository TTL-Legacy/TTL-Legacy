package com.ttllegacy.ui

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.ttllegacy.services.VaultDeepLink
import com.ttllegacy.services.VaultDeepLinkParser
import com.ttllegacy.ui.screens.AuthScreen
import com.ttllegacy.ui.screens.BeneficiaryAcceptanceScreen
import com.ttllegacy.ui.screens.VaultDeepLinkScreen
import com.ttllegacy.ui.screens.VaultListScreen
import com.ttllegacy.ui.theme.TTLLegacyTheme
import dagger.hilt.android.AndroidEntryPoint

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    private var pendingBeneficiaryAcceptVaultId by mutableStateOf<String?>(null)
    private var pendingVaultDeepLink by mutableStateOf<VaultDeepLink?>(null)
    private var showPermissionRationale by mutableStateOf(false)

    private val notificationPermissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { /* result handled gracefully — denial does not break the app */ }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        handleIncomingIntent(intent)

        setContent {
            TTLLegacyTheme {
                NotificationPermissionEffect(
                    showRationale = showPermissionRationale,
                    onRationaleShown = { showPermissionRationale = false },
                    onRequestPermission = {
                        showPermissionRationale = false
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                            notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                        }
                    }
                )
                AppNavigation(
                    beneficiaryAcceptVaultId = pendingBeneficiaryAcceptVaultId,
                    vaultDeepLink = pendingVaultDeepLink,
                    onBeneficiaryAcceptConsumed = { pendingBeneficiaryAcceptVaultId = null },
                    onVaultDeepLinkConsumed = { pendingVaultDeepLink = null }
                )
            }
        }

        requestNotificationPermissionIfNeeded()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIncomingIntent(intent)
    }

    private fun handleIncomingIntent(intent: Intent) {
        intent.data?.let { uri ->
            VaultDeepLinkParser.parse(uri)?.let {
                pendingVaultDeepLink = it
                pendingBeneficiaryAcceptVaultId = null
                return
            }
        }
        extractBeneficiaryAcceptVaultId(intent)?.let {
            pendingBeneficiaryAcceptVaultId = it
            pendingVaultDeepLink = null
        }
    }

    private fun extractBeneficiaryAcceptVaultId(intent: Intent): String? =
        intent.data
            ?.takeIf { it.scheme == "https" && it.host == "ttl-legacy.app" && it.path == "/accept" }
            ?.getQueryParameter("vault_id")

    private fun requestNotificationPermissionIfNeeded() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        when {
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED -> Unit
            shouldShowRequestPermissionRationale(Manifest.permission.POST_NOTIFICATIONS) ->
                showPermissionRationale = true
            else ->
                notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
}

@Composable
private fun NotificationPermissionEffect(
    showRationale: Boolean,
    onRationaleShown: () -> Unit,
    onRequestPermission: () -> Unit
) {
    if (showRationale) {
        AlertDialog(
            onDismissRequest = onRationaleShown,
            title = { Text("Stay Notified") },
            text = {
                Text(
                    "TTL-Legacy needs notification permission to alert you before your vault " +
                    "expires and remind you to check in. Without it, reminders will not be delivered."
                )
            },
            confirmButton = {
                TextButton(onClick = onRequestPermission) { Text("Allow") }
            },
            dismissButton = {
                TextButton(onClick = onRationaleShown) { Text("Not now") }
            }
        )
    }
}

@Composable
private fun AppNavigation(
    beneficiaryAcceptVaultId: String?,
    vaultDeepLink: VaultDeepLink?,
    onBeneficiaryAcceptConsumed: () -> Unit,
    onVaultDeepLinkConsumed: () -> Unit
) {
    val navController = rememberNavController()
    val authVm: AuthViewModel = hiltViewModel()
    val authState by authVm.state.collectAsStateWithLifecycle()

    LaunchedEffect(authState.isAuthenticated) {
        if (authState.isAuthenticated) navController.navigate("vaults") { popUpTo("auth") { inclusive = true } }
        else navController.navigate("auth") { popUpTo("vaults") { inclusive = true } }
    }

    LaunchedEffect(beneficiaryAcceptVaultId, authState.isAuthenticated) {
        if (beneficiaryAcceptVaultId != null && authState.isAuthenticated) {
            navController.navigate("accept/$beneficiaryAcceptVaultId")
            onBeneficiaryAcceptConsumed()
        }
    }

    LaunchedEffect(vaultDeepLink, authState.isAuthenticated) {
        if (vaultDeepLink != null && authState.isAuthenticated) {
            navController.navigate(
                "vault/${vaultDeepLink.vaultId}/${vaultDeepLink.action.pathSegment}"
            )
            onVaultDeepLinkConsumed()
        }
    }

    NavHost(navController, startDestination = if (authState.isAuthenticated) "vaults" else "auth") {
        composable("auth") { AuthScreen(vm = authVm) }
        composable("vaults") {
            VaultListScreen(onVaultClick = { /* navigate to detail */ })
        }
        composable("accept/{vaultId}") { backStack ->
            val vaultId = backStack.arguments?.getString("vaultId") ?: return@composable
            BeneficiaryAcceptanceScreen(
                vaultId = vaultId,
                onAccepted = { navController.popBackStack() },
                onDecline = { navController.popBackStack() }
            )
        }
        composable("vault/{vaultId}/{action}") { backStack ->
            val vaultId = backStack.arguments?.getString("vaultId") ?: return@composable
            val action = backStack.arguments?.getString("action") ?: return@composable
            VaultDeepLinkScreen(
                vaultId = vaultId,
                actionPath = action,
                onDone = { navController.popBackStack() }
            )
        }
    }
}
