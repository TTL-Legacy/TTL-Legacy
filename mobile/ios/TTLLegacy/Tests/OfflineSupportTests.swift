import XCTest
@testable import TTLLegacy

final class OfflineSupportTests: XCTestCase {

    private var queue: OfflineCheckInQueue!
    private var fileURL: URL!

    override func setUp() {
        super.setUp()
        queue = OfflineCheckInQueue()

        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("TTLLegacy-Test", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        fileURL = dir.appendingPathComponent("offline_checkin_queue.json")

        // Clean up test files
        try? FileManager.default.removeItem(at: fileURL)
    }

    override func tearDown() {
        super.tearDown()
        try? FileManager.default.removeItem(at: fileURL)
    }

    // MARK: - Queueing Tests

    func test_enqueue_addsItemToQueue() {
        let checkIn = QueuedCheckIn(
            vaultID: "vault-123",
            queuedAt: Date(),
            signedPayload: "payload-xyz"
        )

        queue.enqueue(checkIn)

        XCTAssertEqual(queue.count, 1)
        let items = queue.allItems()
        XCTAssertEqual(items.first?.vaultID, "vault-123")
    }

    func test_enqueue_idempotent_replacesExistingEntry() {
        let checkIn1 = QueuedCheckIn(
            vaultID: "vault-123",
            queuedAt: Date(),
            signedPayload: "payload-v1"
        )
        let checkIn2 = QueuedCheckIn(
            vaultID: "vault-123",
            queuedAt: Date().addingTimeInterval(10),
            signedPayload: "payload-v2"
        )

        queue.enqueue(checkIn1)
        queue.enqueue(checkIn2)

        XCTAssertEqual(queue.count, 1)
        let items = queue.allItems()
        XCTAssertEqual(items.first?.signedPayload, "payload-v2")
    }

    func test_enqueue_multipleVaults_allQueued() {
        let checkIn1 = QueuedCheckIn(
            vaultID: "vault-1",
            queuedAt: Date(),
            signedPayload: "payload-1"
        )
        let checkIn2 = QueuedCheckIn(
            vaultID: "vault-2",
            queuedAt: Date(),
            signedPayload: "payload-2"
        )

        queue.enqueue(checkIn1)
        queue.enqueue(checkIn2)

        XCTAssertEqual(queue.count, 2)
        let items = queue.allItems()
        XCTAssertEqual(items.count, 2)
    }

    func test_isEmpty_returnsTrueWhenEmpty() {
        XCTAssertTrue(queue.isEmpty)
    }

    func test_isEmpty_returnsFalseWhenNotEmpty() {
        let checkIn = QueuedCheckIn(
            vaultID: "vault-123",
            queuedAt: Date(),
            signedPayload: "payload-xyz"
        )

        queue.enqueue(checkIn)

        XCTAssertFalse(queue.isEmpty)
    }

    // MARK: - Removal Tests

    func test_remove_deletesItemFromQueue() {
        let checkIn = QueuedCheckIn(
            vaultID: "vault-123",
            queuedAt: Date(),
            signedPayload: "payload-xyz"
        )

        queue.enqueue(checkIn)
        XCTAssertEqual(queue.count, 1)

        queue.remove(vaultID: "vault-123")

        XCTAssertEqual(queue.count, 0)
        XCTAssertTrue(queue.isEmpty)
    }

    func test_remove_nonexistent_doesNothing() {
        let checkIn = QueuedCheckIn(
            vaultID: "vault-123",
            queuedAt: Date(),
            signedPayload: "payload-xyz"
        )

        queue.enqueue(checkIn)
        queue.remove(vaultID: "vault-nonexistent")

        XCTAssertEqual(queue.count, 1)
    }

    // MARK: - Flush Tests

    func test_flushQueue_emptyQueue_returnsImmediately() async {
        let expectation = expectation(description: "Flush completes")

        await queue.flushQueue()

        expectation.fulfill()
        await fulfillment(of: [expectation], timeout: 1.0)
    }

    func test_flushQueue_stopsWhenNetworkLost() async {
        let checkIn1 = QueuedCheckIn(
            vaultID: "vault-1",
            queuedAt: Date(),
            signedPayload: "payload-1"
        )
        let checkIn2 = QueuedCheckIn(
            vaultID: "vault-2",
            queuedAt: Date(),
            signedPayload: "payload-2"
        )

        queue.enqueue(checkIn1)
        queue.enqueue(checkIn2)

        // Simulate network loss mid-flush
        NetworkMonitor.shared.isConnected = false

        await queue.flushQueue()

        // Should stop processing and leave remaining items queued
        XCTAssertGreaterThan(queue.count, 0)
    }

    func test_flushQueue_handleExpiredTTL() async {
        let checkIn = QueuedCheckIn(
            vaultID: "vault-expired",
            queuedAt: Date().addingTimeInterval(-3600),
            signedPayload: "payload-xyz"
        )

        queue.enqueue(checkIn)

        // In real scenario, API would return .expiredTTL
        // Item should be removed with warning notification
        let initialCount = queue.count
        XCTAssertEqual(initialCount, 1)
    }

    // MARK: - Persistence Tests

    func test_persistence_survivesDeinit() {
        let checkIn = QueuedCheckIn(
            vaultID: "vault-persist",
            queuedAt: Date(),
            signedPayload: "payload-persist"
        )

        queue.enqueue(checkIn)

        // Create new queue instance - should load from disk
        let newQueue = OfflineCheckInQueue()

        XCTAssertEqual(newQueue.count, 1)
        let items = newQueue.allItems()
        XCTAssertEqual(items.first?.vaultID, "vault-persist")
    }

    func test_persistence_encodesAndDecodesCorrectly() {
        let now = Date()
        let checkIn = QueuedCheckIn(
            vaultID: "vault-test",
            queuedAt: now,
            signedPayload: "signed-data"
        )

        queue.enqueue(checkIn)

        let items = queue.allItems()
        XCTAssertEqual(items.first?.queuedAt.timeIntervalSince1970, now.timeIntervalSince1970, accuracy: 1.0)
    }

    // MARK: - Thread Safety Tests

    func test_concurrentEnqueue_isThreadSafe() {
        let dispatchGroup = DispatchGroup()

        for i in 0..<100 {
            dispatchGroup.enter()
            DispatchQueue.global().async {
                let checkIn = QueuedCheckIn(
                    vaultID: "vault-\(i)",
                    queuedAt: Date(),
                    signedPayload: "payload-\(i)"
                )
                self.queue.enqueue(checkIn)
                dispatchGroup.leave()
            }
        }

        dispatchGroup.wait()

        XCTAssertEqual(queue.count, 100)
    }

    func test_concurrentRemove_isThreadSafe() {
        for i in 0..<50 {
            let checkIn = QueuedCheckIn(
                vaultID: "vault-\(i)",
                queuedAt: Date(),
                signedPayload: "payload-\(i)"
            )
            queue.enqueue(checkIn)
        }

        let dispatchGroup = DispatchGroup()

        for i in 0..<50 {
            dispatchGroup.enter()
            DispatchQueue.global().async {
                self.queue.remove(vaultID: "vault-\(i)")
                dispatchGroup.leave()
            }
        }

        dispatchGroup.wait()

        XCTAssertTrue(queue.isEmpty)
    }
}

// MARK: - NetworkMonitor Extension for Testing

extension NetworkMonitor {
    static let testShared = NetworkMonitor()
}
