/**
 * Tests for DocumentAnchor component — Issue #1339
 *
 * Covers:
 *  1. Renders section heading and legal disclaimer on mount.
 *  2. Shows loading indicator while fetching anchors.
 *  3. Renders existing anchors returned by the API.
 *  4. Shows "No documents anchored yet" when the list is empty.
 *  5. Shows a load error when the fetch fails.
 *  6. Computes a SHA-256 hash preview after a file is selected.
 *  7. Submit button is disabled until a file is selected.
 *  8. Successful form submission adds the new anchor to the list.
 *  9. A submit error is displayed when the POST fails.
 * 10. Clicking "Remove" soft-removes an anchor (marks it removed).
 */

import React from "react";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { DocumentAnchor, DocumentAnchorRecord } from "../components/DocumentAnchor";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

// crypto.subtle.digest is not available in jsdom — provide a minimal stub.
Object.defineProperty(global, "crypto", {
  value: {
    subtle: {
      digest: jest.fn().mockResolvedValue(new Uint8Array(32).fill(0xab).buffer),
    },
  },
  writable: true,
  configurable: true,
});

// Helpers -----------------------------------------------------------------

const VAULT_ID = "vault-42";
const OWNER = "GOWNER1234567890";
const API_BASE = "http://localhost:3000";

const makeAnchor = (docId: number, removed = false): DocumentAnchorRecord => ({
  id: docId,
  vault_id: VAULT_ID,
  doc_id: docId,
  doc_hash_hex: "a".repeat(64),
  doc_type: "will",
  storage_ref: null,
  anchored_at: "2026-01-01T00:00:00.000Z",
  removed,
});

function mockFetch(response: unknown, status = 200, method = "GET") {
  (global.fetch as jest.Mock) = jest.fn().mockResolvedValueOnce({
    ok: status < 400,
    status,
    json: async () => response,
  });
}

function mockFetchSequence(
  responses: Array<{ body: unknown; status?: number }>
) {
  const mockImpl = responses.reduce((mock, { body, status = 200 }) => {
    return mock.mockResolvedValueOnce({
      ok: (status ?? 200) < 400,
      status: status ?? 200,
      json: async () => body,
    });
  }, jest.fn() as jest.Mock);
  (global.fetch as jest.Mock) = mockImpl;
}

// ---------------------------------------------------------------------------
// 1. Renders heading and legal disclaimer on mount
// ---------------------------------------------------------------------------

test("renders heading and legal disclaimer", async () => {
  mockFetch([]);
  render(
    <DocumentAnchor vaultId={VAULT_ID} ownerAddress={OWNER} apiBaseUrl={API_BASE} />
  );
  expect(screen.getByText(/Legal Document Anchoring/i)).toBeInTheDocument();
  expect(screen.getByTestId("legal-disclaimer")).toBeInTheDocument();
  expect(screen.getByTestId("legal-disclaimer")).toHaveTextContent(/disclaimer/i);
  await waitFor(() => expect(screen.queryByTestId("loading-indicator")).not.toBeInTheDocument());
});

// ---------------------------------------------------------------------------
// 2. Shows loading indicator while fetching
// ---------------------------------------------------------------------------

test("shows loading indicator while fetching anchors", () => {
  // Never resolves — stays loading
  (global.fetch as jest.Mock) = jest.fn(() => new Promise(() => {}));
  render(
    <DocumentAnchor vaultId={VAULT_ID} ownerAddress={OWNER} apiBaseUrl={API_BASE} />
  );
  expect(screen.getByTestId("loading-indicator")).toBeInTheDocument();
});

// ---------------------------------------------------------------------------
// 3. Renders existing anchors returned by API
// ---------------------------------------------------------------------------

test("renders existing anchors from API", async () => {
  const anchors = [makeAnchor(1), makeAnchor(2)];
  mockFetch(anchors);
  render(
    <DocumentAnchor vaultId={VAULT_ID} ownerAddress={OWNER} apiBaseUrl={API_BASE} />
  );
  await waitFor(() =>
    expect(screen.getByTestId("anchor-item-1")).toBeInTheDocument()
  );
  expect(screen.getByTestId("anchor-item-2")).toBeInTheDocument();
  expect(screen.getByTestId("anchor-list")).toBeInTheDocument();
});

// ---------------------------------------------------------------------------
// 4. Shows empty-state message when no anchors exist
// ---------------------------------------------------------------------------

test("shows empty-state message when no anchors", async () => {
  mockFetch([]);
  render(
    <DocumentAnchor vaultId={VAULT_ID} ownerAddress={OWNER} apiBaseUrl={API_BASE} />
  );
  await waitFor(() =>
    expect(screen.getByTestId("no-anchors-message")).toBeInTheDocument()
  );
});

// ---------------------------------------------------------------------------
// 5. Shows load error when fetch fails
// ---------------------------------------------------------------------------

test("shows load error when fetch fails", async () => {
  (global.fetch as jest.Mock) = jest.fn().mockRejectedValueOnce(new Error("Network error"));
  render(
    <DocumentAnchor vaultId={VAULT_ID} ownerAddress={OWNER} apiBaseUrl={API_BASE} />
  );
  await waitFor(() =>
    expect(screen.getByTestId("load-error")).toBeInTheDocument()
  );
  expect(screen.getByTestId("load-error")).toHaveTextContent(/Network error/i);
});

