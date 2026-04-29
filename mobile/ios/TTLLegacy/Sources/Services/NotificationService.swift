import UserNotifications
import Foundation

final class NotificationService: NSObject, UNUserNotificationCenterDelegate {
    static let shared = NotificationService()
    private override init() {
        super.init()
        UNUserNotificationCenter.current().delegate = self
    }

    func requestPermission() async {
        let granted = (try? await UNUserNotificationCenter.current()
            .requestAuthorization(options: [.alert, .badge, .sound])) ?? false
        if granted { await registerForRemoteNotifications() }
    }

    @MainActor
    private func registerForRemoteNotifications() {
        UIApplication.shared.registerForRemoteNotifications()
    }

    func handleDeviceToken(_ tokenData: Data) {
        let token = tokenData.map { String(format: "%02x", $0) }.joined()
        Task { try? await APIClient.shared.registerPushToken(token) }
    }

    // Schedule a local check-in reminder
    func scheduleCheckInReminder(vaultID: String, vaultName: String, ttlRemaining: UInt64) {
        let center = UNUserNotificationCenter.current()
        center.removePendingNotificationRequests(withIdentifiers: ["checkin-\(vaultID)"])

        guard ttlRemaining > 0 else { return }
        let fireIn = max(Int(ttlRemaining) - 86_400, 60) // 24h before expiry, min 1 min

        let content = UNMutableNotificationContent()
        content.title = "Check-in Required"
        content.body = "Your vault expires in ~24 hours. Tap to check in now."
        content.sound = .default
        content.userInfo = ["vault_id": vaultID]
        content.categoryIdentifier = "CHECK_IN"

        let trigger = UNTimeIntervalNotificationTrigger(timeInterval: TimeInterval(fireIn), repeats: false)
        let request = UNNotificationRequest(identifier: "checkin-\(vaultID)", content: content, trigger: trigger)
        center.add(request)
    }

    // MARK: - UNUserNotificationCenterDelegate

    func userNotificationCenter(_ center: UNUserNotificationCenter,
                                 didReceive response: UNNotificationResponse,
                                 withCompletionHandler completionHandler: @escaping () -> Void) {
        let vaultID = response.notification.request.content.userInfo["vault_id"] as? String
        if response.actionIdentifier == "CHECK_IN_ACTION", let id = vaultID {
            Task { try? await APIClient.shared.checkIn(vaultID: id) }
        }
        completionHandler()
    }

    func registerNotificationCategories() {
        let checkInAction = UNNotificationAction(identifier: "CHECK_IN_ACTION", title: "Check In", options: .foreground)
        let category = UNNotificationCategory(identifier: "CHECK_IN", actions: [checkInAction],
                                               intentIdentifiers: [], options: [])
        UNUserNotificationCenter.current().setNotificationCategories([category])
    }
}
