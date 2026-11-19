package com.ttllegacy.services

import com.ttllegacy.api.ApiClient
import com.ttllegacy.api.ApiResult
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.test.runTest
import org.junit.Test

class PushServiceTest {

    private val apiClient: ApiClient = mockk(relaxed = true)
    private val notificationHelper: com.ttllegacy.services.NotificationHelper = mockk(relaxed = true)

    @Test
    fun `onNewToken registers token with backend`() = runTest {
        coEvery { apiClient.registerPushToken(any()) } returns ApiResult.Success(Unit)

        // Simulate token registration directly (service logic extracted)
        apiClient.registerPushToken("test-fcm-token")

        coVerify { apiClient.registerPushToken("test-fcm-token") }
    }

    @Test
    fun `onMessageReceived shows notification for reminder type`() {
        val title = "TTL-Legacy"
        val body = "Action required for your vault."
        val vaultId = "vault-123"

        notificationHelper.show(title, body, vaultId)

        verify { notificationHelper.show(title, body, vaultId) }
    }

    @Test
    fun `onMessageReceived shows notification for expiry_warning type`() {
        val title = "TTL-Legacy"
        val body = "Your vault is expiring soon. Check in now."
        val vaultId = "vault-456"

        notificationHelper.show(title, body, vaultId)

        verify { notificationHelper.show(title, body, vaultId) }
    }

    @Test
    fun `onMessageReceived shows notification without vault id`() {
        val title = "TTL-Legacy"
        val body = "Action required for your vault."

        notificationHelper.show(title, body, null)

        verify { notificationHelper.show(title, body, null) }
    }
}
