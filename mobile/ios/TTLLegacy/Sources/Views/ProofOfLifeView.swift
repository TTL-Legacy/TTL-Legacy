import SwiftUI
import PhotosUI
import UIKit

/// Camera-based proof-of-life submission screen (Issue #1842).
///
/// The user takes a selfie (or chooses an existing photo), reviews a truncated
/// SHA-256 commitment hash, and submits it to the backend.  Only the hash is
/// sent — raw image data never leaves the device.
///
/// Usage
/// -----
/// ```swift
/// .sheet(isPresented: $showProofOfLife) {
///     ProofOfLifeView(vaultID: vault.id, beneficiaryAddress: currentUserAddress)
/// }
/// ```
struct ProofOfLifeView: View {

    // MARK: - Dependencies

    let vaultID: String
    let beneficiaryAddress: String

    // MARK: - State

    @StateObject private var viewModel: ProofOfLifeViewModel
    @Environment(\.dismiss) private var dismiss

    /// Controls the native camera sheet.
    @State private var showCamera = false
    /// Controls the photo library picker.
    @State private var showLibrary = false
    /// Binds to `PhotosPickerItem` from the new `PhotosPicker` API (iOS 16+).
    @State private var photosPickerItem: PhotosPickerItem?
    /// Validity window the user has selected (in days).
    @State private var validityDays: Double = 30

    // MARK: - Init

    init(vaultID: String, beneficiaryAddress: String) {
        self.vaultID = vaultID
        self.beneficiaryAddress = beneficiaryAddress
        _viewModel = StateObject(
            wrappedValue: ProofOfLifeViewModel(
                vaultID: vaultID,
                beneficiaryAddress: beneficiaryAddress
            )
        )
    }

