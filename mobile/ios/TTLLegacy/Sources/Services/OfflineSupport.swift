import Network
import Foundation
import CryptoKit

// MARK: - Network Monitor

final class NetworkMonitor {
    static let shared = NetworkMonitor()
    private let monitor = NWPathMonitor()
    private let queue = DispatchQueue(label: "com.ttllegacy.NetworkMonitor")
    private(set) var isConnected = true

    /// Publishes true when connectivity is restored after being offline.
    var onConnectivityRestored: (() -> Void)?

    private init() {
        monitor.pathUpdateHandler = { [weak self] path in
            guard let self else { return }
            let wasOffline = !self.isConnected
            self.isConnected = path.status == .satisfied
            if wasOffline && self.isConnected {
                self.onConnectivityRestored?()
            }
        }
        monitor.start(queue: queue)
    }
}

// MARK: - Queued Check-In

/// A signed (but not yet submitted) check-in transaction stored locally.
struct QueuedCheckIn: Codable {
    let vaultID: String
    /// ISO-8601 timestamp when the check-in was queued.
    let queuedAt: Date
    /// Passkey-signed payload (base64url) ready for submission.
    let signedPayload: String
}

// MARK: - Offline Check-In Queue

/// Persists queued check-ins to disk and flushes them when connectivity is restored.
final class OfflineCheckInQueue {
    static let shared = OfflineCheckInQueue()

    private let fileURL: URL
    private var items: [QueuedCheckIn] = []
    private let lock = NSLock()

    private init() {
        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("TTLLegacy", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        fileURL = dir.appendingPathComponent("offline_checkin_queue.json")
        items = load()

        // Auto-flush when network comes back.
        NetworkMonitor.shared.onConnectivityRestored = { [weak self] in
            Task { await self?.flushQueue() }
        }
    }

    var count: Int {
        lock.lock(); defer { lock.unlock() }
        return items.count
    }

    var isEmpty: Bool { count == 0 }

    /// Enqueue a signed check-in for later submission.
    func enqueue(_ checkIn: QueuedCheckIn) {
        lock.lock()
        // Replace any existing entry for the same vault (idempotent).
        items.removeAll { $0.vaultID == checkIn.vaultID }
        items.append(checkIn)
        lock.unlock()
        persist()
    }

    /// Remove a successfully submitted check-in.
    func remove(vaultID: String) {
        lock.lock()
        items.removeAll { $0.vaultID == vaultID }
        lock.unlock()
        persist()
    }

    /// Returns a snapshot of all queued items in insertion order.
    func allItems() -> [QueuedCheckIn] {
        lock.lock(); defer { lock.unlock() }
        return items
    }

    // MARK: Flush

    /// Submit all queued check-ins in order. Removes each one after a successful HTTP response.
    /// If connectivity is lost mid-flush, stops and leaves remaining items queued.
    /// If a check-in arrives after TTL expiry (HTTP 409/410), logs a warning but still dequeues
    /// the item (the server will attempt the submission and return the appropriate error).
    func flushQueue() async {
        guard !isEmpty else { return }
        let snapshot = allItems()
        for item in snapshot {
            guard NetworkMonitor.shared.isConnected else { break }
            do {
                let result = try await APIClient.shared.submitQueuedCheckIn(item)
                switch result {
                case .success:
                    remove(vaultID: item.vaultID)
                    NotificationService.shared.postQueueFlushSuccess(vaultID: item.vaultID)
                case .expiredTTL:
                    // Queue arrived after TTL expiry — warn user, dequeue anyway.
                    remove(vaultID: item.vaultID)
                    NotificationService.shared.postCheckInExpiredWarning(vaultID: item.vaultID)
                case .networkUnavailable:
                    break // Will retry on next connectivity-restored event.
                }
            } catch {
                // Unexpected error — leave item in queue for next attempt.
            }
        }
    }
}

// MARK: - Persistence

private extension OfflineCheckInQueue {
    func persist() {
        lock.lock()
        let snapshot = items
        lock.unlock()
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        if let data = try? encoder.encode(snapshot) {
            try? data.write(to: fileURL, options: .atomic)
        }
    }

    func load() -> [QueuedCheckIn] {
        guard let data = try? Data(contentsOf: fileURL) else { return [] }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return (try? decoder.decode([QueuedCheckIn].self, from: data)) ?? []
    }
}

// MARK: - Offline Cache (unchanged)

/// Simple disk-based cache for offline reads.
final class OfflineCache {
    static let shared = OfflineCache()
    private let dir: URL

    private init() {
        dir = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("TTLLegacyOfflineCache", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    }

    func save(_ data: Data, for key: String) {
        let file = dir.appendingPathComponent(key.sha256Hex)
        try? data.write(to: file)
    }

    func load(for key: String) -> Data? {
        let file = dir.appendingPathComponent(key.sha256Hex)
        return try? Data(contentsOf: file)
    }
}

extension String {
    var sha256Hex: String {
        let digest = SHA256.hash(data: Data(utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }
}
