import WidgetKit
import SwiftUI

// MARK: - Timeline Entry

struct VaultEntry: TimelineEntry {
    let date: Date
    let vaultName: String
    let ttlRemaining: UInt64?
    let isExpiringSoon: Bool
}

// MARK: - Timeline Provider

struct TTLTimelineProvider: TimelineProvider {
    func placeholder(in context: Context) -> VaultEntry {
        VaultEntry(date: .now, vaultName: "My Vault", ttlRemaining: 86_400, isExpiringSoon: false)
    }

    func getSnapshot(in context: Context, completion: @escaping (VaultEntry) -> Void) {
        completion(VaultEntry(date: .now, vaultName: "My Vault", ttlRemaining: 86_400, isExpiringSoon: false))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<VaultEntry>) -> Void) {
        Task {
            let entry: VaultEntry
            do {
                let vaults = try await APIClient.shared.listVaults()
                // Show the vault with the lowest TTL remaining (most urgent)
                let critical = vaults
                    .filter { $0.status == .active }
                    .min(by: { ($0.ttlRemaining ?? UInt64.max) < ($1.ttlRemaining ?? UInt64.max) })
                entry = VaultEntry(
                    date: .now,
                    vaultName: critical.map { String($0.id.prefix(12)) + "…" } ?? "No Active Vault",
                    ttlRemaining: critical?.ttlRemaining,
                    isExpiringSoon: critical?.isExpiringSoon ?? false
                )
            } catch {
                entry = VaultEntry(date: .now, vaultName: "Unavailable", ttlRemaining: nil, isExpiringSoon: false)
            }

            let nextUpdate = Calendar.current.date(byAdding: .minute, value: 15, to: .now)!
            completion(Timeline(entries: [entry], policy: .after(nextUpdate)))
        }
    }
}

// MARK: - Widget View

struct TTLWidgetView: View {
    let entry: VaultEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Label("TTL-Legacy", systemImage: "lock.shield.fill")
                .font(.caption2.bold())
                .foregroundStyle(.blue)
            Text(entry.vaultName)
                .font(.headline)
                .lineLimit(1)
            if let ttl = entry.ttlRemaining {
                Text(formatDuration(ttl))
                    .font(.subheadline)
                    .foregroundStyle(entry.isExpiringSoon ? .orange : .secondary)
            } else {
                Text("—").font(.subheadline).foregroundStyle(.secondary)
            }
            if entry.isExpiringSoon {
                Label("Expiring soon", systemImage: "exclamationmark.triangle.fill")
                    .font(.caption2)
                    .foregroundStyle(.orange)
            }
        }
        .padding()
        .containerBackground(.regularMaterial, for: .widget)
    }

    private func formatDuration(_ seconds: UInt64) -> String {
        let days = seconds / 86_400
        let hours = (seconds % 86_400) / 3_600
        if days > 0 { return "\(days)d \(hours)h remaining" }
        return "\(hours)h remaining"
    }
}

// MARK: - Widget Definition

struct TTLWidget: Widget {
    let kind = "TTLWidget"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: TTLTimelineProvider()) { entry in
            TTLWidgetView(entry: entry)
        }
        .configurationDisplayName("TTL Vault Status")
        .description("Shows your most urgent vault's TTL countdown.")
        .supportedFamilies([.systemSmall, .systemMedium, .accessoryRectangular, .accessoryCircular])
    }
}

// MARK: - Widget Bundle Entry Point (app extension @main)

@main
struct TTLWidgetBundle: WidgetBundle {
    var body: some Widget {
        TTLWidget()
    }
}
