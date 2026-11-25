# Mobile Passkey Flow for iOS and Android

## Overview

This document describes how WebAuthn/Passkey credentials are created, registered, and used across iOS and Android platforms in TTL-Legacy. It covers the complete flow from credential creation to authentication.

## Architecture

### Registration Flow

The registration process establishes a new passkey credential for vault owner authentication.

```
┌─────────────────────────────────────────────────────────────────────┐
│ User initiates "Create Passkey"                                     │
└────────────────┬────────────────────────────────────────────────────┘
                 │
      ┌──────────▼──────────┐
      │ iOS (ASAuthorization)│ or │ Android (Credential Manager)       │
      └──────────┬──────────┘     
                 │
    ┌────────────▼────────────┐
    │ Generate credential pair:│
    │ - Public key (register) │
    │ - Private key (secure  │
    │   enclave/TEE)         │
    └────────────┬────────────┘
                 │
    ┌────────────▼──────────────────────┐
    │ Sign challenge with private key   │
    │ (Only accessible with biometric) │
    └────────────┬─────────────────────┘
                 │
    ┌────────────▼──────────────────────┐
    │ Send attestation + public key     │
    │ to TTL-Legacy backend             │
    └────────────┬─────────────────────┘
                 │
    ┌────────────▼──────────────────────┐
    │ Backend stores public key         │
    │ for vault owner                   │
    └────────────▼──────────────────────┘
                 │
          ┌──────▼──────┐
          │ Registration│
          │  Complete   │
          └─────────────┘
```

### Authentication Flow

The authentication process uses stored credentials to prove vault owner identity.

```
┌──────────────────────────────────────────┐
│ User requests authentication              │
│ (Check-in, withdrawal, etc.)             │
└────────────┬─────────────────────────────┘
             │
  ┌──────────▼──────────┐
  │ Backend generates  │
  │ random challenge   │
  └──────────┬─────────┘
             │
  ┌──────────▼────────────────────────────┐
  │ Challenge sent to mobile app          │
  └──────────┬────────────────────────────┘
             │
  ┌──────────▼──────────────────────────────────┐
  │ iOS: ASAuthorization.performRequests()     │
  │ -or-                                       │
  │ Android: CredentialManager.getCredential() │
  └──────────┬───────────────────────────────────┘
             │
  ┌──────────▼───────────────────────────────────┐
  │ Private key accessed (biometric/PIN prompt) │
  └──────────┬──────────────────────────────────┘
             │
  ┌──────────▼──────────────────────────────────────┐
  │ Sign challenge with stored private key         │
  │ Generate signature proof                       │
  └──────────┬─────────────────────────────────────┘
             │
  ┌──────────▼────────────────────────────────────┐
  │ Send signed challenge + public key ID         │
  │ to backend                                     │
  └──────────┬───────────────────────────────────┘
             │
  ┌──────────▼──────────────────────────────────────┐
  │ Backend verifies signature using stored        │
  │ public key                                      │
  └──────────┬─────────────────────────────────────┘
             │
       ┌─────▼────┐
       │  Success │
       │   /Fail  │
       └──────────┘
```

## iOS Implementation

### Credential Creation (Registration)

**Prerequisites:**
- iOS 16.0+
- `AuthenticationServices` framework imported

**Code Example:**

