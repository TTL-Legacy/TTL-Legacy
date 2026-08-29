import UIKit

/// `AppDelegate` bridges UIKit APNs lifecycle callbacks into the SwiftUI app.
///
/// Wired up via `@UIApplicationDelegateAdaptor(AppDelegate.self)` in
/// `TTLLegacyApp`. This is the recommended pattern for accessing APNs
/// device-token registration callbacks that SwiftUI does not expose natively.
class AppDelegate: NSObject, UIApplicationDelegate {

    // MARK: - Remote Notification Registration

    func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        NotificationService.shared.handleDeviceToken(deviceToken)
    }

    func application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        print("[APNs] Failed to register: \(error.localizedDescription)")
    }

    // MARK: - Background / Silent Push

    func application(
        _ application: UIApplication,
        didReceiveRemoteNotification userInfo: [AnyHashable: Any],
        fetchCompletionHandler completionHandler: @escaping (UIBackgroundFetchResult) -> Void
    ) {
        NotificationService.shared.handleRemoteNotification(userInfo: userInfo)
        completionHandler(.newData)
    }
}
