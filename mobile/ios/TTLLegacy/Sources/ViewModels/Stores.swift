import Foundation
import Combine

@MainActor
final class AuthStore: ObservableObject {
    @Published var isAuthenticated = false
    @Published var isLoading = false
    @Published var error: String?

    init() {
        isAuthenticated = KeychainService.shared.loadToken() != nil
    }

    func signIn() async {
        isLoading = true; error = nil
        do {
            let token = try await PasskeyService.shared.authenticate()
            KeychainService.shared.saveToken(token.token)
            isAuthenticated = true
        } catch {
            self.error = error.localizedDescription
        }
        isLoading = false
    }

    func register(username: String) async {
        isLoading = true; error = nil
        do {
            let credID = try await PasskeyService.shared.register(username: username)
            KeychainService.shared.saveCredentialID(credID)
            await signIn()
        } catch {
            self.error = error.localizedDescription
        }
        isLoading = false
    }

    func signOut() {
        KeychainService.shared.deleteToken()
        isAuthenticated = false
    }
}

@MainActor
final class VaultStore: ObservableObject {
    @Published var vaults: [Vault] = []
    @Published var isLoading = false
    @Published var error: String?

    func load() async {
        isLoading = true; error = nil
        do {
            vaults = try await APIClient.shared.listVaults()
            scheduleReminders()
        } catch APIError.networkUnavailable {
            // Vaults already populated from offline cache via APIClient
        } catch {
            self.error = error.localizedDescription
        }
        isLoading = false
    }

    func checkIn(vault: Vault) async {
        do {
            try await APIClient.shared.checkIn(vaultID: vault.id)
            await load()
        } catch { self.error = error.localizedDescription }
    }

    private func scheduleReminders() {
        for vault in vaults where vault.status == .active {
            if let ttl = vault.ttlRemaining {
                NotificationService.shared.scheduleCheckInReminder(
                    vaultID: vault.id, vaultName: vault.id, ttlRemaining: ttl)
            }
        }
    }
}
