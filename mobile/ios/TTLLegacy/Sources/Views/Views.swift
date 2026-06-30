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
    @State private var show2FASetup = false
    @State private var show2FAVerify = false
    @State private var twoFactorStatus: TwoFactorStatus?

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

            Section("Two-Factor Authentication") {
                if let status = twoFactorStatus {
                    if status.enabled {
                        LabeledContent("2FA", value: status.method.map { "\($0.rawValue.uppercased())" } ?? "Enabled")
                        LabeledContent("Verified", value: status.verified ? "Yes" : "No")
                        if !status.verified {
                            Button("Verify Now") { show2FAVerify = true }
                        }
                        Button("Disable 2FA", role: .destructive) { disable2FA() }
                    } else {
                        Button("Enable 2FA") { show2FASetup = true }
                    }
                } else {
                    ProgressView()
                        .task { await load2FAStatus() }
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
        .sheet(isPresented: $show2FASetup) {
            TwoFactorSetupView(vaultID: vault.id)
        }
        .sheet(isPresented: $show2FAVerify) {
            TwoFactorVerifyView(
                vaultID: vault.id,
                method: twoFactorStatus?.method ?? .totp,
                provisioningUri: nil,
                secret: nil,
                onVerified: { Task { await load2FAStatus() } }
            )
        }
    }

    private func load2FAStatus() async {
        do {
            twoFactorStatus = try await APIClient.shared.get2FAStatus(vaultID: vault.id)
        } catch {
            twoFactorStatus = nil
        }
    }

    private func disable2FA() {
        Task {
            do {
                try await APIClient.shared.disable2FA(vaultID: vault.id)
                await load2FAStatus()
            } catch {
                biometricError = error.localizedDescription
            }
        }
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

// MARK: - 2FA Views

struct TwoFactorSetupView: View {
    let vaultID: String
    @Environment(\.dismiss) var dismiss
    @State private var selectedMethod: TwoFactorMethod = .totp
    @State private var phone = ""
    @State private var email = ""
    @State private var setupResponse: Enable2FAResponse?
    @State private var showVerify = false
    @State private var isSettingUp = false
    @State private var error: String?
    @State private var setupComplete = false

    var body: some View {
        NavigationStack {
            if let response = setupResponse {
                TwoFactorVerifyView(
                    vaultID: vaultID,
                    method: response.method,
                    provisioningUri: response.provisioningUri,
                    secret: response.secret,
                    onVerified: { setupComplete = true }
                )
            } else {
                Form {
                    Section("Authentication Method") {
                        Picker("Method", selection: $selectedMethod) {
                            ForEach(TwoFactorMethod.allCases, id: \.self) { method in
                                Text(methodLabel(method)).tag(method)
                            }
                        }
                    }

                    if selectedMethod == .sms {
                        Section("SMS Number") {
                            TextField("Phone number", text: $phone)
                                .keyboardType(.phonePad)
                        }
                    }

                    if selectedMethod == .email {
                        Section("Email Address") {
                            TextField("Email", text: $email)
                                .keyboardType(.emailAddress)
                                .autocapitalization(.none)
                        }
                    }

                    if let error { Section { Text(error).foregroundStyle(.red).font(.caption) } }
                }
                .navigationTitle("Enable 2FA")
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Continue") { setup() }
                            .disabled(isSettingUp || !canContinue)
                    }
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { dismiss() }
                    }
                }
                .overlay { if isSettingUp { ProgressView() } }
            }
        }
        .interactiveDismissDisabled(setupComplete == false)
    }

    private var canContinue: Bool {
        switch selectedMethod {
        case .totp: return true
        case .sms:  return !phone.trimmingCharacters(in: .whitespaces).isEmpty
        case .email: return !email.trimmingCharacters(in: .whitespaces).isEmpty
        }
    }

    private func methodLabel(_ method: TwoFactorMethod) -> String {
        switch method {
        case .totp:  return "Authenticator App (TOTP)"
        case .sms:   return "SMS Code"
        case .email: return "Email Code"
        }
    }

    private func setup() {
        isSettingUp = true; error = nil
        Task {
            do {
                let response = try await APIClient.shared.enable2FA(
                    vaultID: vaultID,
                    method: selectedMethod,
                    phone: selectedMethod == .sms ? phone : nil,
                    email: selectedMethod == .email ? email : nil
                )
                setupResponse = response
            } catch {
                self.error = error.localizedDescription
            }
            isSettingUp = false
        }
    }
}

struct TwoFactorVerifyView: View {
    let vaultID: String
    let method: TwoFactorMethod
    let provisioningUri: String?
    let secret: String?
    let onVerified: () -> Void
    @Environment(\.dismiss) var dismiss

    @State private var otp = ""
    @State private var isVerifying = false
    @State private var error: String?

