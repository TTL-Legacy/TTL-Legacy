package com.ttllegacy.services

import androidx.room.*
import kotlinx.coroutines.flow.Flow

@Entity(tableName = "pending_checkins")
data class PendingCheckIn(
    @PrimaryKey val vaultId: String,
    val queuedAt: Long
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
}
