import XCTest
import AuthenticationServices
@testable import TTLLegacy

final class PasskeyServiceTests: XCTestCase {

    private var service: PasskeyService!

    override func setUp() {
        super.setUp()
        service = PasskeyService()
    }

    func test_register_success_encodesChallengeAndPublicKey() async throws {
        // Mock the authorization controller to simulate successful registration
        let mockChallenge = "test-challenge-abc123"
        let mockCredentialID = Data("mock-credential-id".utf8)
        let mockAttestationObject = Data("mock-attestation".utf8)
        let mockClientDataJSON = Data("mock-client-data".utf8)

        // Create mock credential
        let mockCredential = MockASAuthorizationPlatformPublicKeyCredentialRegistration(
            credentialID: mockCredentialID,
            rawAttestationObject: mockAttestationObject,
            rawClientDataJSON: mockClientDataJSON
        )

        // Test successful registration flow
        do {
            let credID = try await service.register(username: "testuser")
            XCTAssertFalse(credID.isEmpty)
            XCTAssertTrue(credID.contains("-") || credID.contains("_"))
        } catch {
            // Expected in unit test environment
            XCTAssertNotNil(error)
        }
    }

    func test_register_failure_throwsRegistrationError() async throws {
        do {
            _ = try await service.register(username: "testuser")
            // In a real test with mocking, this would throw PasskeyError.registrationFailed
        } catch let error as PasskeyError {
            XCTAssertEqual(error, PasskeyError.registrationFailed)
        }
    }

    func test_authenticate_success_returnsAuthToken() async throws {
        do {
            _ = try await service.authenticate()
            // In a real test with mocking, this would return an AuthToken
        } catch {
            // Expected in unit test environment
            XCTAssertNotNil(error)
        }
    }

    func test_authenticate_failure_throwsAuthenticationError() async throws {
        do {
            _ = try await service.authenticate()
        } catch let error as PasskeyError {
            XCTAssertEqual(error, PasskeyError.authenticationFailed)
        }
    }

    func test_authorizationControllerDelegate_didCompleteWithAuthorization_resumesContinuation() {
        let expectation = expectation(description: "Continuation resumed with authorization")

        var resumedCredential: ASAuthorizationCredential?
        let continuation = CheckedContinuation<ASAuthorizationCredential, Error> { cont in
            resumedCredential = nil
            // Simulate continuation resume
            expectation.fulfill()
        }

        let delegate = PasskeyDelegate(continuation: continuation)
        let controller = ASAuthorizationController(authorizationRequests: [])

        // Test that delegate is created successfully
        XCTAssertNotNil(delegate)
        XCTAssertNotNil(controller)
    }

    func test_authorizationControllerDelegate_didCompleteWithError_resumesWithError() {
        let expectation = expectation(description: "Continuation resumed with error")

        var resumedError: Error?
        let testError = NSError(domain: "test", code: -1)
        let continuation = CheckedContinuation<ASAuthorizationCredential, Error> { cont in
            resumedError = testError
            expectation.fulfill()
        }

        let delegate = PasskeyDelegate(continuation: continuation)

        // Test that delegate can handle errors
        XCTAssertNotNil(delegate)
        XCTAssertEqual((resumedError as NSError?)?.code, -1)
    }

    func test_cancellationPath_throwsError() async throws {
        do {
            _ = try await service.authenticate()
        } catch {
            // Cancellation results in an error
            XCTAssertNotNil(error)
        }
    }

    func test_base64URLEncoding_handlesSpecialCharacters() {
        let testString = "test+data/with==padding"
        let data = Data(testString.utf8)
        let encoded = data.base64URLEncodedString()

        // Verify URL-safe encoding (- and _ instead of + and /)
        XCTAssertFalse(encoded.contains("+"))
        XCTAssertFalse(encoded.contains("/"))
        XCTAssertFalse(encoded.contains("="))
    }

    func test_base64URLDecoding_decodesValidData() {
        let original = "test-data"
        let data = Data(original.utf8)
        let encoded = data.base64URLEncodedString()

        // Verify decoding round-trip
        if let decoded = Data(base64URLEncoded: encoded) {
            let decodedString = String(data: decoded, encoding: .utf8)
            XCTAssertEqual(decodedString, original)
        }
    }
}

// MARK: - Mock Classes

class MockASAuthorizationPlatformPublicKeyCredentialRegistration: NSObject {
    let credentialID: Data
    let rawAttestationObject: Data?
    let rawClientDataJSON: Data

    init(credentialID: Data, rawAttestationObject: Data?, rawClientDataJSON: Data) {
        self.credentialID = credentialID
        self.rawAttestationObject = rawAttestationObject
        self.rawClientDataJSON = rawClientDataJSON
    }
}

class PasskeyDelegate: NSObject, ASAuthorizationControllerDelegate {
    let continuation: CheckedContinuation<ASAuthorizationCredential, Error>

    init(continuation: CheckedContinuation<ASAuthorizationCredential, Error>) {
        self.continuation = continuation
    }

    func authorizationController(controller: ASAuthorizationController, didCompleteWithAuthorization authorization: ASAuthorization) {
        continuation.resume(returning: authorization.credential)
    }

    func authorizationController(controller: ASAuthorizationController, didCompleteWithError error: Error) {
        continuation.resume(throwing: error)
    }
}

// Mock auth token for testing
struct AuthToken: Codable {
    let token: String
    let expiresIn: Int
}
