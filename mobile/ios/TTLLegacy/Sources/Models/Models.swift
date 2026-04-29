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
