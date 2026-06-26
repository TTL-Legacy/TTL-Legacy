package com.ttllegacy.services

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import com.ttllegacy.ui.MainActivity
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class NotificationHelper @Inject constructor(@ApplicationContext private val context: Context) {

    companion object {
        const val CHANNEL_ID = "ttl_reminders"
        const val CHANNEL_NAME = "Check-in Reminders"
        const val QUEUED_CHANNEL_ID = "ttl_queued"
        const val QUEUED_CHANNEL_NAME = "Queued Check-ins"
        const val QUEUED_NOTIFICATION_ID = 9_001
    }

    init {
        createChannel(CHANNEL_ID, CHANNEL_NAME, NotificationManager.IMPORTANCE_HIGH)
        createChannel(QUEUED_CHANNEL_ID, QUEUED_CHANNEL_NAME, NotificationManager.IMPORTANCE_DEFAULT)
    }

    fun show(title: String, body: String, vaultId: String?) {
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP
            vaultId?.let { putExtra("vault_id", it) }
        }
        val pi = PendingIntent.getActivity(context, 0, intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)

        val notification = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_lock_idle_lock)
            .setContentTitle(title)
            .setContentText(body)
            .setAutoCancel(true)
            .setContentIntent(pi)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .build()

        val nm = context.getSystemService(NotificationManager::class.java)
        nm.notify(vaultId.hashCode(), notification)
    }

    fun showQueuedCheckIn(count: Int) {
        val intent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
        val pi = PendingIntent.getActivity(context, QUEUED_NOTIFICATION_ID, intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)

        val body = if (count == 1) "1 check-in will be submitted when back online"
                   else "$count check-ins will be submitted when back online"

        val notification = NotificationCompat.Builder(context, QUEUED_CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_lock_idle_lock)
            .setContentTitle("Check-in queued")
            .setContentText(body)
            .setOngoing(true)
            .setAutoCancel(false)
            .setContentIntent(pi)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .build()

        context.getSystemService(NotificationManager::class.java)
            .notify(QUEUED_NOTIFICATION_ID, notification)
    }

    fun cancelQueuedCheckIn() {
        context.getSystemService(NotificationManager::class.java).cancel(QUEUED_NOTIFICATION_ID)
    }

    private fun createChannel(id: String, name: String, importance: Int) {
        val channel = NotificationChannel(id, name, importance)
        context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }
}
