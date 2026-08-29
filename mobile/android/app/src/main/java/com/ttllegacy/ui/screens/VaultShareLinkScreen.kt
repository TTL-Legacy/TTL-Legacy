package com.ttllegacy.ui.screens

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.ttllegacy.models.Vault
import com.ttllegacy.models.VaultStatus

/**
 * A read-only vault summary screen designed to be presented as a bottom sheet or
 * full-screen dialog. It lets the owner share a preview link for the vault with
 * a beneficiary or other party.
 *
 * The preview URL `https://ttl-legacy.app/vaults/{vault.id}/preview` renders a
 * public, read-only view — no vault actions can be performed via the link.
 *
 * @param vault     The vault whose details will be shown.
 * @param onDismiss Called when the user taps the close button or completes sharing.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VaultShareLinkScreen(vault: Vault, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val previewUrl = "https://ttl-legacy.app/vaults/${vault.id}/preview"
    var copiedToClipboard by remember { mutableStateOf(false) }

    // Reset the "Copied!" label after 2 seconds.
    LaunchedEffect(copiedToClipboard) {
        if (copiedToClipboard) {
            kotlinx.coroutines.delay(2_000)
            copiedToClipboard = false
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Share Vault Preview") },
                navigationIcon = {
                    IconButton(onClick = onDismiss) {
                        Icon(Icons.Default.Close, contentDescription = "Close")
                    }
                }
            )
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(horizontal = 16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            Spacer(Modifier.height(8.dp))

            // ── Vault Summary Card ──────────────────────────────────────────
            Surface(
                shape = RoundedCornerShape(12.dp),
                tonalElevation = 2.dp,
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    Text(
                        text = "Vault Summary",
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )

                    HorizontalDivider()

                    VaultSummaryRow(label = "Vault ID", value = vault.id, monospace = true)
                    VaultSummaryRow(label = "Owner", value = vault.owner, monospace = true)
                    VaultSummaryRow(label = "Beneficiary", value = vault.beneficiary, monospace = true)
                    VaultSummaryRow(label = "Balance", value = vault.formattedBalance)

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = "Status",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        VaultStatusChip(status = vault.status)
                    }

                    vault.ttlRemaining?.let { ttl ->
                        VaultSummaryRow(label = "TTL Remaining", value = formatTtlDuration(ttl))
                    }
                }
            }

            // ── Share Actions ───────────────────────────────────────────────
            Button(
                onClick = { sharePreviewLink(context, previewUrl) },
                modifier = Modifier.fillMaxWidth()
            ) {
                Icon(Icons.Default.Share, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text("Share Preview Link")
            }

            OutlinedButton(
                onClick = {
                    copyToClipboard(context, previewUrl)
                    copiedToClipboard = true
                },
                modifier = Modifier.fillMaxWidth()
            ) {
                Icon(Icons.Default.ContentCopy, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text(if (copiedToClipboard) "Copied!" else "Copy Link")
            }

            // ── Disclaimer ─────────────────────────────────────────────────
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(
                        color = MaterialTheme.colorScheme.surfaceVariant,
                        shape = RoundedCornerShape(8.dp)
                    )
                    .padding(12.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(
                    text = "ℹ",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Text(
                    text = "This is a read-only preview. No vault actions can be performed via this link.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Spacer(Modifier.height(16.dp))
        }
    }
}

// ── Private helpers ─────────────────────────────────────────────────────────

@Composable
private fun VaultSummaryRow(label: String, value: String, monospace: Boolean = false) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.Top
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(0.35f)
        )
        Text(
            text = value,
            style = if (monospace)
                MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace)
            else MaterialTheme.typography.bodyMedium,
            modifier = Modifier.weight(0.65f),
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.End
        )
    }
}

@Composable
private fun VaultStatusChip(status: VaultStatus) {
    val (label, containerColor) = when (status) {
        VaultStatus.active   -> "Active"   to MaterialTheme.colorScheme.primaryContainer
        VaultStatus.expired  -> "Expired"  to MaterialTheme.colorScheme.errorContainer
        VaultStatus.released -> "Released" to MaterialTheme.colorScheme.secondaryContainer
        VaultStatus.paused   -> "Paused"   to MaterialTheme.colorScheme.surfaceVariant
    }
    Surface(
        shape = RoundedCornerShape(50),
        color = containerColor,
        modifier = Modifier.wrapContentSize()
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp)
        )
    }
}

private fun sharePreviewLink(context: Context, url: String) {
    val intent = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_SUBJECT, "TTL-Legacy Vault Preview")
        putExtra(Intent.EXTRA_TEXT, "View my vault on TTL-Legacy: $url")
    }
    context.startActivity(Intent.createChooser(intent, "Share Vault Preview"))
}

private fun copyToClipboard(context: Context, url: String) {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    val clip = ClipData.newPlainText("Vault Preview Link", url)
    clipboard.setPrimaryClip(clip)
}

private fun formatTtlDuration(seconds: Long): String {
    val days = seconds / 86_400
    val hours = (seconds % 86_400) / 3_600
    return if (days > 0) "${days}d ${hours}h" else "${hours}h"
}
