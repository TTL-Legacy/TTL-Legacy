package com.ttllegacy.widget

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.action.actionStartActivity
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.provideContent
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Column
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.hilt.work.HiltWorker
import androidx.work.*
import com.ttllegacy.api.ApiClient
import com.ttllegacy.api.ApiResult
import com.ttllegacy.ui.MainActivity
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import java.util.concurrent.TimeUnit

/** Data holder persisted in SharedPreferences between refreshes. */
private const val GLANCE_PREFS = "vault_glance_widget_prefs"
private const val KEY_VAULT_NAME = "glance_vault_name"
private const val KEY_TTL = "glance_ttl_remaining"
private const val KEY_EXPIRING = "glance_expiring_soon"

private fun saveWidgetData(context: Context, vaultName: String, ttl: String, expiringSoon: Boolean) {
    context.getSharedPreferences(GLANCE_PREFS, Context.MODE_PRIVATE).edit()
        .putString(KEY_VAULT_NAME, vaultName)
        .putString(KEY_TTL, ttl)
        .putBoolean(KEY_EXPIRING, expiringSoon)
        .apply()
}

private fun loadWidgetData(context: Context): Triple<String, String, Boolean> {
    val prefs = context.getSharedPreferences(GLANCE_PREFS, Context.MODE_PRIVATE)
    return Triple(
        prefs.getString(KEY_VAULT_NAME, "—") ?: "—",
        prefs.getString(KEY_TTL, "Unknown") ?: "Unknown",
        prefs.getBoolean(KEY_EXPIRING, false)
    )
}

/** Glance widget that shows the most urgent vault's TTL countdown. */
class VaultGlanceWidget : GlanceAppWidget() {

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val (vaultName, ttl, expiringSoon) = loadWidgetData(context)
        provideContent {
            VaultGlanceWidgetContent(vaultName = vaultName, ttl = ttl, expiringSoon = expiringSoon)
        }
    }
}

@Composable
internal fun VaultGlanceWidgetContent(vaultName: String, ttl: String, expiringSoon: Boolean) {
    val accentColor = if (expiringSoon) Color(0xFFED6C02) else Color(0xFF1565C0)
    Column(
        modifier = GlanceModifier
            .fillMaxSize()
            .background(Color.White)
            .padding(12.dp)
            .clickable(actionStartActivity<MainActivity>()),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = "TTL-Legacy",
            style = TextStyle(
                color = androidx.glance.unit.ColorProvider(accentColor),
                fontSize = 11.sp,
                fontWeight = FontWeight.Bold
            )
        )
        Spacer(modifier = GlanceModifier.height(4.dp))
        Text(
            text = vaultName,
            style = TextStyle(fontSize = 14.sp, fontWeight = FontWeight.Bold)
        )
        Spacer(modifier = GlanceModifier.height(2.dp))
        Text(
            text = ttl,
            style = TextStyle(
                color = androidx.glance.unit.ColorProvider(accentColor),
                fontSize = 12.sp
            )
        )
        if (expiringSoon) {
            Spacer(modifier = GlanceModifier.height(2.dp))
            Text(
                text = "⚠ Expiring soon",
                style = TextStyle(
                    color = androidx.glance.unit.ColorProvider(Color(0xFFED6C02)),
                    fontSize = 11.sp
                )
            )
        }
    }
}

/** BroadcastReceiver that links Android to [VaultGlanceWidget]. */
class VaultGlanceWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = VaultGlanceWidget()
}

/** WorkManager worker that fetches vault data and updates the Glance widget. */
@HiltWorker
class VaultGlanceUpdateWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val apiClient: ApiClient
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val result = apiClient.listVaults()
        if (result is ApiResult.Success) {
            val vault = result.data
                .filter { it.status == com.ttllegacy.models.VaultStatus.active }
                .minByOrNull { it.ttlRemaining ?: Long.MAX_VALUE }
                ?: return Result.success()

            val ttl = formatTtl(vault.ttlRemaining)
            saveWidgetData(
                applicationContext,
                vaultName = vault.id.take(12) + "…",
                ttl = ttl,
                expiringSoon = vault.isExpiringSoon
            )
            VaultGlanceWidget().updateAll(applicationContext)
        }
        return Result.success()
    }

    private fun formatTtl(seconds: Long?): String {
        if (seconds == null) return "Unknown"
        val days = seconds / 86_400
        val hours = (seconds % 86_400) / 3_600
        return if (days > 0) "${days}d ${hours}h remaining" else "${hours}h remaining"
    }

    companion object {
        const val WORK_NAME = "vault_glance_widget_update"

        fun schedule(context: Context) {
            val request = PeriodicWorkRequestBuilder<VaultGlanceUpdateWorker>(15, TimeUnit.MINUTES)
                .setConstraints(
                    Constraints.Builder()
                        .setRequiredNetworkType(NetworkType.CONNECTED)
                        .build()
                )
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                WORK_NAME,
                ExistingPeriodicWorkPolicy.KEEP,
                request
            )
        }
    }
}
