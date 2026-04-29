package com.ttllegacy.ui

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.*
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.ttllegacy.ui.screens.AuthScreen
import com.ttllegacy.ui.screens.VaultListScreen
import com.ttllegacy.ui.theme.TTLLegacyTheme
import dagger.hilt.android.AndroidEntryPoint

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            TTLLegacyTheme {
                AppNavigation()
            }
        }
    }
}

@Composable
private fun AppNavigation() {
    val navController = rememberNavController()
    val authVm: AuthViewModel = hiltViewModel()
    val authState by authVm.state.collectAsStateWithLifecycle()

    LaunchedEffect(authState.isAuthenticated) {
        if (authState.isAuthenticated) navController.navigate("vaults") { popUpTo("auth") { inclusive = true } }
        else navController.navigate("auth") { popUpTo("vaults") { inclusive = true } }
    }

    NavHost(navController, startDestination = if (authState.isAuthenticated) "vaults" else "auth") {
        composable("auth") { AuthScreen(vm = authVm) }
        composable("vaults") {
            VaultListScreen(onVaultClick = { /* navigate to detail */ })
        }
    }
}