    var body: some View {
        VStack(spacing: 24) {
            Image(systemName: iconName)
                .font(.system(size: 56))
                .foregroundStyle(.blue)

            Text("Verify Setup").font(.title.bold())

            if method == .totp, let uri = provisioningUri {
                VStack(spacing: 8) {
                    Text("Scan this URI in your authenticator app:").foregroundStyle(.secondary)
                    Text(uri).font(.caption).foregroundStyle(.secondary).lineLimit(3)
                    if let secret {
                        Label(secret, systemImage: "key.fill").font(.system(.caption, design: .monospaced))
                    }
                }
            } else {
                Text("A verification code has been sent to your \(methodLabel).").foregroundStyle(.secondary)
            }

            TextField("Enter 6-digit code", text: $otp)
                .textFieldStyle(.roundedBorder)
                .keyboardType(.numberPad)
                .frame(maxWidth: 200)
                .multilineTextAlignment(.center)
                .font(.title2)

            if let error { Text(error).foregroundStyle(.red).font(.caption) }

            Button(action: verify) {
                Label(isVerifying ? "Verifying…" : "Verify", systemImage: "checkmark.circle.fill")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .disabled(otp.count != 6 || isVerifying)
        }
        .padding(32)
        .navigationTitle("Verify 2FA")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
        }
    }

    private var iconName: String {
        switch method {
        case .totp:  return "lock.shield.fill"
        case .sms:   return "message.fill"
        case .email: return "envelope.fill"
        }
    }

    private var methodLabel: String {
        switch method {
        case .totp:  return "authenticator app"
        case .sms:   return "phone"
        case .email: return "email"
        }
    }

    private func verify() {
        isVerifying = true; error = nil
        Task {
            do {
                try await APIClient.shared.verify2FA(vaultID: vaultID, otp: otp)
                onVerified()
                dismiss()
            } catch {
                self.error = error.localizedDescription
            }
            isVerifying = false
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
            case .vaultAction(let vaultID, let action):
                VaultActionDeepLinkView(vaultID: vaultID, action: action)
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

struct VaultActionDeepLinkView: View {
    let vaultID: String
    let action: UniversalLinkRouter.VaultAction
    @EnvironmentObject var vaultStore: VaultStore
    @Environment(\.dismiss) var dismiss
    @State private var isProcessing = false
    @State private var error: String?

    private var vault: Vault? { vaultStore.vaults.first { $0.id == vaultID } }

    var body: some View {
        Group {
            switch action {
            case .viewDetails:
                if let vault {
                    VaultDetailView(vault: vault)
                } else {
                    vaultNotFoundContent
                }
            case .checkIn:
                actionContent(
                    title: "Check In",
                    systemImage: "checkmark.circle.fill",
                    description: "Confirm check-in for vault \(vaultID.prefix(16))…"
                ) {
                    guard let vault else { error = "Vault not found"; return }
                    isProcessing = true
                    error = nil
                    Task {
                        do {
                            try await BiometricService.shared.authenticate(reason: "Confirm vault check-in")
                            await vaultStore.checkIn(vault: vault)
                            dismiss()
                        } catch let checkInError {
                            self.error = checkInError.localizedDescription
                        }
                        isProcessing = false
                    }
                }
            case .withdraw:
                actionContent(
                    title: "Withdraw",
                    systemImage: "arrow.up.circle.fill",
                    description: "Withdraw funds from vault \(vaultID.prefix(16))…"
                ) {
                    error = "Withdrawal is not yet available in the mobile app."
                }
            case .manageBeneficiary:
                actionContent(
                    title: "Manage Beneficiary",
                    systemImage: "person.2.fill",
                    description: "Update the beneficiary for vault \(vaultID.prefix(16))…"
                ) {
                    error = "Beneficiary management is not yet available in the mobile app."
                }
            }
        }
        .task { if vaultStore.vaults.isEmpty { await vaultStore.load() } }
    }

    private var vaultNotFoundContent: some View {
        ContentUnavailableView(
            "Vault Not Found",
            systemImage: "lock.slash",
            description: Text("Vault \(vaultID.prefix(16))… could not be loaded.")
        )
        .navigationTitle("Vault")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Dismiss") { dismiss() } } }
    }

    private func actionContent(
        title: String,
        systemImage: String,
        description: String,
        onAction: @escaping () -> Void
    ) -> some View {
        VStack(spacing: 24) {
            Image(systemName: systemImage).font(.system(size: 56)).foregroundStyle(.blue)
            Text(title).font(.title.bold())
            Text(description).multilineTextAlignment(.center).foregroundStyle(.secondary)
            if let error { Text(error).foregroundStyle(.red).font(.caption) }
            Button(action: onAction) {
                Text(isProcessing ? "Processing…" : title).frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .disabled(isProcessing || (action == .checkIn && vault == nil))
        }
        .padding(32)
        .navigationTitle(title)
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