```swift
import AuthenticationServices

func registerPasskey(vaultId: String) async throws {
    // 1. Fetch registration challenge from backend
    let challenge = try await fetchRegistrationChallenge(vaultId: vaultId)
    
    // 2. Create platform authentication controller
    let platformProvider = ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier: "ttl-legacy.app")
    
    // 3. Create registration request
    let registrationRequest = platformProvider.createCredentialRegistrationRequest(
        userID: vaultId.data(using: .utf8)!,
        userName: "vault_owner_\(vaultId)",
        displayName: "TTL-Legacy Vault \(vaultId)"
    )
    
    // 4. Set challenge (random data from server)
    registrationRequest.challenge = challenge.data(using: .utf8)!
    
    // 5. Configure attestation (optional, for audit trail)
    registrationRequest.attestationPreference = .direct
    
    // 6. Create authorization controller
    let authController = ASAuthorizationController(authorizationRequests: [registrationRequest])
    
    // 7. Present to user and handle response
    authController.delegate = self
    authController.performRequests()
}

// Handle successful registration
func authorizationController(
    _ controller: ASAuthorizationController,
    didCompleteWithAuthorization authorization: ASAuthorization
) {
    if let credential = authorization.credential as? ASAuthorizationPlatformPublicKeyCredentialRegistration {
        // 1. Extract public key from credential
        let publicKeyDer = credential.credentialPublicKey  // DER-encoded public key
        
        // 2. Extract attestation object (proof of credential creation)
        let attestationObject = credential.attestationObject
        
        // 3. Extract client data (challenge proof)
        let clientDataJSON = credential.rawClientDataJSON
        
        // 4. Send to backend for verification and storage
        try await registerCredentialWithBackend(
            vaultId: vaultId,
            publicKey: publicKeyDer,
            attestation: attestationObject,
            clientData: clientDataJSON
        )
    }
}
```

### Authentication (Signing)

**Code Example:**

```swift
func authenticateWithPasskey(vaultId: String, action: String) async throws -> String {
    // 1. Fetch authentication challenge from backend
    let challenge = try await fetchAuthenticationChallenge(vaultId: vaultId, action: action)
    
    // 2. Create platform authentication provider
    let platformProvider = ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier: "ttl-legacy.app")
    
    // 3. Create assertion (signing) request
    let assertionRequest = platformProvider.createCredentialAssertionRequest(challenge: challenge.data(using: .utf8)!)
    
    // 4. Create authorization controller
    let authController = ASAuthorizationController(authorizationRequests: [assertionRequest])
    
    // 5. Present to user (triggers biometric/Face ID prompt)
    authController.delegate = self
    authController.performRequests()
    
    return "pending_user_action"
}

// Handle successful authentication
func authorizationController(
    _ controller: ASAuthorizationController,
    didCompleteWithAuthorization authorization: ASAuthorization
) {
    if let credential = authorization.credential as? ASAuthorizationPlatformPublicKeyCredentialAssertion {
        // 1. Extract signature proof
        let signature = credential.signature
        
        // 2. Extract signed challenge
        let authenticatorData = credential.rawAuthenticatorData
        let clientDataJSON = credential.rawClientDataJSON
        
        // 3. Send to backend for verification
        Task {
            let result = try await verifySignatureWithBackend(
                vaultId: vaultId,
                signature: signature,
                authenticatorData: authenticatorData,
                clientData: clientDataJSON
            )
            
            // Handle result (e.g., proceed with check-in, withdrawal, etc.)
            handleAuthenticationResult(result)
        }
    }
}
```

### Key Points (iOS)

