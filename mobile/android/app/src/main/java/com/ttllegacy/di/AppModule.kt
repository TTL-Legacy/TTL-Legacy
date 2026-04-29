package com.ttllegacy.di

import android.content.Context
import com.ttllegacy.BuildConfig
import com.ttllegacy.api.ApiClient
import com.ttllegacy.api.NetworkMonitor
import com.ttllegacy.api.OfflineCache
import com.ttllegacy.api.TokenProvider
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object AppModule {

    @Provides @Singleton
    fun provideApiClient(
        tokenProvider: TokenProvider,
        networkMonitor: NetworkMonitor,
        offlineCache: OfflineCache
    ): ApiClient = ApiClient(tokenProvider, networkMonitor, offlineCache, BuildConfig.API_BASE_URL)
}
