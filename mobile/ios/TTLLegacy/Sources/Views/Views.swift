import SwiftUI

struct RootView: View {
    @EnvironmentObject var authStore: AuthStore

    var body: some View {
        if authStore.isAuthenticated {
            VaultListView()
        } else {
            AuthView()
        }
    }
}

// MARK: - Auth

struct AuthView: View {
    @EnvironmentObject var authStore: AuthStore
    @State private var username = ""
    @State private var showRegister = false

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                Image(systemName: "lock.shield.fill")
                    .font(.system(size: 64))
                    .foregroundStyle(.blue)
                Text("TTL-Legacy").font(.largeTitle.bold())
                Text("Secure digital inheritance").foregroundStyle(.secondary)

                if let error = authStore.error {
                    Text(error).foregroundStyle(.red).font(.caption).multilineTextAlignment(.center)
                }

                Button(action: { Task { await authStore.signIn() } }) {
                    Label("Sign in with Passkey", systemImage: "person.badge.key.fill")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .disabled(authStore.isLoading)

                Button("Create account") { showRegister = true }
                    .foregroundStyle(.blue)
            }
            .padding(32)
            .overlay { if authStore.isLoading { ProgressView() } }
            .sheet(isPresented: $showRegister) { RegisterView() }
        }
    }
}

struct RegisterView: View {
    @EnvironmentObject var authStore: AuthStore
    @Environment(\.dismiss) var dismiss
    @State private var username = ""

    var body: some View {
        NavigationStack {
            Form {
                Section("Account") {
                    TextField("Username", text: $username)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }
                if let error = authStore.error {
                    Section { Text(error).foregroundStyle(.red).font(.caption) }
                }
            }
            .navigationTitle("Create Account")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Register") {
                        Task { await authStore.register(username: username); dismiss() }
                    }
                    .disabled(username.isEmpty || authStore.isLoading)
                }
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }
}

// MARK: - Vault List

struct VaultListView: View {
    @EnvironmentObject var vaultStore: VaultStore
    @EnvironmentObject var authStore: AuthStore
    @State private var showCreate = false
    @State private var showDeepLinkSheet = false

    var body: some View {
        NavigationStack {
            Group {
                if vaultStore.isLoading && vaultStore.vaults.isEmpty {
                    ProgressView("Loading vaults…")
                } else if vaultStore.vaults.isEmpty {
                    ContentUnavailableView("No Vaults", systemImage: "lock.open", description: Text("Create your first vault to get started."))
                } else {
                    List(vaultStore.vaults) { vault in
                        NavigationLink(destination: VaultDetailView(vault: vault)) {
                            VaultRowView(vault: vault)
                        }
                    }
                    .refreshable { await vaultStore.load() }
                }
            }
            .navigationTitle("My Vaults")
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(action: { showCreate = true }) { Image(systemName: "plus") }
                }
                ToolbarItem(placement: .secondaryAction) {
                    Button("Sign Out") { authStore.signOut() }
                }
            }
            .task { await vaultStore.load() }
            .sheet(isPresented: $showCreate) { CreateVaultView() }
            .sheet(isPresented: $showDeepLinkSheet, onDismiss: { vaultStore.pendingDeepLink = nil }) {
                if let link = vaultStore.pendingDeepLink {
                    DeepLinkView(link: link)
                }
            }
            .onChange(of: vaultStore.pendingDeepLink) { _, link in
                if link != nil { showDeepLinkSheet = true }
            }
        }
    }
}

struct VaultRowView: View {
    let vault: Vault

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(vault.id.prefix(12) + "…").font(.headline)
                Spacer()
                StatusBadge(status: vault.status)
            }
            Text(vault.formattedBalance).font(.subheadline).foregroundStyle(.secondary)
            if vault.isExpiringSoon {
                Label("Expiring soon!", systemImage: "exclamationmark.triangle.fill")
                    .font(.caption).foregroundStyle(.orange)
            }
        }
        .padding(.vertical, 4)
    }
}

struct StatusBadge: View {
    let status: Vault.VaultStatus
    var body: some View {
        Text(status.rawValue.capitalized)
            .font(.caption.bold())
            .padding(.horizontal, 8).padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }
    private var color: Color {
        switch status {
        case .active:   return .green
        case .expired:  return .orange
        case .released: return .blue
        case .paused:   return .gray
        }
    }
}

// MARK: - Vault Detail

struct VaultDetailView: View {
    let vault: Vault
    @EnvironmentObject var vaultStore: VaultStore
    @State private var isCheckingIn = false
    @State private var biometricError: String?

