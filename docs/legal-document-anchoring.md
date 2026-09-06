# Legal Document Anchoring

> **Issue #1339** — Integration Enhancement

TTL-Legacy allows vault owners to anchor signed legal documents (wills, trust deeds, powers of attorney) to their vault by storing the document's SHA-256 hash on Stellar via the vault's Soroban contract storage.

---

## Overview

Anchoring provides an **immutable, on-chain timestamp proof** that a document with a specific cryptographic hash existed at a known ledger sequence.

The on-chain anchor record stores:

| Field | Description |
|---|---|
| `doc_id` | Sequential document ID within the vault (1-indexed) |
| `doc_hash` | SHA-256 hash of the signed document (32 bytes / 64 hex chars) |
| `doc_type` | Document category (`Will`, `TrustDeed`, `PowerOfAttorney`, `Other`) |
| `storage_ref` | Optional IPFS CID or external storage reference (max 128 chars) |
| `anchored_at` | Ledger timestamp when the anchor was recorded |
| `removed` | Whether the owner has soft-removed the anchor |

Raw document bytes are **never** submitted to the contract or stored on-chain.

---

## How It Works

```
Vault owner selects document
        │
        ▼
SHA-256 hash computed client-side (WebCrypto API)
        │
        ▼
anchor_document(vault_id, caller, doc_hash, doc_type, storage_ref)
        │
        ▼
LegalDocumentAnchor stored at StorageKey::LegalDocumentAnchor(vault_id, doc_id)
DOC_ANCHORED_TOPIC event emitted on-chain
        │
        ▼
Backend records off-chain mirror in legal_document_anchors table
```

### Verification

Any party can verify a document against an anchor:

1. Obtain the original document.
2. Compute its SHA-256 hash.
3. Call `get_document_anchor(vault_id, doc_id)` on the contract.
4. Compare the computed hash against `anchor.doc_hash`.
5. The `anchored_at` ledger timestamp provides the proof-of-existence time.

---

## Smart Contract API

```rust
/// Anchor a document hash to the vault.
/// Returns the sequential doc_id (1-indexed per vault).
anchor_document(
    env:         Env,
    vault_id:    u64,
    caller:      Address,   // must be the vault owner
    doc_hash:    BytesN<32>,
    doc_type:    LegalDocumentType,
    storage_ref: Option<String>,  // max 128 chars
) -> u32

/// Retrieve a specific anchor record.
get_document_anchor(env: Env, vault_id: u64, doc_id: u32) -> LegalDocumentAnchor

/// List all anchors for a vault (including soft-removed ones).
list_document_anchors(env: Env, vault_id: u64) -> Vec<LegalDocumentAnchor>

/// Soft-remove an anchor (sets removed = true; hash record is preserved).
remove_document_anchor(env: Env, vault_id: u64, caller: Address, doc_id: u32)
```

### Document Types

```rust
pub enum LegalDocumentType {
    Will,
    TrustDeed,
    PowerOfAttorney,
    Other,
}
```

---

## Backend API

### POST `/api/vaults/:vault_id/document-anchors`

Registers a new document anchor.

**Request body:**

```json
{
  "owner_address": "GOWNER...",
  "doc_hash_hex": "abcd...ef01",
  "doc_type": "will",
  "storage_ref": "ipfs://QmExampleCid..."
}
```

- `doc_hash_hex`: 64-character hex-encoded SHA-256 hash (required).
- `doc_type`: one of `will`, `trust_deed`, `power_of_attorney`, `other` (required).
- `storage_ref`: IPFS CID or URL pointing to the encrypted document off-chain (optional, max 128 chars).

**Response `201 Created`:**

```json
{
  "id": 1,
  "vault_id": "42",
  "doc_id": 1,
  "doc_hash_hex": "abcd...ef01",
  "doc_type": "will",
  "storage_ref": "ipfs://QmExampleCid...",
  "anchored_at": "2026-09-06T04:00:00.000Z",
  "removed": false
}
```

---

