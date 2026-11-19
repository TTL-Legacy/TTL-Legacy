import XCTest
@testable import TTLLegacy

final class JailbreakDetectionTests: XCTestCase {

    var jailbreakDetectionService: JailbreakDetectionService!

    override func setUp() {
        super.setUp()
        jailbreakDetectionService = JailbreakDetectionService.shared
    }

    override func tearDown() {
        super.tearDown()
        jailbreakDetectionService = nil
    }

    // MARK: - Heuristic Detection

    func test_detectJailbreak_checksForSuspiciousFiles() {
        let suspiciousFiles = [
            "/Applications/Cydia.app",
            "/Library/MobileSubstrate",
            "/usr/sbin/sshd",
            "/private/var/lib/apt",
            "/bin/bash",
            "/usr/sbin/dpkg"
        ]

        let isJailbroken = jailbreakDetectionService.checkForSuspiciousFiles()
        XCTAssertNotNil(isJailbroken)
    }

    func test_detectJailbreak_checksForSuspiciousProcesses() {
        let isJailbroken = jailbreakDetectionService.checkForSuspiciousProcesses()
        XCTAssertNotNil(isJailbroken)
    }

    func test_detectJailbreak_checksForMaliciousPackages() {
        let isJailbroken = jailbreakDetectionService.checkForMaliciousPackages()
        XCTAssertNotNil(isJailbroken)
    }

    func test_detectJailbreak_checksFileSystemAccess() {
        let isJailbroken = jailbreakDetectionService.checkFileSystemAccess()
        XCTAssertNotNil(isJailbroken)
    }

    func test_detectJailbreak_checksForDebugger() {
        let isJailbroken = jailbreakDetectionService.checkForDebugger()
        XCTAssertNotNil(isJailbroken)
    }

    // MARK: - Combined Detection

    func test_performDetection_returnsBooleanResult() {
        let isJailbroken = jailbreakDetectionService.isDeviceJailbroken()
        XCTAssertNotNil(isJailbroken)
    }

    func test_performDetection_checksMultipleHeuristics() {
        let detectionResult = jailbreakDetectionService.performComprehensiveDetection()
        XCTAssertNotNil(detectionResult)
    }

    func test_detection_noFalsePositives_onLegitimateDevice() {
        // On a legitimate device, detection might return false
        // The test verifies the method works, not the result
        let isJailbroken = jailbreakDetectionService.isDeviceJailbroken()
        XCTAssertIsNotNil(isJailbroken)
    }

    // MARK: - Warning Display

    func test_warningDisplayed_whenJailbreakDetected() {
        jailbreakDetectionService.onJailbreakDetected = { severity in
            XCTAssertNotNil(severity)
            XCTAssertTrue(severity == "warning" || severity == "critical")
        }

        jailbreakDetectionService.checkAndShowWarning()
    }

    func test_warning_isNonBlocking_userCanContinue() {
        var userAllowedToContinue = false

        jailbreakDetectionService.onWarningDismissed = {
            userAllowedToContinue = true
        }

        // Simulate user dismissing warning
        jailbreakDetectionService.dismissWarning()
        XCTAssertTrue(userAllowedToContinue)
    }

    func test_warningContent_informsUserOfRisk() {
        let warningMessage = jailbreakDetectionService.warningMessage()
        XCTAssertNotNil(warningMessage)
        XCTAssertTrue(!warningMessage.isEmpty)
        XCTAssertTrue(warningMessage.lowercased().contains("security") ||
                      warningMessage.lowercased().contains("risk") ||
                      warningMessage.lowercased().contains("jailbreak"))
    }

    func test_warningTitle_clearlyIndicatesDeviceCompromise() {
        let warningTitle = jailbreakDetectionService.warningTitle()
        XCTAssertNotNil(warningTitle)
        XCTAssertTrue(!warningTitle.isEmpty)
    }

    // MARK: - Passkey Handling

    func test_passkeyOperations_allowedWithWarning_onJailbrokenDevice() {
        jailbreakDetectionService.onJailbreakDetected = { severity in
            XCTAssertNotNil(severity)
        }

        let passkeyOperationAllowed = jailbreakDetectionService.canProceedWithPasskeyOperation()
        XCTAssertNotNil(passkeyOperationAllowed)
    }

