import Foundation

final class UniversalLinkRouter {
    static let shared = UniversalLinkRouter()
    private init() {}

    enum DeepLink: Equatable {
        case vaultInvitation(vaultID: String)
        case beneficiaryAcceptance(vaultID: String, token: String)
    }

    /// Parses a universal link URL into a typed DeepLink, or returns nil if unrecognised.
    func parse(url: URL) -> DeepLink? {
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
