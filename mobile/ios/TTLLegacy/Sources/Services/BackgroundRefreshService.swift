import BackgroundTasks
import Foundation

// Requires "app.ttl-legacy.vault-ttl-refresh" in BGTaskSchedulerPermittedIdentifiers (Info.plist).
final class BackgroundRefreshService {
    static let shared = BackgroundRefreshService()
    static let taskIdentifier = "app.ttl-legacy.vault-ttl-refresh"

    private init() {}

    func registerBackgroundTask() {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: Self.taskIdentifier, using: nil) { [weak self] task in
            self?.handleRefresh(task: task as! BGAppRefreshTask)
        }
    }

    func scheduleAppRefresh() {
        let request = BGAppRefreshTaskRequest(identifier: Self.taskIdentifier)
        request.earliestBeginDate = Date(timeIntervalSinceNow: 3_600) // poll every hour
        try? BGTaskScheduler.shared.submit(request)
    }

    private func handleRefresh(task: BGAppRefreshTask) {
        scheduleAppRefresh()

        let refreshTask = Task {
            do {
                let vaults = try await APIClient.shared.listVaults()
                for vault in vaults where vault.status == .active {
                    if let ttl = vault.ttlRemaining, ttl < 86_400 {
                        NotificationService.shared.scheduleTTLWarning(vaultID: vault.id, ttlRemaining: ttl)
                    }
                }
                task.setTaskCompleted(success: true)
            } catch {
                task.setTaskCompleted(success: false)
            }
        }

        task.expirationHandler = { refreshTask.cancel() }
    }
}