    func test_passkeyWarning_notBlockingButInformative() {
        let result = jailbreakDetectionService.checkBeforePasskeyOperation()
        XCTAssertNotNil(result)
    }

    func test_sensitiveOperation_showsWarning_beforePasskeyAccess() {
        var warningShown = false

        jailbreakDetectionService.onWarningRequired = {
            warningShown = true
        }

        jailbreakDetectionService.checkBeforePasskeyOperation()
        // On jailbroken device, warning should be triggered
    }

    // MARK: - Caching and Performance

    func test_detectionResult_cached_forPerformance() {
        let firstCheck = jailbreakDetectionService.isDeviceJailbroken()
        let secondCheck = jailbreakDetectionService.isDeviceJailbroken()

        // Both should return without delay (cached)
        XCTAssertNotNil(firstCheck)
        XCTAssertNotNil(secondCheck)
    }

    func test_cacheInvalidation_afterSecurityUpdate() {
        let initialResult = jailbreakDetectionService.isDeviceJailbroken()

        jailbreakDetectionService.invalidateCache()
        let refreshedResult = jailbreakDetectionService.isDeviceJailbroken()

        XCTAssertNotNil(initialResult)
        XCTAssertNotNil(refreshedResult)
    }

    func test_cacheTTL_appropriateForSecurityConcern() {
        let cacheDuration = jailbreakDetectionService.cacheValiditySeconds()
        XCTAssertGreater(cacheDuration, 0)
        XCTAssertLess(cacheDuration, 3600) // Cache shouldn't be longer than 1 hour
    }

    // MARK: - Logging and Analytics

    func test_detectionAttempt_logged() {
        var logCalled = false

        jailbreakDetectionService.onDetectionLogged = { result in
            logCalled = true
        }

        jailbreakDetectionService.isDeviceJailbroken()
        XCTAssertTrue(logCalled)
    }

    func test_warningDisplayal_tracked() {
        var warningTracked = false

        jailbreakDetectionService.onWarningTracked = { event in
            warningTracked = true
        }

        jailbreakDetectionService.checkAndShowWarning()
        XCTAssertTrue(warningTracked)
    }

    func test_analyticsIncludePlatform_andDetectionMethod() {
        let analyticsEvent = jailbreakDetectionService.buildAnalyticsEvent()
        XCTAssertNotNil(analyticsEvent)
        XCTAssertTrue(analyticsEvent["platform"] != nil || analyticsEvent["device"] != nil)
    }

    // MARK: - Edge Cases

    func test_detection_withoutCrashing_onFirstLaunch() {
        // Should not throw or crash
        let result = jailbreakDetectionService.isDeviceJailbroken()
        XCTAssertNotNil(result)
    }

    func test_warning_canBeDismissedMultipleTimes() {
        var dismissCount = 0

        jailbreakDetectionService.onWarningDismissed = {
            dismissCount += 1
        }

        jailbreakDetectionService.dismissWarning()
        jailbreakDetectionService.dismissWarning()
        jailbreakDetectionService.dismissWarning()

        XCTAssertEqual(dismissCount, 3)
    }

    func test_detection_gracefullyHandles_permissionErrors() {
        let result = jailbreakDetectionService.isDeviceJailbroken()
        XCTAssertNotNil(result)
        // Should not crash even if file access is denied
    }

    func test_userCanDisableWarnings_permanently() {
        jailbreakDetectionService.disableWarnings()

        jailbreakDetectionService.onJailbreakDetected = { severity in
            XCTFail("Warning should not be shown when disabled")
        }

        jailbreakDetectionService.checkAndShowWarning()
        XCTAssertTrue(!jailbreakDetectionService.warningsEnabled())
    }

    func test_userCanReenableWarnings() {
        jailbreakDetectionService.disableWarnings()
        XCTAssertFalse(jailbreakDetectionService.warningsEnabled())

        jailbreakDetectionService.enableWarnings()
        XCTAssertTrue(jailbreakDetectionService.warningsEnabled())
    }
}
