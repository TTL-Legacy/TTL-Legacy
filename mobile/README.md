# Mobile App Architecture

## Overview

TTL-Legacy mobile apps (iOS + Android) provide a native interface for managing vaults, checking in, and receiving expiry reminders. Both apps share the same REST API contract and feature set.

## Structure

```
mobile/
├── shared/
│   └── api-contract.md          # Shared API spec (iOS + Android)
├── ios/TTLLegacy/
│   └── Sources/
│       ├── App/                 # Entry point, app lifecycle
│       ├── Models/              # Vault, AuthToken, etc.
│       ├── Services/
│       │   ├── APIClient.swift      # Ktor-style async HTTP client
│       │   ├── PasskeyService.swift # ASAuthorization / WebAuthn
│       │   ├── KeychainService.swift# Secure token storage
│       │   ├── NotificationService.swift # APNs + local reminders
│       │   └── OfflineSupport.swift # NetworkMonitor + disk cache
│       ├── ViewModels/          # AuthStore, VaultStore (ObservableObject)
│       └── Views/               # SwiftUI screens
└── android/app/src/main/java/com/ttllegacy/
    ├── api/
    │   ├── ApiClient.kt         # Ktor HTTP client
    │   └── Infrastructure.kt    # NetworkMonitor, OfflineCache, TokenProvider
    ├── models/                  # Kotlinx.serialization data classes
    ├── services/
    │   ├── PasskeyService.kt    # CredentialManager / WebAuthn
    │   ├── PushService.kt       # Firebase Messaging
    │   └── NotificationHelper.kt# Local notification display
    ├── ui/
    │   ├── ViewModels.kt        # AuthViewModel, VaultViewModel (Hilt)
    │   ├── MainActivity.kt      # NavHost entry point
    │   ├── screens/Screens.kt   # Compose screens
    │   └── theme/Theme.kt       # Material3 dynamic color
    └── di/AppModule.kt          # Hilt DI bindings
```

## Key Design Decisions

### Passkey Authentication (WebAuthn)
- **iOS**: `ASAuthorizationPlatformPublicKeyCredentialProvider` (iOS 16+)
- **Android**: `CredentialManager` API (Android 9+, API 28+)
- Flow: `getChallenge()` → device biometric prompt → `verifyPasskey()` → JWT stored in Keychain/SharedPreferences
- Relying party: `ttl-legacy.app` (requires `.well-known/assetlinks.json` + Apple App Site Association)

### Push Notifications
- **iOS**: APNs via `UNUserNotificationCenter`. Device token registered to backend on first launch.
  - Local reminders scheduled 24h before vault expiry via `UNTimeIntervalNotificationTrigger`
  - Actionable notification category `CHECK_IN` with inline "Check In" action
- **Android**: Firebase Cloud Messaging (FCM). Token refreshed via `onNewToken`.
  - Notification channel `ttl_reminders` (IMPORTANCE_HIGH)
  - Deep-link intent to `MainActivity` with `vault_id` extra

### Offline Support
- `NetworkMonitor` checks live connectivity before every request
- `OfflineCache` stores last successful GET responses keyed by URL (SHA-256 filename)
- On network unavailable: cached data served transparently; mutations show "offline" error
- iOS: `CryptoKit.SHA256` for cache keys; Android: `MessageDigest("SHA-256")`

### State Management
- **iOS**: `@StateObject` / `ObservableObject` stores (`AuthStore`, `VaultStore`) injected via SwiftUI environment
- **Android**: Hilt-injected `ViewModel`s with `StateFlow` + `collectAsStateWithLifecycle`

## Setup

### iOS
1. Open `mobile/ios/TTLLegacy` in Xcode 15+
2. Set bundle ID and team in signing settings
3. Add `API_BASE_URL` to `Info.plist`
4. Configure Apple App Site Association at `https://ttl-legacy.app/.well-known/apple-app-site-association`
5. Enable Push Notifications + Associated Domains capabilities

### Android
1. Open `mobile/android` in Android Studio Hedgehog+
2. Add `google-services.json` from Firebase Console
3. Configure `assetlinks.json` at `https://ttl-legacy.app/.well-known/assetlinks.json`
4. Set `API_BASE_URL` in `build.gradle.kts` `buildConfigField`

## Testing

### iOS
```bash
cd mobile/ios/TTLLegacy
swift test
```
Covers: model decoding, Keychain round-trip, offline cache, Base64URL encoding.

### Android
```bash
cd mobile/android
./gradlew test                  # Unit tests (JVM)
./gradlew connectedAndroidTest  # Instrumented tests (device/emulator)
```
Covers: ViewModel state transitions, model logic, Compose UI smoke tests.
