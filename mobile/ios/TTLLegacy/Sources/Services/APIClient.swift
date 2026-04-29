import Foundation

enum APIError: LocalizedError {
    case unauthorized
    case notFound
    case serverError(String)
    case networkUnavailable
    case decodingFailed

    var errorDescription: String? {
        switch self {
        case .unauthorized:       return "Authentication required"
        case .notFound:           return "Resource not found"
        case .serverError(let m): return m
        case .networkUnavailable: return "No internet connection"
        case .decodingFailed:     return "Invalid server response"
        }
    }
}

final class APIClient {
    static let shared = APIClient()

    private let baseURL: URL
    private let session: URLSession
    private let decoder: JSONDecoder

    private init() {
        let urlString = Bundle.main.object(forInfoDictionaryKey: "API_BASE_URL") as? String
            ?? "https://api.ttl-legacy.app/v1"
        baseURL = URL(string: urlString)!
        session = URLSession(configuration: .default)
        decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        decoder.dateDecodingStrategy = .iso8601
    }

    // MARK: - Auth

    func getChallenge() async throws -> AuthChallenge {
        try await post(path: "/auth/challenge", body: EmptyBody())
    }

    func verifyPasskey(credentialID: String, clientDataJSON: String, signature: String) async throws -> AuthToken {
        let body = ["credential_id": credentialID,
                    "client_data_json": clientDataJSON,
                    "signature": signature]
        return try await post(path: "/auth/verify", body: body)
    }

    func registerPasskey(credentialID: String, publicKey: String, clientDataJSON: String) async throws {
        let body = ["credential_id": credentialID,
                    "public_key": publicKey,
                    "client_data_json": clientDataJSON]
        let _: EmptyBody = try await post(path: "/auth/register", body: body)
    }

    // MARK: - Vaults

    func listVaults() async throws -> [Vault] {
        try await get(path: "/vaults")
    }

    func getVault(id: String) async throws -> Vault {
        try await get(path: "/vaults/\(id)")
    }

    func createVault(beneficiary: String, checkInInterval: UInt64) async throws -> Vault {
        let body = ["beneficiary": beneficiary, "check_in_interval": checkInInterval] as [String: Any]
        return try await post(path: "/vaults", body: body)
    }

    func checkIn(vaultID: String) async throws {
        let _: EmptyBody = try await post(path: "/vaults/\(vaultID)/checkin", body: EmptyBody())
    }

    func deposit(vaultID: String, amount: Int64) async throws -> Vault {
        try await post(path: "/vaults/\(vaultID)/deposit", body: ["amount": amount])
    }

    func withdraw(vaultID: String, amount: Int64) async throws -> Vault {
        try await post(path: "/vaults/\(vaultID)/withdraw", body: ["amount": amount])
    }

    func getTTL(vaultID: String) async throws -> UInt64 {
        let result: [String: UInt64] = try await get(path: "/vaults/\(vaultID)/ttl")
        return result["ttl_remaining"] ?? 0
    }

    // MARK: - Push Notifications

    func registerPushToken(_ token: String) async throws {
        let body = PushRegistration(token: token, platform: "ios")
        let _: EmptyBody = try await post(path: "/notifications/register", body: body)
    }

    func unregisterPushToken(_ token: String) async throws {
        var req = request(path: "/notifications/register")
        req.httpMethod = "DELETE"
        req.httpBody = try? JSONEncoder().encode(PushRegistration(token: token, platform: "ios"))
        _ = try await execute(req)
    }

    // MARK: - Private helpers

    private func get<T: Decodable>(path: String) async throws -> T {
        var req = request(path: path)
        req.httpMethod = "GET"
        let data = try await execute(req)
        return try decode(data)
    }

    private func post<B: Encodable, T: Decodable>(path: String, body: B) async throws -> T {
        var req = request(path: path)
        req.httpMethod = "POST"
        req.httpBody = try JSONEncoder().encode(body)
        let data = try await execute(req)
        return try decode(data)
    }

    private func request(path: String) -> URLRequest {
        var req = URLRequest(url: baseURL.appendingPathComponent(path))
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        if let token = KeychainService.shared.loadToken() {
            req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        return req
    }

    private func execute(_ request: URLRequest) async throws -> Data {
        guard NetworkMonitor.shared.isConnected else {
            // Return cached data if available
            if let cached = OfflineCache.shared.load(for: request.url?.absoluteString ?? "") {
                return cached
            }
            throw APIError.networkUnavailable
        }
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw APIError.serverError("Invalid response") }
        switch http.statusCode {
        case 200...299:
            OfflineCache.shared.save(data, for: request.url?.absoluteString ?? "")
            return data
        case 401: throw APIError.unauthorized
        case 404: throw APIError.notFound
        default:
            let msg = (try? JSONDecoder().decode([String: String].self, from: data))?["error"] ?? "Server error"
            throw APIError.serverError(msg)
        }
    }

    private func decode<T: Decodable>(_ data: Data) throws -> T {
        if T.self == EmptyBody.self { return EmptyBody() as! T }
        do { return try decoder.decode(T.self, from: data) }
        catch { throw APIError.decodingFailed }
    }
}

private struct EmptyBody: Codable {}