    var body: some View {
        List {
            Section("Overview") {
                LabeledContent("Balance", value: vault.formattedBalance)
                LabeledContent("Status", value: vault.status.rawValue.capitalized)
                LabeledContent("Beneficiary", value: vault.beneficiary.prefix(16) + "…")
                if let ttl = vault.ttlRemaining {
                    LabeledContent("TTL Remaining", value: formatDuration(ttl))
                }
            }
            Section {
                Button(action: checkIn) {
                    Label(isCheckingIn ? "Checking in…" : "Check In Now", systemImage: "checkmark.circle.fill")
                }
                .disabled(isCheckingIn || vault.status != .active)
                if let error = biometricError {
                    Text(error).foregroundStyle(.red).font(.caption)
                }
            }
        }
        .navigationTitle("Vault")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func checkIn() {
        biometricError = nil
        isCheckingIn = true
        Task {
            do {
                try await BiometricService.shared.authenticate(reason: "Confirm vault check-in")
                await vaultStore.checkIn(vault: vault)
            } catch {
                biometricError = error.localizedDescription
            }
            isCheckingIn = false
        }
    }

    private func formatDuration(_ seconds: UInt64) -> String {
        let days = seconds / 86_400
        let hours = (seconds % 86_400) / 3_600
        if days > 0 { return "\(days)d \(hours)h" }
        return "\(hours)h"
    }
}

// MARK: - Create Vault

struct CreateVaultView: View {
    @EnvironmentObject var vaultStore: VaultStore
    @Environment(\.dismiss) var dismiss
    @State private var beneficiary = ""
    @State private var intervalDays = 30.0
    @State private var isCreating = false
    @State private var error: String?

    var body: some View {
        NavigationStack {
            Form {
                Section("Beneficiary") {
                    TextField("Stellar address", text: $beneficiary)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .font(.system(.body, design: .monospaced))
                }
                Section("Check-in Interval") {
                    Slider(value: $intervalDays, in: 1...365, step: 1)
                    Text("\(Int(intervalDays)) days").foregroundStyle(.secondary)
                }
                if let error { Section { Text(error).foregroundStyle(.red).font(.caption) } }
            }
            .navigationTitle("New Vault")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") { create() }.disabled(beneficiary.isEmpty || isCreating)
                }
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }

    private func create() {
        isCreating = true
        Task {
            do {
                let interval = UInt64(intervalDays * 86_400)
                _ = try await APIClient.shared.createVault(beneficiary: beneficiary, checkInInterval: interval)
                await vaultStore.load()
                dismiss()
            } catch { self.error = error.localizedDescription }
            isCreating = false
        }
    }
}

// MARK: - Deep Link Views

struct DeepLinkView: View {
    let link: UniversalLinkRouter.DeepLink
    @Environment(\.dismiss) var dismiss

    var body: some View {
        NavigationStack {
            switch link {
            case .vaultInvitation(let vaultID):
                VaultInvitationView(vaultID: vaultID)
            case .beneficiaryAcceptance(let vaultID, let token):
                BeneficiaryAcceptanceView(vaultID: vaultID, token: token)
            }
        }
    }
}

struct VaultInvitationView: View {
    let vaultID: String
    @Environment(\.dismiss) var dismiss

    var body: some View {
        VStack(spacing: 24) {
            Image(systemName: "envelope.open.fill").font(.system(size: 56)).foregroundStyle(.blue)
            Text("Vault Invitation").font(.title.bold())
            Text("You have been invited to a vault.\nVault ID: \(vaultID.prefix(16))…")
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
            Button("Open App") { dismiss() }
                .buttonStyle(.borderedProminent)
        }
        .padding(32)
        .navigationTitle("Invitation")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Dismiss") { dismiss() } } }
    }
}

struct BeneficiaryAcceptanceView: View {
    let vaultID: String
    let token: String
    @Environment(\.dismiss) var dismiss
    @State private var isAccepting = false
    @State private var error: String?
    @State private var accepted = false

    var body: some View {
        VStack(spacing: 24) {
            Image(systemName: "checkmark.seal.fill").font(.system(size: 56)).foregroundStyle(.green)
            Text("Accept Beneficiary Role").font(.title.bold())
            Text("You have been nominated as a beneficiary for vault \(vaultID.prefix(16))…")
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
            if accepted {
                Label("Accepted", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
            } else {
                if let error { Text(error).foregroundStyle(.red).font(.caption) }
                Button(action: accept) {
                    Label(isAccepting ? "Accepting…" : "Accept", systemImage: "hand.thumbsup.fill")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .disabled(isAccepting)
            }
        }
        .padding(32)
        .navigationTitle("Beneficiary Acceptance")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Dismiss") { dismiss() } } }
    }

    private func accept() {
        isAccepting = true
        Task {
            do {
                try await APIClient.shared.acceptBeneficiary(vaultID: vaultID, token: token)
                accepted = true
            } catch {
                self.error = error.localizedDescription
            }
            isAccepting = false
        }
    }
}
