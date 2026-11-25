/**
 * Step 2: Beneficiary configuration.
 *
 * Supports one or multiple beneficiaries. All percentages must sum to 100.
 *
 * Issue #2067.
 */

import React from "react";
import { BeneficiaryEntry, WizardDraft } from "../types";

interface StepBeneficiariesProps {
  draft: WizardDraft;
  onChange: (patch: Partial<WizardDraft>) => void;
}

export function StepBeneficiaries({ draft, onChange }: StepBeneficiariesProps) {
  const { beneficiaries } = draft;

  const totalPct = beneficiaries.reduce((sum, b) => sum + (b.percentage || 0), 0);
  const isValid = totalPct === 100 && beneficiaries.every((b) => b.address.trim() !== "");

  function updateEntry(idx: number, patch: Partial<BeneficiaryEntry>) {
    const updated = beneficiaries.map((b, i) => (i === idx ? { ...b, ...patch } : b));
    onChange({ beneficiaries: updated });
  }

  function addEntry() {
    onChange({
      beneficiaries: [...beneficiaries, { address: "", percentage: 0 }],
    });
  }

  function removeEntry(idx: number) {
    onChange({ beneficiaries: beneficiaries.filter((_, i) => i !== idx) });
  }

  return (
    <section aria-labelledby="step-beneficiaries-heading">
      <h2 id="step-beneficiaries-heading" style={{ marginTop: 0 }}>
        Beneficiaries
      </h2>
      <p style={{ color: "#555", marginBottom: 24 }}>
        Add one or more beneficiaries. Percentages must add up to exactly 100.
      </p>

      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {beneficiaries.map((b, idx) => (
          <div
            key={idx}
            style={{
              display: "grid",
              gridTemplateColumns: "1fr 100px auto",
              gap: 8,
              alignItems: "end",
            }}
          >
            <label>
              <span style={labelStyle}>Stellar address</span>
              <input
                type="text"
                value={b.address}
                placeholder="G…"
                onChange={(e) => updateEntry(idx, { address: e.target.value })}
                style={inputStyle}
                aria-label={`Beneficiary ${idx + 1} address`}
              />
            </label>

            <label>
              <span style={labelStyle}>%</span>
              <input
                type="number"
                value={b.percentage}
                min={1}
                max={100}
                onChange={(e) =>
                  updateEntry(idx, { percentage: Number(e.target.value) })
                }
                style={inputStyle}
                aria-label={`Beneficiary ${idx + 1} percentage`}
              />
            </label>

            {beneficiaries.length > 1 && (
              <button
                type="button"
                onClick={() => removeEntry(idx)}
                style={removeBtnStyle}
                aria-label={`Remove beneficiary ${idx + 1}`}
              >
                ✕
              </button>
            )}
          </div>
        ))}
      </div>

      <div style={{ marginTop: 12, display: "flex", gap: 12, alignItems: "center" }}>
        <button type="button" onClick={addEntry} style={addBtnStyle}>
          + Add beneficiary
        </button>

        <span
          style={{
            fontSize: 13,
            color: isValid ? "#2e7d32" : "#d32f2f",
            fontWeight: 600,
          }}
          role="status"
          aria-live="polite"
        >
          Total: {totalPct}% {totalPct === 100 ? "✓" : "(must equal 100)"}
        </span>
      </div>
    </section>
  );
}

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: 13,
  fontWeight: 600,
  marginBottom: 4,
  color: "#333",
};

const inputStyle: React.CSSProperties = {
  display: "block",
  width: "100%",
  padding: "8px 10px",
  border: "1px solid #ccc",
  borderRadius: 4,
  fontSize: 14,
  boxSizing: "border-box",
};

const addBtnStyle: React.CSSProperties = {
  padding: "6px 14px",
  background: "#1976d2",
  color: "#fff",
  border: "none",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: 13,
};

const removeBtnStyle: React.CSSProperties = {
  padding: "8px 10px",
  background: "#ffebee",
  color: "#d32f2f",
  border: "1px solid #ef9a9a",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: 13,
  alignSelf: "flex-end",
};