- **Relying Party ID**: Must match domain (e.g., "ttl-legacy.app")
- **User ID**: Unique identifier per vault owner (not user's Stellar address)
- **Private Key Storage**: Automatically managed by Secure Enclave on modern iPhones
- **Biometric Requirement**: Face ID / Touch ID required at time of registration and authentication
- **Attestation**: Optional but recommended for initial registration (allows backend to verify genuine Apple hardware)

## Android Implementation

### Credential Creation (Registration)

**Prerequisites:**
- Android 9.0+ (API 28+)
- Androidx credentials library
- Play Services support

**Code Example:**

```kotlin
import androidx.credentials.CreatePublicKeyCredentialRequest
import androidx.credentials.CredentialManager

suspend fun registerPasskey(vaultId: String) {
    // 1. Fetch registration challenge from backend
    val challenge = fetchRegistrationChallenge(vaultId)
    
    // 2. Build credential request (JSON in WebAuthn format)
    val createPublicKeyCredentialRequest = CreatePublicKeyCredentialRequest(
        requestJson = buildRegistrationJson(
            challenge = challenge,
            userId = vaultId,
            userName = "vault_owner_$vaultId"
        )
    )
    
    try {
        // 3. Initialize CredentialManager
        val credentialManager = CredentialManager.create(context)
        
        // 4. Request credential creation
        val result = credentialManager.createCredential(
            request = createPublicKeyCredentialRequest,
            activity = this
        )
        
        // 5. Extract registration response
        val credentialResponse = result.credential as PublicKeyCredential
        val registrationResponse = credentialResponse.registrationResponseJson
        
        // 6. Send to backend
        registerCredentialWithBackend(vaultId, registrationResponse)
    } catch (e: CreateCredentialException) {
        handleError(e)
    }
}

// Helper to build WebAuthn registration JSON
private fun buildRegistrationJson(
    challenge: String,
    userId: String,
    userName: String
): String {
    return """
    {
        "challenge": "$challenge",
        "rp": {
            "name": "TTL-Legacy",
            "id": "ttl-legacy.app"
        },
        "user": {
            "id": "$userId",
            "name": "$userName",
            "displayName": "TTL-Legacy Vault $userId"
        },
        "pubKeyCredParams": [
            {"alg": -7, "type": "public-key"},
            {"alg": -257, "type": "public-key"}
        ],
        "timeout": 60000,
        "attestation": "direct"
    }
    """.trimIndent()
}
```

### Authentication (Signing)

**Code Example:**

```kotlin
suspend fun authenticateWithPasskey(vaultId: String, action: String): String {
    // 1. Fetch authentication challenge from backend
    val challenge = fetchAuthenticationChallenge(vaultId, action)
    
    // 2. Build credential request (JSON in WebAuthn format)
    val getPublicKeyCredentialRequest = GetPublicKeyCredentialRequest(
        requestJson = buildAssertionJson(
            challenge = challenge,
            relyingPartyId = "ttl-legacy.app"
        )
    )
    
    try {
        // 3. Initialize CredentialManager
        val credentialManager = CredentialManager.create(context)
        
        // 4. Request credential assertion
        // This will trigger biometric/PIN authentication
        val result = credentialManager.getCredential(
            request = getPublicKeyCredentialRequest,
            activity = this
        )
        
        // 5. Extract authentication response
        val credentialResponse = result.credential as PublicKeyCredential
        val assertionResponse = credentialResponse.authenticationResponseJson
        
        // 6. Send to backend for verification
        val verified = verifySignatureWithBackend(vaultId, assertionResponse)
        
        return if (verified) "authenticated" else "failed"
    } catch (e: GetCredentialException) {
        handleError(e)
        return "error"
    }
}

// Helper to build WebAuthn assertion JSON
private fun buildAssertionJson(
    challenge: String,
    relyingPartyId: String
): String {
    return """
    {
        "challenge": "$challenge",
        "timeout": 60000,
        "rpId": "$relyingPartyId",
        "userVerification": "preferred",
        "allowCredentials": []
    }
    """.trimIndent()
}
```

### Key Points (Android)

- **CredentialManager**: New unified API for credential management (replaces older SafetyNet)
- **Relying Party ID**: Must match domain configuration
- **User ID**: Unique identifier per vault owner
- **Biometric**: Leverages device's biometric system (fingerprint, face, PIN)
- **Transport**: Credentials stored locally on device, synced via Google Account if enabled
- **JSON Format**: WebAuthn format JSON required for cross-platform compatibility

## Backend Registration Flow

### Verify Registration Response

The backend receives the registration response and must validate it:

```rust
pub async fn verify_registration(
    vault_id: u64,
    registration_response: RegistrationResponse,
) -> Result<PublicKey, Error> {
    // 1. Verify attestation challenge matches what was sent
    verify_challenge(&registration_response.client_data_json)?;
    
    // 2. Verify origin matches expected domain
    verify_origin(&registration_response.client_data_json, "ttl-legacy.app")?;
    
    // 3. For optional attestation verification:
    // Verify the credential is from a trusted authenticator (e.g., Apple/Google)
    if let Some(attestation_object) = registration_response.attestation_object {
        verify_attestation(&attestation_object)?;
    }
    
    // 4. Extract and validate public key
    let public_key = extract_public_key(&registration_response)?;
    
    // 5. Store public key associated with vault owner
    store_passkey(vault_id, public_key.clone()).await?;
    
    Ok(public_key)
}
```

### Verify Authentication Response

During authentication, the backend verifies the signature:

```rust
pub async fn verify_authentication(
    vault_id: u64,
    auth_response: AuthenticationResponse,
) -> Result<bool, Error> {
    // 1. Retrieve stored public key for vault owner
    let public_key = get_passkey(vault_id).await?;
    
    // 2. Verify challenge in client data matches what was sent
    verify_challenge(&auth_response.client_data_json)?;
    
    // 3. Verify origin matches expected domain
    verify_origin(&auth_response.client_data_json, "ttl-legacy.app")?;
    
    // 4. Reconstruct signed data:
    // authenticatorData || sha256(clientDataJSON)
    let signed_data = reconstruct_signed_data(
        &auth_response.authenticator_data,
        &auth_response.client_data_json,
    );
    
    // 5. Verify signature using stored public key
    let is_valid = verify_signature(
        &public_key,
        &signed_data,
        &auth_response.signature,
    );
    
    Ok(is_valid)
}
```

## Integration with TTL-Legacy Smart Contract

### Public Key Registration

When a passkey is successfully verified on the backend, the public key should be registered with the smart contract (optional, for on-chain audit trail):

```rust
// Backend contract interaction (pseudo-code)
pub fn register_passkey_with_contract(
    env: &Env,
    vault_id: u64,
    owner: &Address,
    public_key: &PublicKey,
) -> Result<(), ContractError> {
    // Call smart contract to record public key
    ttl_vault_contract::register_passkey(
        env,
        vault_id,
        owner,
        public_key.clone(),
    )
}
```

## Security Best Practices

1. **Challenge Nonce**: Every registration and authentication request must use a unique challenge (random bytes)
2. **Challenge Verification**: Backend must verify the challenge in the response matches what was sent
3. **Origin Verification**: Backend must verify that client data origin matches expected domain
4. **Relying Party ID**: Must be configured consistently across all platforms
5. **Private Key Protection**: Private keys never leave secure enclave/TEE
6. **Biometric Requirement**: Require biometric/device credential at registration and authentication
7. **Attestation**: Use attestation when registering new credentials to verify genuine authenticator
8. **Session Management**: After successful authentication, issue short-lived session token
9. **Rate Limiting**: Implement rate limiting on authentication attempts to prevent brute force
10. **Logging**: Log all successful and failed authentication attempts with timestamp and device info

## Troubleshooting

### iOS Issues

| Issue | Solution |
|-------|----------|
| "User cancelled" error | Normal user cancellation; retry is allowed |
| "Not interactive" error | Must be called from UIViewController in foreground |
| "Invalid domain" error | Verify relying party ID matches domain and associated domain is properly configured |
| Biometric not working | Ensure device has Face ID/Touch ID enrolled and enabled |

### Android Issues

| Issue | Solution |
|-------|----------|
| "User cancelled" error | Normal user cancellation; retry is allowed |
| "PasskeyNotSupportedPermanently" | Device doesn't support passkeys; fall back to alternative auth |
| "Invalid challenge format" | Challenge must be base64url-encoded |
| Credential not found | Ensure credential was successfully registered prior |

## References

- [Apple AuthenticationServices Documentation](https://developer.apple.com/documentation/authenticationservices)
- [Android CredentialManager Documentation](https://developer.android.com/training/sign-in/credential-manager)
- [W3C WebAuthn Specification](https://www.w3.org/TR/webauthn/)
- [FIDO2 Overview](https://fidoalliance.org/fido2/)
