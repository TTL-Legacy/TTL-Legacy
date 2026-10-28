import Foundation

struct Vault: Codable, Identifiable, Equatable {
    let id: String
    let owner: String
    let beneficiary: String
    let balance: Int64
    let checkInInterval: UInt64
    let lastCheckIn: Date
    let ttlRemaining: UInt64?
    let status: VaultStatus

    enum VaultStatus: String, Codable {
        case active, expired, released, paused
    }

    var isExpiringSoon: Bool {
        guard let ttl = ttlRemaining else { return false }
        return ttl < 86_400 // < 24 hours
    }

    var formattedBalance: String {
        let xlm = Double(balance) / 10_000_000
        return String(format: "%.7f XLM", xlm)
    }
}

struct AuthChallenge: Codable {
    let challenge: String
    let expiresAt: Date
}

struct AuthToken: Codable {
    let token: String
    let expiresAt: Date
}

struct PushRegistration: Codable {
    let token: String
    let platform: String  // "ios" | "android"
}

// MARK: - 2FA Models

enum TwoFactorMethod: String, Codable, CaseIterable {
    case totp
    case sms
    case email
}

struct TwoFactorStatus: Codable {
    let vaultId: String
    let enabled: Bool
    let method: TwoFactorMethod?
    let verified: Bool
    let phone: String?
    let email: String?
}

struct Enable2FARequest: Codable {
    let method: TwoFactorMethod
    let phone: String?
    let email: String?
}

struct Enable2FAResponse: Codable {
    let vaultId: String
    let method: TwoFactorMethod
    let secret: String?
    let provisioningUri: String?
}

struct Verify2FARequest: Codable {
    let otp: String
}

// MARK: - Proof-of-Life Models (Issue #1842)

/// A proof-of-life submission sent to `POST /api/vaults/{id}/proof-of-life`.
///
/// The `proofHash` is the lowercase hex-encoded SHA-256 digest of the image
/// data the user captured with the camera.  Raw image bytes are never sent to
/// the server; only the commitment hash is transmitted.
struct ProofOfLifeRequest: Codable {
    let beneficiary: String
    /// Hex-encoded SHA-256 hash of the captured image data.
    let proof: String
    /// How long (in seconds) the proof is considered valid.  Defaults to 30 days.
    let validitySeconds: UInt64?
}

/// Response returned by the server after a successful proof-of-life submission.
struct ProofOfLifeResponse: Codable {
    let status: String
    let vaultId: String
    let beneficiary: String
}

/// Lightweight summary displayed in the vault detail after a proof has been submitted.
struct ProofOfLifeSummary: Equatable {
    let submittedAt: Date
    let validUntil: Date
    let proofHash: String

    var isValid: Bool { validUntil > Date() }

    var formattedExpiry: String {
        let f = DateFormatter()
        f.dateStyle = .medium
        f.timeStyle = .short
        return f.string(from: validUntil)
    }
}
