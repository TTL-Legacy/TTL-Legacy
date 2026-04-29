import SwiftUI

@main
struct TTLLegacyApp: App {
    @StateObject private var authStore = AuthStore()
    @StateObject private var vaultStore = VaultStore()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(authStore)
                .environmentObject(vaultStore)
                .task { await NotificationService.shared.requestPermission() }
        }
    }
}
