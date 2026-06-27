import SwiftUI

@main
struct TTLLegacyApp: App {
    @StateObject private var authStore = AuthStore()
    @StateObject private var vaultStore = VaultStore()

    init() {
        BackgroundRefreshService.shared.registerBackgroundTask()
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(authStore)
                .environmentObject(vaultStore)
                .task {
                    await NotificationService.shared.requestPermission()
                    BackgroundRefreshService.shared.scheduleAppRefresh()
                }
                .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { activity in
                    guard let url = activity.webpageURL else { return }
                    vaultStore.pendingDeepLink = UniversalLinkRouter.shared.parse(url: url)
                }
        }
    }
}
