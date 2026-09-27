package com.ttllegacy.services

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.work.ListenableWorker
import androidx.work.testing.TestListenableWorkerBuilder
import androidx.work.testing.WorkManagerTestInitHelper
import com.ttllegacy.api.ApiClient
import com.ttllegacy.api.ApiResult
import kotlinx.coroutines.runBlocking
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.mockito.Mock
import org.mockito.MockitoAnnotations
import org.mockito.kotlin.any
import org.mockito.kotlin.verify
import org.mockito.kotlin.whenever
import org.robolectric.RobolectricTestRunner
import kotlin.test.assertEquals

@RunWith(RobolectricTestRunner::class)
class CheckInSyncWorkerTest {

    @Mock
    private lateinit var apiClient: ApiClient

    @Mock
    private lateinit var dao: PendingCheckInDao

    @Mock
    private lateinit var notificationHelper: NotificationHelper

    private lateinit var context: Context

    @Before
    fun setup() {
        MockitoAnnotations.openMocks(this)
        context = ApplicationProvider.getApplicationContext()
        WorkManagerTestInitHelper.initializeTestWorkManager(context)
    }

    @Test
    fun testSuccess_removesItemsFromQueueAndNotifies() = runBlocking {
        val mockItem = PendingCheckIn(vaultId = "vault-123", ttlExpiresAt = 0)
        whenever(dao.getAll()).thenReturn(listOf(mockItem))
        whenever(apiClient.checkIn("vault-123")).thenReturn(ApiResult.Success(Unit))

        val worker = TestListenableWorkerBuilder<CheckInSyncWorker>(context)
            .build()

        val result = worker.doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        verify(dao).delete(mockItem)
        verify(notificationHelper).notifyCheckInSyncSuccess("vault-123")
        verify(notificationHelper).cancelQueuedCheckIn()
    }

    @Test
    fun testRetry_onNetworkFailure() = runBlocking {
        val mockItem = PendingCheckIn(vaultId = "vault-123", ttlExpiresAt = 0)
        whenever(dao.getAll()).thenReturn(listOf(mockItem))
        whenever(apiClient.checkIn("vault-123")).thenReturn(ApiResult.NetworkUnavailable)

        val worker = TestListenableWorkerBuilder<CheckInSyncWorker>(context)
            .build()

        val result = worker.doWork()

        assertEquals(ListenableWorker.Result.retry(), result)
        verify(dao).deleteNever()
    }

    @Test
    fun testFailure_removesItemOnApiError() = runBlocking {
        val mockItem = PendingCheckIn(vaultId = "vault-123", ttlExpiresAt = 0)
        whenever(dao.getAll()).thenReturn(listOf(mockItem))
        whenever(apiClient.checkIn("vault-123")).thenReturn(ApiResult.Error("Server error"))

        val worker = TestListenableWorkerBuilder<CheckInSyncWorker>(context)
            .build()

        val result = worker.doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        verify(dao).delete(mockItem)
    }

    @Test
    fun testExpiredItems_notifiesAndRemoves() = runBlocking {
        val nowMillis = System.currentTimeMillis()
        val expiredItem = PendingCheckIn(
            vaultId = "vault-expired",
            ttlExpiresAt = nowMillis - 1000
        )
        val validItem = PendingCheckIn(
            vaultId = "vault-valid",
            ttlExpiresAt = nowMillis + 10000
        )
        whenever(dao.getAll()).thenReturn(listOf(expiredItem, validItem))
        whenever(apiClient.checkIn(any())).thenReturn(ApiResult.Success(Unit))

        val worker = TestListenableWorkerBuilder<CheckInSyncWorker>(context)
            .build()

        val result = worker.doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        verify(notificationHelper).notifyExpiredCheckInsInQueue(
            vaultIds = listOf("vault-expired")
        )
    }

    @Test
    fun testEmpty_returnsSuccessImmediately() = runBlocking {
        whenever(dao.getAll()).thenReturn(emptyList())

        val worker = TestListenableWorkerBuilder<CheckInSyncWorker>(context)
            .build()

        val result = worker.doWork()

        assertEquals(ListenableWorker.Result.success(), result)
    }

    @Test
    fun testNetworkConstraint_isRequired() {
        val request = CheckInSyncWorker.buildWorkRequest()
        val constraints = request.constraints
        assertEquals(androidx.work.NetworkType.CONNECTED, constraints.requiredNetworkType)
    }
}

data class PendingCheckIn(
    val vaultId: String,
    val ttlExpiresAt: Long
)

interface PendingCheckInDao {
    fun getAll(): List<PendingCheckIn>
    fun delete(item: PendingCheckIn)
    fun deleteNever()
}

interface NotificationHelper {
    fun notifyExpiredCheckInsInQueue(vaultIds: List<String>)
    fun notifyCheckInSyncSuccess(vaultId: String)
    fun cancelQueuedCheckIn()
}
