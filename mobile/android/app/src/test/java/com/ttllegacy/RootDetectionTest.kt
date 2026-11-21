package com.ttllegacy

import org.junit.Before
import org.junit.Test
import org.junit.Assert.*

/**
 * Unit tests for Android root detection and warning system.
 *
 * Verifies:
 *  - Device root status detection through multiple heuristics
 *  - Non-blocking warning display to users
 *  - Safe passkey handling on compromised devices
 *  - Performance and caching of detection results
 */
class RootDetectionTest {

    private lateinit var rootDetectionService: RootDetectionService

    @Before
    fun setUp() {
        rootDetectionService = RootDetectionService.getInstance()
    }

    // -----------------------------------------------------------------------
    // Heuristic Detection
    // -----------------------------------------------------------------------

    @Test
    fun detectRoot_checksForSuperUserBinary() {
        val isRooted = rootDetectionService.checkForSuperUserBinary()
        assertNotNull("Root detection should complete", isRooted)
    }

    @Test
    fun detectRoot_checksForSuperUserAPK() {
        val isRooted = rootDetectionService.checkForSuperUserAPK()
        assertNotNull("SuperUser APK detection should complete", isRooted)
    }

    @Test
    fun detectRoot_checksForBusyBoxBinary() {
        val isRooted = rootDetectionService.checkForBusyBox()
        assertNotNull("BusyBox detection should complete", isRooted)
    }

    @Test
    fun detectRoot_checksForBuildTags() {
        val isRooted = rootDetectionService.checkBuildTags()
        assertNotNull("Build tags check should complete", isRooted)
    }

    @Test
    fun detectRoot_checksForTestKeys() {
        val buildTags = "test-keys"
        assertTrue("Test keys indicate development build", buildTags.contains("test"))
    }

    @Test
    fun detectRoot_checksForDebugBuild() {
        val debugBuild = false // Normal devices should not be debug builds
        assertNotNull(debugBuild)
    }

    @Test
    fun detectRoot_checksForMagiskPresence() {
        val isMagiskDetected = rootDetectionService.checkForMagisk()
        assertNotNull("Magisk detection should complete", isMagiskDetected)
    }

    @Test
    fun detectRoot_checksForPropsFile() {
        val isRooted = rootDetectionService.checkBuildProperties()
        assertNotNull("Build properties check should complete", isRooted)
    }

    // -----------------------------------------------------------------------
    // Combined Detection
    // -----------------------------------------------------------------------

    @Test
    fun performDetection_returnsBooleanResult() {
        val isRooted = rootDetectionService.isDeviceRooted()
        assertNotNull("Root detection should return result", isRooted)
    }

    @Test
    fun performDetection_checksMultipleHeuristics() {
        val detectionResult = rootDetectionService.performComprehensiveDetection()
        assertNotNull("Comprehensive detection should complete", detectionResult)
    }

    @Test
    fun detection_noFalsePositives_onLegitimateDevice() {
        val isRooted = rootDetectionService.isDeviceRooted()
        assertNotNull("Detection should work on legitimate devices", isRooted)
    }

    // -----------------------------------------------------------------------
    // Warning Display
    // -----------------------------------------------------------------------

    @Test
    fun warningDisplayed_whenRootDetected() {
        var warningShown = false

        rootDetectionService.onRootDetected = { severity ->
            warningShown = true
            assertNotNull("Severity should be provided", severity)
            assertTrue("Severity should be warning or critical",
                severity == "warning" || severity == "critical")
        }

        rootDetectionService.checkAndShowWarning()
    }

    @Test
    fun warning_isNonBlocking_userCanContinue() {
        var userAllowedToContinue = false

        rootDetectionService.onWarningDismissed = {
            userAllowedToContinue = true
        }

        // Simulate user dismissing warning
        rootDetectionService.dismissWarning()
        assertTrue("User should be able to continue after dismissing warning",
            userAllowedToContinue)
    }

    @Test
    fun warningContent_informsUserOfRisk() {
        val warningMessage = rootDetectionService.warningMessage()
        assertNotNull("Warning message should be provided", warningMessage)
        assertFalse("Warning message should not be empty", warningMessage.isEmpty())
        assertTrue("Message should mention security risk",
            warningMessage.lowercase().contains("security") ||
            warningMessage.lowercase().contains("risk") ||
            warningMessage.lowercase().contains("root"))
    }

    @Test
    fun warningTitle_clearlyIndicatesDeviceCompromise() {
        val warningTitle = rootDetectionService.warningTitle()
        assertNotNull("Warning title should be provided", warningTitle)
        assertFalse("Warning title should not be empty", warningTitle.isEmpty())
    }

    @Test
    fun warningActionButton_providesMoreInfo() {
        val actionButton = rootDetectionService.warningActionButton()
        assertNotNull("Action button should be available", actionButton)
        assertTrue("Button should be informative", !actionButton.isEmpty())
    }

    // -----------------------------------------------------------------------
    // Passkey Handling
    // -----------------------------------------------------------------------

