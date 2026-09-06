/**
 * DocumentAnchor — Issue #1339
 *
 * UI component that lets vault owners:
 *   1. Upload (select) a document and compute its SHA-256 hash client-side.
 *   2. Submit the hash to the backend to anchor it on Stellar.
 *   3. View a list of previously anchored documents for the vault.
 *   4. Soft-remove (revoke) an anchor.
 *
 * **Legal disclaimer**: anchoring a document hash does NOT constitute legal
 * execution of the underlying document.  It provides an immutable, timestamped
 * cryptographic proof that a document with the recorded hash existed at the
 * time of anchoring.  Consult a qualified legal professional for estate-planning
 * advice.
 *
 * No raw document bytes are sent to the backend or stored on-chain — only the
 * SHA-256 hash is transmitted.
 */

import React, { useState, useCallback, useEffect } from "react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type LegalDocumentType = "will" | "trust_deed" | "power_of_attorney" | "other";

export interface DocumentAnchorRecord {
  id: number;
  vault_id: string;
  doc_id: number;
  doc_hash_hex: string;
  doc_type: LegalDocumentType;
  storage_ref: string | null;
  anchored_at: string; // ISO-8601
  removed: boolean;
}

interface AnchorDocumentRequest {
  owner_address: string;
  doc_hash_hex: string;
  doc_type: LegalDocumentType;
  storage_ref?: string;
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface DocumentAnchorProps {
  /** The numeric vault ID to anchor documents against. */
  vaultId: string;
  /** Stellar account address of the vault owner (used for the POST body). */
  ownerAddress: string;
  /**
   * Base URL for the backend API (e.g. "http://localhost:3000").
   * Defaults to an empty string (same-origin).
   */
  apiBaseUrl?: string;
}

// ---------------------------------------------------------------------------
// Utility: compute SHA-256 of a File using the Web Crypto API
// ---------------------------------------------------------------------------

async function sha256Hex(file: File): Promise<string> {
  const buffer = await file.arrayBuffer();
  const hashBuffer = await crypto.subtle.digest("SHA-256", buffer);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface AnchorListProps {
  anchors: DocumentAnchorRecord[];
  onRemove: (docId: number) => void;
  removing: Set<number>;
}

function AnchorList({ anchors, onRemove, removing }: AnchorListProps) {
  if (anchors.length === 0) {
    return (
      <p
        data-testid="no-anchors-message"
        style={{ color: "#888", fontStyle: "italic" }}
      >
        No documents anchored yet.
      </p>
    );
  }

  return (
    <ul
      data-testid="anchor-list"
      style={{ listStyle: "none", padding: 0, margin: 0 }}
    >
      {anchors.map((anchor) => (
        <li
          key={anchor.doc_id}
          data-testid={`anchor-item-${anchor.doc_id}`}
          style={{
            display: "flex",
            alignItems: "flex-start",
            gap: "1rem",
            padding: "0.75rem 0",
            borderBottom: "1px solid #eee",
            opacity: anchor.removed ? 0.45 : 1,
          }}
        >
          <div style={{ flex: 1 }}>
            <div style={{ fontWeight: 600, marginBottom: "0.25rem" }}>
              #{anchor.doc_id} — {anchor.doc_type.replace(/_/g, " ")}
              {anchor.removed && (
                <span
                  data-testid={`removed-badge-${anchor.doc_id}`}
                  style={{
                    marginLeft: "0.5rem",
                    background: "#f44336",
                    color: "#fff",
                    padding: "0 6px",
                    borderRadius: "3px",
                    fontSize: "0.75rem",
                  }}
                >
                  removed
                </span>
              )}
            </div>
            <div
              style={{
                fontFamily: "monospace",
                fontSize: "0.8rem",
                wordBreak: "break-all",
                color: "#444",
              }}
            >
              <span title="SHA-256 document hash">🔒 {anchor.doc_hash_hex}</span>
            </div>
            {anchor.storage_ref && (
              <div style={{ marginTop: "0.25rem", fontSize: "0.8rem", color: "#555" }}>
                📎{" "}
                <a href={anchor.storage_ref} target="_blank" rel="noopener noreferrer">
                  {anchor.storage_ref}
                </a>
              </div>
            )}
            <div style={{ marginTop: "0.25rem", fontSize: "0.75rem", color: "#888" }}>
              Anchored: {new Date(anchor.anchored_at).toLocaleString()}
            </div>
          </div>
          {!anchor.removed && (
            <button
              data-testid={`remove-btn-${anchor.doc_id}`}
              onClick={() => onRemove(anchor.doc_id)}
              disabled={removing.has(anchor.doc_id)}
              aria-label={`Remove anchor for document ${anchor.doc_id}`}
              style={{
                background: "none",
                border: "1px solid #ccc",
                borderRadius: "4px",
                cursor: "pointer",
                padding: "0.25rem 0.5rem",
                fontSize: "0.8rem",
                color: "#666",
                whiteSpace: "nowrap",
              }}
            >
              {removing.has(anchor.doc_id) ? "Removing…" : "Remove"}
            </button>
          )}
        </li>
      ))}
    </ul>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function DocumentAnchor({
  vaultId,
  ownerAddress,
  apiBaseUrl = "",
}: DocumentAnchorProps) {
  // ── State ─────────────────────────────────────────────────────────────────
  const [anchors, setAnchors] = useState<DocumentAnchorRecord[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // Form state
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [docType, setDocType] = useState<LegalDocumentType>("will");
  const [storageRef, setStorageRef] = useState("");
  const [hashPreview, setHashPreview] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [submitSuccess, setSubmitSuccess] = useState(false);

  // Remove state
  const [removing, setRemoving] = useState<Set<number>>(new Set());

  // ── Fetch existing anchors ────────────────────────────────────────────────
  const fetchAnchors = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const res = await fetch(
        `${apiBaseUrl}/api/vaults/${vaultId}/document-anchors`
      );
      if (!res.ok) {
        throw new Error(`Server returned ${res.status}`);
      }
      const data: DocumentAnchorRecord[] = await res.json();
      setAnchors(data);
    } catch (err: unknown) {
      setLoadError(err instanceof Error ? err.message : "Failed to load anchors");
    } finally {
      setLoading(false);
    }
  }, [apiBaseUrl, vaultId]);

  useEffect(() => {
    fetchAnchors();
  }, [fetchAnchors]);

  // ── File selection → hash preview ─────────────────────────────────────────
  const handleFileChange = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0] ?? null;
      setSelectedFile(file);
      setHashPreview(null);
      setSubmitError(null);
      setSubmitSuccess(false);
      if (file) {
        const hex = await sha256Hex(file);
        setHashPreview(hex);
      }
    },
    []
  );

  // ── Submit anchor ─────────────────────────────────────────────────────────
  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!hashPreview) {
        setSubmitError("Please select a document first.");
        return;
      }

      const body: AnchorDocumentRequest = {
        owner_address: ownerAddress,
        doc_hash_hex: hashPreview,
        doc_type: docType,
        storage_ref: storageRef.trim() || undefined,
      };

      setSubmitting(true);
      setSubmitError(null);
      setSubmitSuccess(false);

      try {
        const res = await fetch(
          `${apiBaseUrl}/api/vaults/${vaultId}/document-anchors`,
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
          }
        );

        if (!res.ok) {
          const errBody = await res.json().catch(() => ({}));
          throw new Error((errBody as { message?: string }).message ?? `Server error ${res.status}`);
        }

        const newAnchor: DocumentAnchorRecord = await res.json();
        setAnchors((prev) => [...prev, newAnchor]);
        setSubmitSuccess(true);
        // Reset form
        setSelectedFile(null);
        setHashPreview(null);
        setStorageRef("");
      } catch (err: unknown) {
        setSubmitError(err instanceof Error ? err.message : "Failed to anchor document");
      } finally {
        setSubmitting(false);
      }
    },
    [apiBaseUrl, vaultId, ownerAddress, docType, hashPreview, storageRef]
  );

  // ── Remove anchor ─────────────────────────────────────────────────────────
  const handleRemove = useCallback(
    async (docId: number) => {
      setRemoving((prev) => new Set(prev).add(docId));
      try {
        const res = await fetch(
          `${apiBaseUrl}/api/vaults/${vaultId}/document-anchors/${docId}`,
          { method: "DELETE" }
        );
        if (!res.ok) {
          throw new Error(`Server returned ${res.status}`);
        }
        // Update local state: mark as removed
        setAnchors((prev) =>
          prev.map((a) => (a.doc_id === docId ? { ...a, removed: true } : a))
        );
      } catch {
        // Surface error in a real app; silently ignored here for brevity
      } finally {
        setRemoving((prev) => {
          const next = new Set(prev);
          next.delete(docId);
          return next;
        });
      }
    },
    [apiBaseUrl, vaultId]
  );

  // ── Render ────────────────────────────────────────────────────────────────
  return (
    <section
      data-testid="document-anchor-section"
      style={{ fontFamily: "sans-serif", maxWidth: "680px", margin: "0 auto" }}
    >
      <h2 style={{ marginBottom: "0.5rem" }}>Legal Document Anchoring</h2>

      {/* Legal disclaimer */}
      <div
        data-testid="legal-disclaimer"
        role="note"
        style={{
          background: "#fff8e1",
          border: "1px solid #ffe082",
          borderRadius: "6px",
          padding: "0.75rem 1rem",
          marginBottom: "1.5rem",
          fontSize: "0.875rem",
          lineHeight: "1.5",
        }}
      >
        <strong>⚠️ Legal Disclaimer:</strong> Anchoring a document hash creates an
        immutable on-chain timestamp proof that the document existed at the time of
        anchoring. It does <em>not</em> constitute legal execution of any document.
        Consult a qualified legal professional for estate-planning advice.
      </div>

      {/* Upload form */}
      <form
        data-testid="anchor-form"
        onSubmit={handleSubmit}
        style={{ marginBottom: "2rem" }}
        aria-label="Anchor a legal document"
      >
        <div style={{ marginBottom: "1rem" }}>
          <label htmlFor="doc-upload" style={{ display: "block", marginBottom: "0.25rem" }}>
            Select document to anchor <span aria-hidden="true">*</span>
          </label>
          <input
            id="doc-upload"
            data-testid="file-input"
            type="file"
            onChange={handleFileChange}
            aria-describedby="hash-preview"
            aria-required="true"
          />
          {hashPreview && (
            <div
              id="hash-preview"
              data-testid="hash-preview"
              style={{ marginTop: "0.5rem", fontSize: "0.8rem", color: "#555" }}
            >
              SHA-256:{" "}
              <code
                style={{
                  wordBreak: "break-all",
                  background: "#f5f5f5",
                  padding: "2px 4px",
                  borderRadius: "3px",
                }}
              >
                {hashPreview}
              </code>
            </div>
          )}
        </div>

        <div style={{ marginBottom: "1rem" }}>
          <label htmlFor="doc-type" style={{ display: "block", marginBottom: "0.25rem" }}>
            Document type <span aria-hidden="true">*</span>
          </label>
          <select
            id="doc-type"
            data-testid="doc-type-select"
            value={docType}
            onChange={(e) => setDocType(e.target.value as LegalDocumentType)}
            aria-required="true"
            style={{ padding: "0.35rem 0.5rem", minWidth: "200px" }}
          >
            <option value="will">Will</option>
            <option value="trust_deed">Trust Deed</option>
            <option value="power_of_attorney">Power of Attorney</option>
            <option value="other">Other</option>
          </select>
        </div>

        <div style={{ marginBottom: "1rem" }}>
          <label htmlFor="storage-ref" style={{ display: "block", marginBottom: "0.25rem" }}>
            Storage reference (optional IPFS CID or URL, max 128 chars)
          </label>
          <input
            id="storage-ref"
            data-testid="storage-ref-input"
            type="text"
            value={storageRef}
            onChange={(e) => setStorageRef(e.target.value)}
            maxLength={128}
            placeholder="ipfs://Qm…"
            style={{ width: "100%", padding: "0.35rem 0.5rem", boxSizing: "border-box" }}
          />
        </div>

        {submitError && (
          <div
            data-testid="submit-error"
            role="alert"
            style={{ color: "#c62828", marginBottom: "0.75rem" }}
          >
            {submitError}
          </div>
        )}

        {submitSuccess && (
          <div
            data-testid="submit-success"
            role="status"
            style={{ color: "#2e7d32", marginBottom: "0.75rem" }}
          >
            ✓ Document anchored successfully.
          </div>
        )}

        <button
          type="submit"
          data-testid="anchor-submit-btn"
          disabled={submitting || !hashPreview}
          style={{
            background: "#1565c0",
            color: "#fff",
            border: "none",
            borderRadius: "5px",
            padding: "0.5rem 1.25rem",
            cursor: submitting || !hashPreview ? "not-allowed" : "pointer",
            opacity: submitting || !hashPreview ? 0.6 : 1,
          }}
          aria-disabled={submitting || !hashPreview}
        >
          {submitting ? "Anchoring…" : "Anchor Document"}
        </button>
      </form>

      {/* Anchor list */}
      <h3 style={{ marginBottom: "0.75rem" }}>Anchored Documents</h3>

      {loading && (
        <p data-testid="loading-indicator" aria-live="polite">
          Loading…
        </p>
      )}

      {loadError && (
        <div
          data-testid="load-error"
          role="alert"
          style={{ color: "#c62828" }}
        >
          {loadError}
        </div>
      )}

      {!loading && !loadError && (
        <AnchorList anchors={anchors} onRemove={handleRemove} removing={removing} />
      )}
    </section>
  );
}

export default DocumentAnchor;
