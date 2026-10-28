import Foundation
import CryptoKit

extension APIClient {

    // MARK: - Proof-of-Life (Issue #1842)

    /// Submits a beneficiary proof-of-life to the backend.
    ///
    /// The `imageData` is hashed on-device with SHA-256; only the 64-character
    /// lowercase hex digest travels over the network.  The raw bytes never leave
    /// the device.
    ///
    /// - Parameters:
    ///   - vaultID: Vault the beneficiary belongs to.
    ///   - beneficiary: Stellar address of the beneficiary submitting the proof.
    ///   - imageData: JPEG/PNG bytes captured from the camera.  Must be non-empty.
    ///   - validitySeconds: How long the proof should remain valid (default: 30 days).
    /// - Returns: The server's `ProofOfLifeResponse`.
    /// - Throws: `APIError.serverError` if the vault/beneficiary combination is
    ///           rejected, or any networking / decoding error.
    func submitProofOfLife(
        vaultID: String,
        beneficiary: String,
        imageData: Data,
        validitySeconds: UInt64 = 30 * 24 * 3_600
    ) async throws -> ProofOfLifeResponse {
        let digest = SHA256.hash(data: imageData)
        let proofHash = digest.map { String(format: "%02x", $0) }.joined()

        let body = ProofOfLifeRequest(
            beneficiary: beneficiary,
            proof: proofHash,
            validitySeconds: validitySeconds
        )
        return try await post(path: "/vaults/\(vaultID)/proof-of-life", body: body)
    }
}
