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

    // Fires immediately to warn the user their vault TTL is under 24 hours (called from background refresh).
    func scheduleTTLWarning(vaultID: String, ttlRemaining: UInt64) {
        let center = UNUserNotificationCenter.current()
        center.removePendingNotificationRequests(withIdentifiers: ["ttl-warning-\(vaultID)"])

        let content = UNMutableNotificationContent()
        content.title = "Vault Expiring Soon"
        content.body = "Your vault expires in less than 24 hours. Open the app to check in and keep it active."
        content.sound = .default
        content.userInfo = ["vault_id": vaultID]
        content.categoryIdentifier = "CHECK_IN"

        let trigger = UNTimeIntervalNotificationTrigger(timeInterval: 5, repeats: false)
        let request = UNNotificationRequest(identifier: "ttl-warning-\(vaultID)", content: content, trigger: trigger)
        center.add(request)
    }

    func registerNotificationCategories() {
        let checkInAction = UNNotificationAction(identifier: "CHECK_IN_ACTION", title: "Check In", options: .foreground)
        let category = UNNotificationCategory(identifier: "CHECK_IN", actions: [checkInAction],
                                               intentIdentifiers: [], options: [])
        UNUserNotificationCenter.current().setNotificationCategories([category])
    }

    // MARK: - APNs Action Categories

    /// Registers the notification action categories required for APNs interactive notifications.
    /// Call this once during app launch (e.g. from `NotificationService.shared.requestPermission()`).
    func registerNotificationActionsForAPNs() {
        registerNotificationCategories()
    }

    // MARK: - Remote Notification Dispatch

    /// Dispatches an incoming APNs payload to the appropriate local notification helper.
    ///
    /// Recognised `type` values:
    /// - `expiry_warning`     → schedules an immediate TTL-warning local notification
    /// - `check_in_reminder`  → schedules a check-in reminder using the provided `ttl_remaining`
    /// - `vault_released`     → fires a "Vault Released" local notification
    func handleRemoteNotification(userInfo: [AnyHashable: Any]) {
        guard let vaultID = userInfo["vault_id"] as? String else { return }
        let type = userInfo["type"] as? String ?? ""

        switch type {
        case "expiry_warning":
            scheduleTTLWarning(vaultID: vaultID, ttlRemaining: 0)

        case "check_in_reminder":
            let ttlRemaining: UInt64
            if let raw = userInfo["ttl_remaining"] as? UInt64 {
                ttlRemaining = raw
            } else if let raw = userInfo["ttl_remaining"] as? Int {
                ttlRemaining = UInt64(max(raw, 0))
            } else {
                ttlRemaining = 86_400 // default: 24 h
            }
            let vaultName = userInfo["vault_name"] as? String ?? vaultID
            scheduleCheckInReminder(vaultID: vaultID, vaultName: vaultName, ttlRemaining: ttlRemaining)

        case "vault_released":
            let center = UNUserNotificationCenter.current()
            center.removePendingNotificationRequests(withIdentifiers: ["vault-released-\(vaultID)"])

            let content = UNMutableNotificationContent()
            content.title = "Vault Released"
            content.body = "Your vault has been released to the beneficiary."
            content.sound = .default
            content.userInfo = ["vault_id": vaultID]

            let trigger = UNTimeIntervalNotificationTrigger(timeInterval: 1, repeats: false)
            let request = UNNotificationRequest(identifier: "vault-released-\(vaultID)",
                                                content: content, trigger: trigger)
            center.add(request)

        default:
            // Unknown type — schedule a generic TTL warning so APNs silent push still wakes the UI.
            scheduleTTLWarning(vaultID: vaultID, ttlRemaining: 0)
        }
    }
}
