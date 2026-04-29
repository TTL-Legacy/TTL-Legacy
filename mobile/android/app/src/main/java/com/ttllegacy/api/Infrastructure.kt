package com.ttllegacy.api

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import java.security.MessageDigest
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class NetworkMonitor @Inject constructor(@ApplicationContext private val context: Context) {
    val isConnected: Boolean
        get() {
            val cm = context.getSystemService(ConnectivityManager::class.java)
            val network = cm.activeNetwork ?: return false
            val caps = cm.getNetworkCapabilities(network) ?: return false
            return caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
        }
}

@Singleton
class OfflineCache @Inject constructor(@ApplicationContext private val context: Context) {
    private val dir = File(context.cacheDir, "ttl_offline").also { it.mkdirs() }

    fun save(key: String, json: String) {
        File(dir, key.sha256()).writeText(json)
    }

    fun load(key: String): String? = runCatching { File(dir, key.sha256()).readText() }.getOrNull()

    private fun String.sha256(): String {
        val digest = MessageDigest.getInstance("SHA-256").digest(toByteArray())
        return digest.joinToString("") { "%02x".format(it) }
    }
}

@Singleton
class TokenProvider @Inject constructor(@ApplicationContext private val context: Context) {
    private val prefs = context.getSharedPreferences("ttl_auth", Context.MODE_PRIVATE)

    var token: String?
        get() = prefs.getString("token", null)
        set(value) = prefs.edit().apply {
            if (value != null) putString("token", value) else remove("token")
        }.apply()

    fun clear() { token = null }
}
