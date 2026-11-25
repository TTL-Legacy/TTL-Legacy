package com.ttllegacy.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import androidx.hilt.work.HiltWorker
import androidx.work.*
import com.ttllegacy.R
import com.ttllegacy.api.ApiClient
import com.ttllegacy.api.ApiResult
import com.ttllegacy.ui.MainActivity
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import java.util.concurrent.TimeUnit

class VaultStatusWidget : AppWidgetProvider() {

    override fun onUpdate(context: Context, manager: AppWidgetManager, widgetIds: IntArray) {
        widgetIds.forEach { updateWidget(context, manager, it) }
    }

    companion object {
        private const val PREFS = "vault_widget_prefs"
        private const val KEY_VAULT_NAME = "vault_name"
        private const val KEY_TTL = "ttl_remaining"
        private const val KEY_LAST_CHECK_IN = "last_check_in"

        fun saveVaultData(context: Context, vaultName: String, ttlRemaining: String, lastCheckIn: String) {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
                .putString(KEY_VAULT_NAME, vaultName)
                .putString(KEY_TTL, ttlRemaining)
                .putString(KEY_LAST_CHECK_IN, lastCheckIn)
                .apply()
        }

        fun updateWidget(context: Context, manager: AppWidgetManager, widgetId: Int) {
            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            val vaultName = prefs.getString(KEY_VAULT_NAME, "—") ?: "—"
            val ttl = prefs.getString(KEY_TTL, "Unknown") ?: "Unknown"
            val lastCheckIn = prefs.getString(KEY_LAST_CHECK_IN, "Never") ?: "Never"

            val openIntent = Intent(context, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
            }
            val pendingIntent = PendingIntent.getActivity(
                context, 0, openIntent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )

            val views = RemoteViews(context.packageName, R.layout.vault_widget).apply {
                setTextViewText(R.id.widget_vault_name, vaultName)
                setTextViewText(R.id.widget_ttl, "TTL: $ttl")
                setTextViewText(R.id.widget_last_check_in, "Last check-in: $lastCheckIn")
                setOnClickPendingIntent(R.id.widget_root, pendingIntent)
            }
            manager.updateAppWidget(widgetId, views)
        }

        fun refreshAll(context: Context) {
            val manager = AppWidgetManager.getInstance(context)
            val ids = manager.getAppWidgetIds(ComponentName(context, VaultStatusWidget::class.java))
            ids.forEach { updateWidget(context, manager, it) }
        }
    }
}

@HiltWorker
class VaultWidgetUpdateWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val apiClient: ApiClient
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val result = apiClient.listVaults()
        if (result is ApiResult.Success) {
            val vault = result.data.firstOrNull() ?: return Result.success()
            val ttl = formatTtl(vault.ttlRemaining)
            VaultStatusWidget.saveVaultData(
                applicationContext,
                vaultName = vault.id.take(12) + "…",
                ttlRemaining = ttl,
                lastCheckIn = vault.lastCheckIn
            )
            VaultStatusWidget.refreshAll(applicationContext)
        }
        return Result.success()
    }

    private fun formatTtl(seconds: Long?): String {
        if (seconds == null) return "Unknown"
        val days = seconds / 86400
        val hours = (seconds % 86400) / 3600
        return if (days > 0) "${days}d ${hours}h" else "${hours}h"
    }

    companion object {
        const val WORK_NAME = "vault_widget_update"

        fun schedule(context: Context) {
            val request = PeriodicWorkRequestBuilder<VaultWidgetUpdateWorker>(15, TimeUnit.MINUTES)
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
