import Foundation
import UIKit
import CryptoKit

/// Manages the proof-of-life capture-and-submit flow for a vault beneficiary
/// (Issue #1842).
///
/// The flow is:
///   1. User opens `ProofOfLifeView` from a vault's detail screen.
///   2. They capture a selfie (or choose a photo from their library).
///   3. The view model hashes the image bytes with SHA-256 on device.
///   4. Only the hex digest is sent to `POST /api/vaults/{id}/proof-of-life`.
///   5. On success a `ProofOfLifeSummary` is surfaced to the UI and the vault
///      store is refreshed so the next check-in deadline reflects the new proof.
@MainActor
final class ProofOfLifeViewModel: ObservableObject {

    // MARK: - State

    /// The photo the user selected or captured.  `nil` while no image has been
    /// chosen yet.
    @Published var capturedImage: UIImage?

    /// Hex-encoded SHA-256 hash shown to the user as a commitment preview.
    @Published var proofHash: String?

    /// Progress / loading state while the submission is in flight.
    @Published var isSubmitting = false

    /// Non-nil when the last submission succeeded.
    @Published var submissionResult: ProofOfLifeSummary?

    /// Non-nil when an error occurred during hashing or submission.
    @Published var errorMessage: String?

    // MARK: - Private state

    private let vaultID: String
    private let beneficiaryAddress: String

    // MARK: - Init

    init(vaultID: String, beneficiaryAddress: String) {
        self.vaultID = vaultID
        self.beneficiaryAddress = beneficiaryAddress
    }

    // MARK: - Actions

    /// Called whenever the user picks or captures a new image.
    ///
    /// Synchronously hashes the JPEG representation of the image and stores
    /// the hex digest so the UI can show a preview before the user submits.
    func setImage(_ image: UIImage) {
        capturedImage = image
        submissionResult = nil
        errorMessage = nil

        // Hash the JPEG bytes on-device — raw pixels never leave the device.
        guard let jpegData = image.jpegData(compressionQuality: 0.8) else {
            errorMessage = "Could not process the selected image. Please try again."
            proofHash = nil
            return
        }
        let digest = SHA256.hash(data: jpegData)
        proofHash = digest.map { String(format: "%02x", $0) }.joined()
    }

    /// Resets the view model back to the initial capture state.
    func reset() {
        capturedImage = nil
        proofHash = nil
        submissionResult = nil
        errorMessage = nil
        isSubmitting = false
    }

    /// Submits the hashed proof to the backend.
    ///
    /// Requires `capturedImage` to be set; does nothing and surfaces an error
    /// message if called without a captured image.
    func submit(validityDays: UInt64 = 30) async {
        guard let image = capturedImage else {
            errorMessage = "Please capture or select an image before submitting."
            return
        }
        guard let jpegData = image.jpegData(compressionQuality: 0.8) else {
            errorMessage = "Could not process the image. Please try a different photo."
            return
        }

        isSubmitting = true
        errorMessage = nil

        do {
            let validitySeconds = validityDays * 24 * 3_600
            let response = try await APIClient.shared.submitProofOfLife(
                vaultID: vaultID,
                beneficiary: beneficiaryAddress,
                imageData: jpegData,
                validitySeconds: validitySeconds
            )

            let now = Date()
            let validUntil = now.addingTimeInterval(TimeInterval(validitySeconds))
            submissionResult = ProofOfLifeSummary(
                submittedAt: now,
                validUntil: validUntil,
                proofHash: proofHash ?? ""
            )
        } catch APIError.unauthorized {
            errorMessage = "You are not authorised to submit proof of life for this vault."
        } catch APIError.notFound {
            errorMessage = "Vault not found or you are not listed as a beneficiary."
        } catch APIError.networkUnavailable {
            errorMessage = "No internet connection. Please try again when online."
        } catch {
            errorMessage = error.localizedDescription
        }

        isSubmitting = false
    }
}
