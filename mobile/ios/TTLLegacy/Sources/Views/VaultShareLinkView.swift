import SwiftUI

/// A sheet that displays a read-only summary of a vault and lets the owner
/// share a preview link with a beneficiary or other party.
///
/// The link leads to `https://ttl-legacy.app/vaults/{vault.id}/preview` which
/// renders a public, read-only view of the vault — no actions can be performed.
struct VaultShareLinkView: View {
    let vault: Vault

    @Environment(\.dismiss) private var dismiss
    @State private var copiedToClipboard = false

    private var previewURL: URL {
        URL(string: "https://ttl-legacy.app/vaults/\(vault.id)/preview")!
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {

                    // MARK: - Vault Summary Card
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Vault Summary")
                            .font(.headline)
                            .foregroundStyle(.secondary)

                        Divider()

                        summaryRow(label: "Vault ID", value: vault.id)
                        summaryRow(label: "Owner", value: vault.owner)
                        summaryRow(label: "Beneficiary", value: vault.beneficiary)
                        summaryRow(label: "Balance", value: vault.formattedBalance)

                        HStack {
                            Text("Status")
                                .foregroundStyle(.secondary)
                            Spacer()
                            StatusBadge(status: vault.status)
                        }

                        if let ttl = vault.ttlRemaining {
                            summaryRow(label: "TTL Remaining", value: formatDuration(ttl))
                        }
                    }
                    .padding()
                    .background(Color(.systemGroupedBackground))
                    .clipShape(RoundedRectangle(cornerRadius: 12))

                    // MARK: - Share Actions
                    VStack(spacing: 12) {
                        ShareLink(
                            item: previewURL,
                            subject: Text("TTL-Legacy Vault Preview"),
                            message: Text("View my vault on TTL-Legacy: \(previewURL.absoluteString)")
                        ) {
                            Label("Share Preview Link", systemImage: "square.and.arrow.up")
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)

                        Button(action: copyLink) {
                            Label(copiedToClipboard ? "Copied!" : "Copy Link", systemImage: "doc.on.doc")
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.bordered)
                        .animation(.easeInOut(duration: 0.2), value: copiedToClipboard)
                    }

                    // MARK: - Disclaimer
                    HStack(alignment: .top, spacing: 8) {
                        Image(systemName: "info.circle.fill")
                            .foregroundStyle(.secondary)
                        Text("This is a read-only preview. No vault actions can be performed via this link.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.top, 4)
                }
                .padding()
            }
            .navigationTitle("Share Vault Preview")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    // MARK: - Helpers

    @ViewBuilder
    private func summaryRow(label: String, value: String) -> some View {
        HStack(alignment: .top) {
            Text(label)
                .foregroundStyle(.secondary)
                .frame(width: 110, alignment: .leading)
            Text(value)
                .font(.system(.body, design: .monospaced))
                .textSelection(.enabled)
                .lineLimit(2)
                .minimumScaleFactor(0.7)
        }
    }

    private func copyLink() {
        UIPasteboard.general.string = previewURL.absoluteString
        copiedToClipboard = true
        Task {
            try? await Task.sleep(nanoseconds: 2_000_000_000)
            copiedToClipboard = false
        }
    }

    private func formatDuration(_ seconds: UInt64) -> String {
        let days = seconds / 86_400
        let hours = (seconds % 86_400) / 3_600
        if days > 0 { return "\(days)d \(hours)h" }
        return "\(hours)h"
    }
}
