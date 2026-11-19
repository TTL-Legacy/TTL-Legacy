package com.ttllegacy.services

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.work.ListenableWorker
import androidx.work.NetworkType
import androidx.work.testing.TestListenableWorkerBuilder
import androidx.work.testing.WorkManagerTestInitHelper
import com.ttllegacy.api.ApiClient
import com.ttllegacy.api.ApiResult
import com.ttllegacy.data.PendingCheckIn
import com.ttllegacy.data.PendingCheckInDao
import com.ttllegacy.notifications.NotificationHelper
import kotlinx.coroutines.runBlocking
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.mockito.Mock
import org.mockito.MockitoAnnotations
import org.mockito.kotlin.any
import org.mockito.kotlin.never
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

    private fun pendingCheckIn(
        vaultId: String,
        ttlExpiresAt: Long = Long.MAX_VALUE,
        enqueuedAt: Long = 0L
    ) = PendingCheckIn(
        vaultId = vaultId,
        ttlExpiresAt = ttlExpiresAt,
        enqueuedAt = enqueuedAt
    )

    @Test
    fun testSuccess_removesItemsFromQueueAndNotifies() = runBlocking {
        val mockItem = pendingCheckIn("vault-123")
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
    fun testFifo_syncsInEnqueueOrder() = runBlocking {
        val first = pendingCheckIn("vault-first", enqueuedAt = 1L)
        val second = pendingCheckIn("vault-second", enqueuedAt = 2L)
        val third = pendingCheckIn("vault-third", enqueuedAt = 3L)
        whenever(dao.getAll()).thenReturn(listOf(first, second, third))
        whenever(apiClient.checkIn(any())).thenReturn(ApiResult.Success(Unit))

        val worker = TestListenableWorkerBuilder<CheckInSyncWorker>(context)
            .build()

        val result = worker.doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        val inOrder = org.mockito.kotlin.inOrder(apiClient, dao)
        inOrder.verify(apiClient).checkIn("vault-first")
        inOrder.verify(dao).delete(first)
        inOrder.verify(apiClient).checkIn("vault-second")
        inOrder.verify(dao).delete(second)
        inOrder.verify(apiClient).checkIn("vault-third")
        inOrder.verify(dao).delete(third)
    }

    @Test
    fun testRetry_onNetworkFailure() = runBlocking {
        val mockItem = pendingCheckIn("vault-123")
        whenever(dao.getAll()).thenReturn(listOf(mockItem))
        whenever(apiClient.checkIn("vault-123")).thenReturn(ApiResult.NetworkUnavailable)

        val worker = TestListenableWorkerBuilder<CheckInSyncWorker>(context)
            .build()

        val result = worker.doWork()

        assertEquals(ListenableWorker.Result.retry(), result)
        verify(dao, never()).delete(any())
    }

    @Test
    fun testFailure_removesItemOnApiError() = runBlocking {
        val mockItem = pendingCheckIn("vault-123")
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
        val expiredItem = pendingCheckIn(
            vaultId = "vault-expired",
            ttlExpiresAt = nowMillis - 1000
        )
        val validItem = pendingCheckIn(
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
        verify(dao).delete(expiredItem)
        verify(apiClient).checkIn("vault-valid")
        verify(dao).delete(validItem)
    }

    @Test
    fun testEmpty_returnsSuccessImmediately() = runBlocking {
        whenever(dao.getAll()).thenReturn(emptyList())

        val worker = TestListenableWorkerBuilder<CheckInSyncWorker>(context)
            .build()

        val result = worker.doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        verify(apiClient, never()).checkIn(any())
        verify(dao, never()).delete(any())
    }

    @Test
    fun testNetworkConstraint_isRequired() {
        val request = CheckInSyncWorker.buildWorkRequest()
        val constraints = request.workSpec.constraints
        assertEquals(NetworkType.CONNECTED, constraints.requiredNetworkType)
    }
}
