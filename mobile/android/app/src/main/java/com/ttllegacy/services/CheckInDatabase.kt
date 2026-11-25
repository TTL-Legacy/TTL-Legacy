package com.ttllegacy.services

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase

@Database(entities = [PendingCheckIn::class], version = 1, exportSchema = false)
abstract class CheckInDatabase : RoomDatabase() {
    abstract fun pendingCheckInDao(): PendingCheckInDao

    companion object {
        fun create(context: Context): CheckInDatabase =
            Room.databaseBuilder(context, CheckInDatabase::class.java, "checkin_queue.db")
                .fallbackToDestructiveMigration()
                .build()
    }
}
