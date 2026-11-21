package com.ttllegacy.services

import androidx.room.*
import kotlinx.coroutines.flow.Flow

/**
 * Represents a signed (but not yet submitted) check-in transaction.
 *
 * [signedPayload] is the base64-encoded passkey-signed blob ready for the backend.
 * [queuedAt] is epoch-millis when the check-in was queued locally.
 * [ttlExpiresAt] is the epoch-millis deadline. If the queue is flushed after this
 * time, the sync worker will warn the user rather than silently succeed.
 */
@Entity(tableName = "pending_checkins")
data class PendingCheckIn(
    @PrimaryKey val vaultId: String,
    val signedPayload: String = "",
    val queuedAt: Long = System.currentTimeMillis(),
    /** Expected TTL expiry in epoch-millis. 0 = unknown. */
    val ttlExpiresAt: Long = 0L
)

@Dao
interface PendingCheckInDao {
    @Query("SELECT * FROM pending_checkins ORDER BY queuedAt ASC")
    suspend fun getAll(): List<PendingCheckIn>

    @Query("SELECT COUNT(*) FROM pending_checkins")
    fun observeCount(): Flow<Int>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insert(item: PendingCheckIn)

    @Delete
    suspend fun delete(item: PendingCheckIn)

    @Query("DELETE FROM pending_checkins")
    suspend fun deleteAll()

    /** Returns items whose TTL has already passed (potential expired check-ins). */
    @Query("SELECT * FROM pending_checkins WHERE ttlExpiresAt > 0 AND ttlExpiresAt < :nowMillis")
    suspend fun getExpired(nowMillis: Long = System.currentTimeMillis()): List<PendingCheckIn>
}

/**
 * Room database backing the offline check-in queue.
 *
 * The [PendingCheckIn] entity is registered here so the queue survives process
 * death and can be flushed by the sync worker once connectivity is restored.
 */
@Database(
    entities = [PendingCheckIn::class],
    version = 1,
    exportSchema = false
)
abstract class CheckInDatabase : RoomDatabase() {
    abstract fun pendingCheckInDao(): PendingCheckInDao

    companion object {
        private const val DATABASE_NAME = "checkin_queue.db"

        @Volatile
        private var INSTANCE: CheckInDatabase? = null

        fun getInstance(context: android.content.Context): CheckInDatabase =
            INSTANCE ?: synchronized(this) {
                INSTANCE ?: Room.databaseBuilder(
                    context.applicationContext,
                    CheckInDatabase::class.java,
                    DATABASE_NAME
                ).build().also { INSTANCE = it }
            }
    }
}
