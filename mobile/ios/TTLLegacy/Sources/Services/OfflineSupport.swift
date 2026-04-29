import Network
import Foundation

final class NetworkMonitor {
    static let shared = NetworkMonitor()
    private let monitor = NWPathMonitor()
    private(set) var isConnected = true

    private init() {
        monitor.pathUpdateHandler = { [weak self] path in
            self?.isConnected = path.status == .satisfied
        }
        monitor.start(queue: DispatchQueue(label: "NetworkMonitor"))
    }
}

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

import CryptoKit
extension String {
    var sha256Hex: String {
        let digest = SHA256.hash(data: Data(utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }
}
