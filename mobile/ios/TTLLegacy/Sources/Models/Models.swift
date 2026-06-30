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
