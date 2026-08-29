# Offline Check-In Queue — Shared Design

## Overview

When a vault owner opens the mobile app without network connectivity, they can still
perform a check-in. The app signs the transaction locally using the device's Passkey
(WebAuthn) and places it in a local queue. When connectivity is restored the queue is
automatically flushed and each signed transaction is submitted to the backend in order.

## Queue Lifecycle

```
[User taps Check In] ──► [Online?]
                              │ Yes ──► Submit to API immediately
                              │ No  ──► Sign locally → Enqueue (disk-persisted)
                                            │
                              [Network restored]
                                            │
                                    Flush queue (FIFO)
                                            │
                              ┌─────────────┴─────────────┐
                              │ Success                   │ TTL already expired
                              │ → Dequeue                 │ → Warn user, dequeue
                              │ → Notify user             │
                              └───────────────────────────┘
```

## Platform Implementations

### iOS (`OfflineSupport.swift`)

- `NetworkMonitor` — wraps `NWPathMonitor`; fires `onConnectivityRestored` callback.
- `QueuedCheckIn` — `Codable` struct: `vaultID`, `queuedAt` (ISO-8601), `signedPayload` (base64url).
- `OfflineCheckInQueue` — singleton; persists queue as JSON in Application Support.
  - `enqueue(_:)` — idempotent per vault (replaces existing entry for the same vault).
  - `flushQueue()` — async; submits in FIFO order, stops on connectivity loss.
  - `remove(vaultID:)` — called after successful submission.
- `OfflineQueueBanner` — SwiftUI view; shows red banner when offline, orange when syncing.
- `OfflineStatusViewModel` — `ObservableObject`; polls every 2 s for connectivity + queue count.

### Android (`CheckInQueue.kt`, `CheckInSyncWorker.kt`)

- `PendingCheckIn` — Room entity: `vaultId`, `signedPayload`, `queuedAt`, `ttlExpiresAt`.
- `PendingCheckInDao` — Room DAO with `getAll`, `getExpired`, `insert`, `delete`, `deleteAll`.
- `CheckInSyncWorker` — `CoroutineWorker` constrained to `CONNECTED` network.
  - Warns via notification when expired items are detected before submission.
  - Posts success notification after each successfully synced check-in.
  - Retries on `NetworkUnavailable`; dequeues on `Error` (server-side error).
- `NotificationHelper` — `notifyExpiredCheckInsInQueue` and `notifyCheckInSyncSuccess` added.

## Banner / UI Behaviour

| State | iOS | Android |
|---|---|---|
| Online, queue empty | No banner | No banner |
| Offline | Red banner: "You're offline" | System notification: queued check-in pending |
| Online, queue flushing | Orange banner: "Syncing queued check-ins…" | Progress notification |
| Sync success | Banner dismisses | "✅ Check-in synced" notification |
| TTL expired on flush | In-app alert | "⚠️ Check-in may be too late" notification |

## Conflict Case: TTL Expiry During Queue

If the vault's TTL expires while a check-in is queued:

1. The sync worker detects `ttlExpiresAt < now` (Android) or receives an HTTP 409/410 (iOS).
2. Both platforms **still attempt submission** — the backend decides the final outcome.
3. The user is shown a warning notification/alert so they can check vault status manually.
4. The item is removed from the queue regardless of the server response to prevent
   indefinite retries on an already-expired vault.

## Testing

Unit test files:
- iOS: `mobile/ios/TTLLegacy/Tests/TTLLegacyTests.swift` — add tests for `OfflineCheckInQueue`
  (enqueue, dequeue, persistence, flush ordering).
- Android: `mobile/android/app/src/test/` — add `OfflineCheckInQueueTest.kt` for DAO and
  worker logic.
