package com.ttllegacy.services

import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import com.ttllegacy.api.ApiClient
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import javax.inject.Inject

@AndroidEntryPoint
class TTLFirebaseMessagingService : FirebaseMessagingService() {

    @Inject lateinit var apiClient: ApiClient
    @Inject lateinit var notificationHelper: NotificationHelper

    override fun onNewToken(token: String) {
        CoroutineScope(Dispatchers.IO).launch {
            apiClient.registerPushToken(token)
        }
    }

    override fun onMessageReceived(message: RemoteMessage) {
        val vaultId = message.data["vault_id"]
        val type = message.data["type"] ?: "reminder"
        val title = message.notification?.title ?: "TTL-Legacy"
        val body = message.notification?.body ?: when (type) {
            "expiry_warning" -> "Your vault is expiring soon. Check in now."
            "released" -> "Your vault has been released to the beneficiary."
            else -> "Action required for your vault."
        }
        notificationHelper.show(title, body, vaultId)
    }
}
