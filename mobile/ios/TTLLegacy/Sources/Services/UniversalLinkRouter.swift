import Foundation

final class UniversalLinkRouter {
    static let shared = UniversalLinkRouter()
    private init() {}

    enum VaultAction: String, Equatable {
        case checkIn = "check-in"
        case withdraw = "withdraw"
        case viewDetails = "view-details"
        case manageBeneficiary = "manage-beneficiary"
    }

    enum DeepLink: Equatable {
        case vaultInvitation(vaultID: String)
        case beneficiaryAcceptance(vaultID: String, token: String)
        case vaultAction(vaultID: String, action: VaultAction)
    }

    /// Parses a universal link or custom-scheme URL into a typed DeepLink, or returns nil if unrecognised.
    func parse(url: URL) -> DeepLink? {
        // ttllegacy://vault/{vault_id}/{action}
        if url.scheme == "ttllegacy", url.host == "vault" {
            let parts = url.pathComponents.filter { $0 != "/" }
            guard parts.count == 2, let action = VaultAction(rawValue: parts[1]) else { return nil }
            return .vaultAction(vaultID: parts[0], action: action)
        }

        guard url.host == "ttl-legacy.app" else { return nil }
        let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
        let parts = url.pathComponents.filter { $0 != "/" }

        // /vaults/{vaultID}/invite
        if parts.count == 3, parts[0] == "vaults", parts[2] == "invite" {
            return .vaultInvitation(vaultID: parts[1])
        }

        // /vaults/{vaultID}/accept?token={token}
        if parts.count == 3, parts[0] == "vaults", parts[2] == "accept" {
            let token = components?.queryItems?.first(where: { $0.name == "token" })?.value ?? ""
            return .beneficiaryAcceptance(vaultID: parts[1], token: token)
        }

        return nil
    }
}
