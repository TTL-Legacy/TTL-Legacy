import SwiftUI

struct SettingsView: View {
    @State private var iCloudSyncEnabled = ICloudSyncService.shared.isSyncEnabled

    var body: some View {
        Form {
            Section {
                Toggle("Sync vault associations to iCloud", isOn: $iCloudSyncEnabled)
                    .onChange(of: iCloudSyncEnabled) { _, newValue in
                        ICloudSyncService.shared.isSyncEnabled = newValue
                    }
                Text("Syncs which vaults are linked to your passkeys across your devices. Your passkey private keys are never uploaded.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } header: {
                Text("iCloud Backup")
            }
        }
        .navigationTitle("Settings")
    }
}
