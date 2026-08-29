package com.ttllegacy.services

import android.content.Context
import androidx.hilt.work.HiltWorker
import androidx.work.*
import com.ttllegacy.api.ApiClient
import com.ttllegacy.api.ApiResult
import dagger.assisted.Assisted
import dagger.assisted.AssistedInject
import java.util.concurrent.TimeUnit

@HiltWorker
class CheckInSyncWorker @AssistedInject constructor(
    @Assisted context: Context,
    @Assisted params: WorkerParameters,
    private val apiClient: ApiClient,
    private val dao: PendingCheckInDao,
    private val notificationHelper: NotificationHelper
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val pending = dao.getAll()
        if (pending.isEmpty()) return Result.success()

        // Warn about items that may have already expired before we can submit them.
        val nowMillis = System.currentTimeMillis()
        val expired = pending.filter { it.ttlExpiresAt > 0 && it.ttlExpiresAt < nowMillis }
        if (expired.isNotEmpty()) {
            notificationHelper.notifyExpiredCheckInsInQueue(
                vaultIds = expired.map { it.vaultId }
            )
        }

        var hasNetworkFailure = false
        for (item in pending) {
            when (val result = apiClient.checkIn(item.vaultId)) {
                is ApiResult.Success -> {
                    dao.delete(item)
                    notificationHelper.notifyCheckInSyncSuccess(item.vaultId)
                }
                ApiResult.NetworkUnavailable -> hasNetworkFailure = true
                is ApiResult.Error -> {
                    // Remove from queue even on error — prevent infinite retry loops.
                    // The server-side response will reflect the true vault state.
                    dao.delete(item)
                }
            }
        }

        if (dao.getAll().isEmpty()) {
            notificationHelper.cancelQueuedCheckIn()
        }

        return if (hasNetworkFailure) Result.retry() else Result.success()
    }

    companion object {
        const val WORK_NAME = "checkin_sync"

        fun schedule(context: Context) {
            val request = OneTimeWorkRequestBuilder<CheckInSyncWorker>()
                .setConstraints(
                    Constraints.Builder()
                        .setRequiredNetworkType(NetworkType.CONNECTED)
                        .build()
                )
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
                .build()
            WorkManager.getInstance(context).enqueueUniqueWork(
                WORK_NAME,
                ExistingWorkPolicy.KEEP,
                request
            )
        }
    }
}