// ---------------------------------------------------------------------------
// 6. Shows SHA-256 hash preview after file selection
// ---------------------------------------------------------------------------

test("shows hash preview after file selected", async () => {
  mockFetch([]);
  render(
    <DocumentAnchor vaultId={VAULT_ID} ownerAddress={OWNER} apiBaseUrl={API_BASE} />
  );
  await waitFor(() => expect(screen.queryByTestId("loading-indicator")).not.toBeInTheDocument());

  const file = new File(["hello world"], "will.pdf", { type: "application/pdf" });
  const input = screen.getByTestId("file-input");

  await act(async () => {
    fireEvent.change(input, { target: { files: [file] } });
  });

  await waitFor(() =>
    expect(screen.getByTestId("hash-preview")).toBeInTheDocument()
  );
});

// ---------------------------------------------------------------------------
// 7. Submit button is disabled until a file is selected
// ---------------------------------------------------------------------------

test("submit button is disabled before file selected", async () => {
  mockFetch([]);
  render(
    <DocumentAnchor vaultId={VAULT_ID} ownerAddress={OWNER} apiBaseUrl={API_BASE} />
  );
  await waitFor(() => expect(screen.queryByTestId("loading-indicator")).not.toBeInTheDocument());

  const btn = screen.getByTestId("anchor-submit-btn");
  expect(btn).toBeDisabled();
});

// ---------------------------------------------------------------------------
// 8. Successful submission adds anchor to list
// ---------------------------------------------------------------------------

test("successful submission adds new anchor to list", async () => {
  const newAnchor = makeAnchor(1);
  // First call: initial fetch (empty), second call: POST response
  mockFetchSequence([
    { body: [] },
    { body: newAnchor, status: 201 },
  ]);

  render(
    <DocumentAnchor vaultId={VAULT_ID} ownerAddress={OWNER} apiBaseUrl={API_BASE} />
  );
  await waitFor(() => expect(screen.queryByTestId("loading-indicator")).not.toBeInTheDocument());

  // Select a file
  const file = new File(["content"], "trust.pdf", { type: "application/pdf" });
  await act(async () => {
    fireEvent.change(screen.getByTestId("file-input"), {
      target: { files: [file] },
    });
  });

  // Wait for hash
  await waitFor(() => screen.getByTestId("hash-preview"));

  // Submit
  await act(async () => {
    fireEvent.click(screen.getByTestId("anchor-submit-btn"));
  });

  await waitFor(() =>
    expect(screen.getByTestId("submit-success")).toBeInTheDocument()
  );
  expect(screen.getByTestId("anchor-item-1")).toBeInTheDocument();
});

// ---------------------------------------------------------------------------
// 9. Submit error is displayed when POST fails
// ---------------------------------------------------------------------------

test("shows submit error when POST fails", async () => {
  // First call: initial fetch, second call: POST failure
  mockFetchSequence([
    { body: [] },
    { body: { message: "Vault not found" }, status: 404 },
  ]);

  render(
    <DocumentAnchor vaultId={VAULT_ID} ownerAddress={OWNER} apiBaseUrl={API_BASE} />
  );
  await waitFor(() => expect(screen.queryByTestId("loading-indicator")).not.toBeInTheDocument());

  const file = new File(["data"], "doc.pdf", { type: "application/pdf" });
  await act(async () => {
    fireEvent.change(screen.getByTestId("file-input"), {
      target: { files: [file] },
    });
  });

  await waitFor(() => screen.getByTestId("hash-preview"));

  await act(async () => {
    fireEvent.click(screen.getByTestId("anchor-submit-btn"));
  });

  await waitFor(() =>
    expect(screen.getByTestId("submit-error")).toBeInTheDocument()
  );
  expect(screen.getByTestId("submit-error")).toHaveTextContent(/Vault not found/i);
});

// ---------------------------------------------------------------------------
// 10. Clicking Remove soft-removes an anchor
// ---------------------------------------------------------------------------

test("clicking Remove marks anchor as removed", async () => {
  const anchors = [makeAnchor(1, false)];
  // First call: initial fetch; second call: DELETE
  mockFetchSequence([
    { body: anchors },
    { body: null, status: 204 },
  ]);

  // DELETE returns no body — patch json to return null gracefully
  (global.fetch as jest.Mock) = jest
    .fn()
    .mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: async () => anchors,
    })
    .mockResolvedValueOnce({
      ok: true,
      status: 204,
      json: async () => null,
    });

  render(
    <DocumentAnchor vaultId={VAULT_ID} ownerAddress={OWNER} apiBaseUrl={API_BASE} />
  );

  await waitFor(() =>
    expect(screen.getByTestId("remove-btn-1")).toBeInTheDocument()
  );

  await act(async () => {
    fireEvent.click(screen.getByTestId("remove-btn-1"));
  });

  await waitFor(() =>
    expect(screen.getByTestId("removed-badge-1")).toBeInTheDocument()
  );
});