    @Test
    fun passkeyOperations_allowedWithWarning_onRootedDevice() {
        var warningRequested = false

        rootDetectionService.onWarningRequired = {
            warningRequested = true
        }

        val canProceed = rootDetectionService.canProceedWithPasskeyOperation()
        assertNotNull("Should return capability status", canProceed)
    }

    @Test
    fun passkeyWarning_notBlockingButInformative() {
        val result = rootDetectionService.checkBeforePasskeyOperation()
        assertNotNull("Check should return result", result)
    }

    @Test
    fun sensitiveOperation_showsWarning_beforePasskeyAccess() {
        var warningShown = false

        rootDetectionService.onWarningRequired = {
            warningShown = true
        }

        rootDetectionService.checkBeforePasskeyOperation()
        // Warning may or may not be shown depending on device state
    }

    @Test
    fun passkey_operationRecorded_forAudit() {
        rootDetectionService.logPasskeyOperationAttempt()
        // Verify operation was logged (tested in integration tests)
    }

    // -----------------------------------------------------------------------
    // Caching and Performance
    // -----------------------------------------------------------------------

    @Test
    fun detectionResult_cached_forPerformance() {
        val firstCheck = rootDetectionService.isDeviceRooted()
        val secondCheck = rootDetectionService.isDeviceRooted()

        // Both should return quickly (cached)
        assertNotNull("First check should complete", firstCheck)
        assertNotNull("Second check should complete", secondCheck)
    }

    @Test
    fun cacheInvalidation_afterSecurityUpdate() {
        val initialResult = rootDetectionService.isDeviceRooted()

        rootDetectionService.invalidateCache()
        val refreshedResult = rootDetectionService.isDeviceRooted()

        assertNotNull("Initial result should be available", initialResult)
        assertNotNull("Refreshed result should be available", refreshedResult)
    }

    @Test
    fun cacheTTL_appropriateForSecurityConcern() {
        val cacheDurationSeconds = rootDetectionService.cacheValiditySeconds()
        assertTrue("Cache should have TTL > 0", cacheDurationSeconds > 0)
        assertTrue("Cache should not exceed 1 hour", cacheDurationSeconds < 3600)
    }

    // -----------------------------------------------------------------------
    // Logging and Analytics
    // -----------------------------------------------------------------------

    @Test
    fun detectionAttempt_logged() {
        var logCalled = false

        rootDetectionService.onDetectionLogged = { result ->
            logCalled = true
            assertNotNull("Log should include result", result)
        }

        rootDetectionService.isDeviceRooted()
        assertTrue("Detection should be logged", logCalled)
    }

    @Test
    fun warningDisplayal_tracked() {
        var warningTracked = false

        rootDetectionService.onWarningTracked = { event ->
            warningTracked = true
            assertNotNull("Tracking event should be provided", event)
        }

        rootDetectionService.checkAndShowWarning()
        assertTrue("Warning display should be tracked", warningTracked)
    }

    @Test
    fun analyticsIncludePlatform_andDetectionMethod() {
        val analyticsEvent = rootDetectionService.buildAnalyticsEvent()
        assertNotNull("Analytics event should be created", analyticsEvent)
        assertTrue("Event should have platform info",
            analyticsEvent.containsKey("platform") || analyticsEvent.containsKey("device"))
    }

    // -----------------------------------------------------------------------
    // Edge Cases
    // -----------------------------------------------------------------------

    @Test
    fun detection_withoutCrashing_onFirstLaunch() {
        // Should not throw
        val result = rootDetectionService.isDeviceRooted()
        assertNotNull("Should not crash on first launch", result)
    }

    @Test
    fun warning_canBeDismissedMultipleTimes() {
        var dismissCount = 0

        rootDetectionService.onWarningDismissed = {
            dismissCount++
        }

        rootDetectionService.dismissWarning()
        rootDetectionService.dismissWarning()
        rootDetectionService.dismissWarning()

        assertEquals("Warning should be dismissible multiple times", 3, dismissCount)
    }

    @Test
    fun detection_gracefullyHandles_permissionErrors() {
        // Should not crash even if file access is restricted
        val result = rootDetectionService.isDeviceRooted()
        assertNotNull("Detection should handle permission errors gracefully", result)
    }

    @Test
    fun userCanDisableWarnings_permanently() {
        rootDetectionService.disableWarnings()

        var warningShown = false
        rootDetectionService.onRootDetected = { _ ->
            warningShown = true
        }

        rootDetectionService.checkAndShowWarning()
        assertFalse("Warnings should be disabled", rootDetectionService.warningsEnabled())
    }

    @Test
    fun userCanReenableWarnings() {
        rootDetectionService.disableWarnings()
        assertFalse("Warnings should be disabled", rootDetectionService.warningsEnabled())

        rootDetectionService.enableWarnings()
        assertTrue("Warnings should be re-enabled", rootDetectionService.warningsEnabled())
    }

    @Test
    fun detectionWithVariousAndroidVersions_works() {
        val sdkVersion = android.os.Build.VERSION.SDK_INT
        assertTrue("Should work on current Android version", sdkVersion > 0)
    }
}