    // MARK: - Body

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 24) {
                    // ── Header ────────────────────────────────────────────────
                    headerSection

                    // ── Camera / image capture ────────────────────────────────
                    captureSection

                    // ── Hash preview ──────────────────────────────────────────
                    if let hash = viewModel.proofHash {
                        hashPreviewSection(hash: hash)
                    }

                    // ── Validity picker ───────────────────────────────────────
                    if viewModel.capturedImage != nil && viewModel.submissionResult == nil {
                        validitySection
                    }

                    // ── Error ─────────────────────────────────────────────────
                    if let error = viewModel.errorMessage {
                        errorSection(message: error)
                    }

                    // ── Success card ──────────────────────────────────────────
                    if let result = viewModel.submissionResult {
                        successSection(result: result)
                    }

                    // ── Submit / Done button ──────────────────────────────────
                    actionButton
                }
                .padding(20)
            }
            .navigationTitle("Proof of Life")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                if viewModel.submissionResult != nil {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") { dismiss() }
                    }
                }
            }
            // Camera sheet (UIImagePickerController wrapper)
            .sheet(isPresented: $showCamera) {
                CameraPickerView { image in
                    viewModel.setImage(image)
                    showCamera = false
                }
                .ignoresSafeArea()
            }
            // Photo library sheet (new PhotosPicker API, iOS 16+)
            .photosPicker(
                isPresented: $showLibrary,
                selection: $photosPickerItem,
                matching: .images,
                photoLibrary: .shared()
            )
            .onChange(of: photosPickerItem) { _, item in
                Task { await loadPickedPhoto(item) }
            }
        }
    }

    // MARK: - Sub-views

    private var headerSection: some View {
        VStack(spacing: 12) {
            Image(systemName: "camera.fill.badge.ellipsis")
                .font(.system(size: 52))
                .foregroundStyle(.blue)
                .accessibilityHidden(true)

            Text("Confirm you're here")
                .font(.title2.bold())

            Text(
                "Take a selfie or choose a photo. Only a cryptographic hash of your image is sent to the server — your photo stays on your device."
            )
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .multilineTextAlignment(.center)
        }
    }

    private var captureSection: some View {
        VStack(spacing: 16) {
            if let image = viewModel.capturedImage {
                // Preview the captured image
                Image(uiImage: image)
                    .resizable()
                    .scaledToFill()
                    .frame(height: 220)
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .overlay(
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .stroke(Color.secondary.opacity(0.3), lineWidth: 1)
                    )
                    .accessibilityLabel("Captured image preview")

                Button(role: .destructive) {
                    viewModel.reset()
                } label: {
                    Label("Retake / Choose different photo", systemImage: "arrow.counterclockwise")
                        .font(.subheadline)
                }
            } else {
                // Placeholder with capture options
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(Color(.secondarySystemBackground))
                    .frame(height: 220)
                    .overlay {
                        VStack(spacing: 8) {
                            Image(systemName: "person.crop.rectangle.badge.plus")
                                .font(.system(size: 44))
                                .foregroundStyle(.tertiary)
                            Text("No image selected")
                                .foregroundStyle(.secondary)
                                .font(.subheadline)
                        }
                    }
                    .accessibilityLabel("No image selected")

                // Capture action buttons
                HStack(spacing: 12) {
                    Button {
                        if UIImagePickerController.isSourceTypeAvailable(.camera) {
                            showCamera = true
                        } else {
                            viewModel.errorMessage = "Camera is not available on this device."
                        }
                    } label: {
                        Label("Take Selfie", systemImage: "camera.fill")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .accessibilityLabel("Open camera to take a selfie")

                    Button { showLibrary = true } label: {
                        Label("Choose Photo", systemImage: "photo.on.rectangle.angled")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .accessibilityLabel("Choose an existing photo from your library")
                }
            }
        }
    }

    @ViewBuilder
    private func hashPreviewSection(hash: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Proof Hash (SHA-256)", systemImage: "key.horizontal.fill")
                .font(.footnote.bold())
                .foregroundStyle(.secondary)

            Text(hash.prefix(32) + "…")
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.primary)
                .lineLimit(1)
                .truncationMode(.tail)
                .padding(10)
                .background(Color(.secondarySystemBackground))
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                .accessibilityLabel("Proof hash: \(hash.prefix(8))…")
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var validitySection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Valid for \(Int(validityDays)) days", systemImage: "calendar.badge.checkmark")
                .font(.subheadline.bold())

            Slider(value: $validityDays, in: 1...90, step: 1)
                .tint(.blue)
                .accessibilityLabel("Proof validity duration")
                .accessibilityValue("\(Int(validityDays)) days")

            Text("The proof expires after this period and may need to be resubmitted.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private func errorSection(message: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.orange)
            Text(message)
                .font(.subheadline)
                .foregroundStyle(.primary)
                .multilineTextAlignment(.leading)
        }
        .padding(12)
        .background(Color.orange.opacity(0.1))
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Error: \(message)")
    }

    @ViewBuilder
    private func successSection(result: ProofOfLifeSummary) -> some View {
        VStack(spacing: 12) {
            Image(systemName: "checkmark.seal.fill")
                .font(.system(size: 48))
                .foregroundStyle(.green)
                .accessibilityHidden(true)

            Text("Proof submitted!")
                .font(.title3.bold())

            Group {
                LabeledContent("Status") {
                    Text(result.isValid ? "Valid" : "Expired")
                        .foregroundStyle(result.isValid ? .green : .red)
                        .fontWeight(.semibold)
                }
                LabeledContent("Valid until", value: result.formattedExpiry)
                LabeledContent("Hash") {
                    Text(result.proofHash.prefix(16) + "…")
                        .font(.system(.caption, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.horizontal, 4)
        }
        .padding(16)
        .background(Color.green.opacity(0.08))
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(Color.green.opacity(0.25), lineWidth: 1)
        )
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Proof of life submitted successfully")
    }

    @ViewBuilder
    private var actionButton: some View {
        if viewModel.submissionResult != nil {
            // Submission succeeded — show a "Done" shortcut in the body too
            Button { dismiss() } label: {
                Label("Done", systemImage: "checkmark.circle.fill")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(.green)
        } else if viewModel.capturedImage != nil {
            Button {
                Task { await viewModel.submit(validityDays: UInt64(validityDays)) }
            } label: {
                if viewModel.isSubmitting {
                    HStack(spacing: 8) {
                        ProgressView()
                            .progressViewStyle(.circular)
                            .tint(.white)
                        Text("Submitting…")
                    }
                    .frame(maxWidth: .infinity)
                } else {
                    Label("Submit Proof of Life", systemImage: "paperplane.fill")
                        .frame(maxWidth: .infinity)
                }
            }
            .buttonStyle(.borderedProminent)
            .disabled(viewModel.isSubmitting)
            .accessibilityLabel(
                viewModel.isSubmitting ? "Submitting proof, please wait" : "Submit proof of life"
            )
        }
    }

    // MARK: - Helpers

    /// Loads the `PhotosPickerItem` and passes the decoded `UIImage` to the
    /// view model.
    private func loadPickedPhoto(_ item: PhotosPickerItem?) async {
        guard let item else { return }
        do {
            if let data = try await item.loadTransferable(type: Data.self),
               let image = UIImage(data: data) {
                viewModel.setImage(image)
            } else {
                viewModel.errorMessage = "Could not load the selected photo."
            }
        } catch {
            viewModel.errorMessage = "Failed to load photo: \(error.localizedDescription)"
        }
        photosPickerItem = nil
    }
}

// MARK: - CameraPickerView (UIViewControllerRepresentable)

/// Thin SwiftUI wrapper around `UIImagePickerController` in camera mode.
///
/// `PhotosPicker` (iOS 16+) handles the photo library. This wrapper handles
/// the camera because `PhotosPicker` does not expose the front-facing camera.
private struct CameraPickerView: UIViewControllerRepresentable {

    let onImageCaptured: (UIImage) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(onImageCaptured: onImageCaptured) }

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let picker = UIImagePickerController()
        picker.sourceType = .camera
        // Front camera preferred for selfie-based proof of life
        picker.cameraDevice = UIImagePickerController.isCameraDeviceAvailable(.front)
            ? .front
            : .rear
        picker.allowsEditing = false
        picker.delegate = context.coordinator
        return picker
    }

    func updateUIViewController(_ uiViewController: UIImagePickerController, context: Context) {}

    final class Coordinator: NSObject,
                              UIImagePickerControllerDelegate,
                              UINavigationControllerDelegate {

        let onImageCaptured: (UIImage) -> Void

        init(onImageCaptured: @escaping (UIImage) -> Void) {
            self.onImageCaptured = onImageCaptured
        }

        func imagePickerController(
            _ picker: UIImagePickerController,
            didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]
        ) {
            if let image = info[.originalImage] as? UIImage {
                onImageCaptured(image)
            }
        }

        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
            picker.dismiss(animated: true)
        }
    }
}

// MARK: - VaultDetailView integration

/// Convenience extension that adds the "Proof of Life" button and sheet to the
/// existing `VaultDetailView` — mirrors the 2FA integration pattern already
/// present in the file.
extension VaultDetailView {

    /// Returns `true` when the vault is active and the current user could be
    /// its beneficiary (heuristic based on vault status; a production version
    /// would compare against the authenticated user's Stellar address).
    var canSubmitProofOfLife: Bool {
        vault.status == .active
    }

    /// Builds the "Proof of Life" list section that is injected into the vault
    /// detail body.  Extract it with a ViewBuilder so it is decoupled from the
    /// main `body` implementation.
    @ViewBuilder
    func proofOfLifeSection(
        showingSheet: Binding<Bool>
    ) -> some View {
        Section("Proof of Life") {
            if canSubmitProofOfLife {
                Button(action: { showingSheet.wrappedValue = true }) {
                    Label("Submit Proof of Life", systemImage: "camera.fill.badge.ellipsis")
                }
            } else {
                Text("Proof of life is only available for active vaults.")
                    .foregroundStyle(.secondary)
                    .font(.subheadline)
            }
        }
    }
}
