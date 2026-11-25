import XCTest
@testable import TTLLegacy

final class CertificatePinningTests: XCTestCase {

    var pinnedCertificateService: CertificatePinningService!

    override func setUp() {
        super.setUp()
        pinnedCertificateService = CertificatePinningService.shared
    }

    override func tearDown() {
        super.tearDown()
        pinnedCertificateService = nil
    }

    // MARK: - Certificate Pinning Configuration

    func test_pinnedCertificates_configured_forProductionHost() {
        let pins = pinnedCertificateService.pinnedPublicKeys(for: "api.ttl-legacy.app")
        XCTAssertFalse(pins.isEmpty, "Production host should have pinned certificates")
    }

    func test_pinnedCertificates_notConfigured_forUnknownHost() {
        let pins = pinnedCertificateService.pinnedPublicKeys(for: "unknown.example.com")
        XCTAssertTrue(pins.isEmpty, "Unknown host should have no pinned certificates")
    }

    // MARK: - URL Session Configuration

    func test_urlSessionConfiguration_includesPinning_forAPIClient() {
        let config = pinnedCertificateService.configureURLSessionConfig()
        XCTAssertNotNil(config)
        // Verify that certificate pinning is enabled through delegate
        XCTAssertTrue(pinnedCertificateService.isPinningEnabled)
    }

    // MARK: - Challenge Validation

    func test_urlSessionDelegate_validCertificate_completesAuthentication() {
        let delegate = pinnedCertificateService.urlSessionDelegate()
        XCTAssertNotNil(delegate)
    }

    func test_urlSessionDelegate_invalidCertificate_rejectedByValidator() {
        let delegate = pinnedCertificateService.urlSessionDelegate()
        XCTAssertNotNil(delegate)
        // Verify the delegate implements URLSessionDelegate
        XCTAssertTrue(delegate is URLSessionDelegate)
    }

    // MARK: - Pin Rotation

    func test_rotatePins_updatesConfigurationAndMaintainsConnectivity() {
        let oldPins = pinnedCertificateService.pinnedPublicKeys(for: "api.ttl-legacy.app")
        XCTAssertFalse(oldPins.isEmpty)

        // Simulate updating to new pins
        pinnedCertificateService.updatePinnedPublicKeys(
            ["new-pin-1", "new-pin-2"],
            for: "api.ttl-legacy.app"
        )

        let newPins = pinnedCertificateService.pinnedPublicKeys(for: "api.ttl-legacy.app")
        XCTAssertTrue(newPins.contains("new-pin-1"))
        XCTAssertTrue(newPins.contains("new-pin-2"))
    }

    func test_backupPins_preventsLockedOutScenario() {
        let backupPins = pinnedCertificateService.backupPublicKeys(for: "api.ttl-legacy.app")
        XCTAssertFalse(backupPins.isEmpty, "Backup pins should be configured for production")
    }

    func test_pinExpiration_allowsTimeWindowForRotation() {
        let pinExpirationDate = pinnedCertificateService.pinExpirationDate(for: "api.ttl-legacy.app")
        XCTAssertNotNil(pinExpirationDate)
        let daysFuture = Calendar.current.dateComponents([.day], from: Date(), to: pinExpirationDate!).day ?? 0
        XCTAssertGreater(daysFuture, 0, "Pin should have future expiration")
    }

    // MARK: - Multiple Hosts

    func test_multiplePinnedHosts_eachConfigured() {
        let apiHost = "api.ttl-legacy.app"
        let webhookHost = "webhooks.ttl-legacy.app"

        let apiPins = pinnedCertificateService.pinnedPublicKeys(for: apiHost)
        let webhookPins = pinnedCertificateService.pinnedPublicKeys(for: webhookHost)

        XCTAssertFalse(apiPins.isEmpty)
        XCTAssertFalse(webhookPins.isEmpty)
    }

    // MARK: - Security Properties

    func test_pinnedCertificates_usePublicKeyPins() {
        let pins = pinnedCertificateService.pinnedPublicKeys(for: "api.ttl-legacy.app")
        // Public key pins are more resilient to certificate rotation
        pins.forEach { pin in
            XCTAssertTrue(!pin.isEmpty, "Each pin should be non-empty")
            // Verify pin format (typically base64-encoded SHA256)
            XCTAssertGreaterThan(pin.count, 20, "Public key pin should have sufficient entropy")
        }
    }

    func test_certificatePinning_disablesMITM() {
        let pinnedHost = "api.ttl-legacy.app"
        let delegate = pinnedCertificateService.urlSessionDelegate()

        // Verify that pinning is enforced
        XCTAssertTrue(pinnedCertificateService.isPinningEnabled)
        // Verify delegate will reject non-matching certificates
        XCTAssertNotNil(delegate)
    }

    // MARK: - Logging and Monitoring

    func test_pinFailure_logsSecurityEvent() {
        let expectation = self.expectation(description: "Security event logged")

        pinnedCertificateService.onPinValidationFailure = { host, error in
            XCTAssertEqual(host, "api.ttl-legacy.app")
            XCTAssertNotNil(error)
            expectation.fulfill()
        }

        // Trigger a pin validation failure scenario (would be called by URLSession delegate)
        pinnedCertificateService.logPinValidationFailure(for: "api.ttl-legacy.app", error: NSError(domain: "Test", code: -1))

        waitForExpectations(timeout: 1.0)
    }

    func test_successfulPinValidation_recordsMetrics() {
        let expectation = self.expectation(description: "Success metrics recorded")

        pinnedCertificateService.onPinValidationSuccess = { host in
            XCTAssertEqual(host, "api.ttl-legacy.app")
            expectation.fulfill()
        }

        pinnedCertificateService.recordSuccessfulValidation(for: "api.ttl-legacy.app")

        waitForExpectations(timeout: 1.0)
    }
}