### GET `/api/vaults/:vault_id/document-anchors`

Returns all document anchors for a vault (including soft-removed).

**Response `200 OK`:** array of `DocumentAnchorRecord`.

---

### DELETE `/api/vaults/:vault_id/document-anchors/:doc_id`

Soft-removes a document anchor (sets `removed = true`). The hash record is preserved for audit purposes.

**Response `204 No Content`** on success.

---

## Frontend Component

The `DocumentAnchor` React component (`frontend/src/components/DocumentAnchor.tsx`) provides:

- **File picker** — select a local document; the SHA-256 hash is computed entirely in the browser using the Web Crypto API. No document bytes leave the device.
- **Hash preview** — shows the computed hash before submission.
- **Document type selector** — will, trust deed, power of attorney, other.
- **Storage reference** — optional IPFS CID or URL.
- **Anchor list** — displays all existing anchors with their hash, type, timestamp, and removal status.
- **Remove button** — soft-removes an anchor.

### Usage

```tsx
import { DocumentAnchor } from "./components/DocumentAnchor";

<DocumentAnchor
  vaultId="42"
  ownerAddress="GOWNER..."
  apiBaseUrl="http://localhost:3000"
/>
```

---

## Legal Disclaimer and Limitations

> ⚠️ **This feature does NOT provide legal advice and does NOT constitute legal execution of any document.**

### What this feature IS

- An **immutable cryptographic timestamp proof** that a document with a specific SHA-256 hash existed at a particular Stellar ledger timestamp.
- A convenient way to **link off-chain legal documents** to a vault's on-chain record.
- A **verification mechanism**: any third party can re-hash the original document and compare it to the stored anchor.

### What this feature IS NOT

- A substitute for proper legal execution (witness signatures, notarization, probate filing, etc.).
- Legal advice of any kind.
- A guarantee that courts or jurisdictions will recognise the anchor as legally valid evidence.
- A means to automatically enforce the document's contents — the smart contract has no knowledge of the document's text.

### Limitations

| Limitation | Detail |
|---|---|
| Hash-only | Only the SHA-256 hash is stored on-chain. Document contents are off-chain and the owner is responsible for preservation. |
| No decryption | The contract cannot read, interpret, or enforce the document's contents. |
| Soft-remove | "Removing" an anchor only sets a flag — the hash record is permanently in the ledger history. |
| Owner-controlled | Only the vault owner can anchor or remove documents. Beneficiaries have read-only access. |
| IPFS not guaranteed | Off-chain storage references (IPFS CIDs, URLs) may become unavailable if the content is not pinned. |
| Jurisdiction | Legal validity of a blockchain timestamp as evidence varies by jurisdiction. Consult local legal counsel. |

### Recommendations

1. **Consult a qualified estate-planning attorney** before relying on document anchoring for legal purposes.
2. **Store the original signed document** in a durable off-chain location (IPFS, encrypted cloud storage) and record the storage reference in the anchor.
3. **Keep copies** of all original documents in physically separate locations.
4. **Inform your beneficiaries** that documents are anchored and how to verify them.
5. **Do not rely solely on this feature** for your estate plan — use it as a complementary layer alongside traditional legal instruments.

---

## Security Considerations

- **SHA-256 collision resistance**: the probability of two different documents producing the same hash is negligible for practical purposes.
- **No private data on-chain**: raw document bytes, personally identifiable information, and financial details must remain off-chain.
- **Owner auth required**: all state-mutating operations require `caller.require_auth()` and verify `vault.owner == caller`.
- **Storage TTL**: anchor records use the same persistent storage TTL as the vault (based on `check_in_interval`). If the vault's TTL expires and the state is archived/pruned, anchors may also be pruned. Use `storage_ref` to point to a durable off-chain copy.

---

## Related

- [Architecture Overview](architecture.md)
- [TTL & State Archival Logic](ttl-logic.md)
- [Vault Hibernation](hibernation.md)
- [Security](security.md)
- [API Reference](api-reference.md)
