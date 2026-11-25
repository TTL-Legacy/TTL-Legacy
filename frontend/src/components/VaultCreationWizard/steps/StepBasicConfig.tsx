/**
 * Step 1: Basic vault configuration.
 *
 * Issue #2067.
 */

import React from "react";
import { WizardDraft } from "../types";

interface StepBasicConfigProps {
  draft: WizardDraft;
  onChange: (patch: Partial<WizardDraft>) => void;
}

export function StepBasicConfig({ draft, onChange }: StepBasicConfigProps) {
  return (
    <section aria-labelledby="step-basic-heading">
      <h2 id="step-basic-heading" style={{ marginTop: 0 }}>
        Basic Configuration
      </h2>
      <p style={{ color: "#555", marginBottom: 24 }}>
        Set the core parameters for your vault. You can change these in the
        Review step before signing.
      </p>

      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <label>
          <span style={labelStyle}>Vault label</span>
          <input
            type="text"
            value={draft.label}
            placeholder="My inheritance vault"
            maxLength={64}
            onChange={(e) => onChange({ label: e.target.value })}
            style={inputStyle}
          />
        </label>

        <label>
          <span style={labelStyle}>Check-in interval (seconds)</span>
          <input
            type="number"
            value={draft.checkInIntervalSeconds}
            min={60}
            step={60}
            onChange={(e) =>
              onChange({ checkInIntervalSeconds: Number(e.target.value) })
            }
            style={inputStyle}
          />
          <span style={hintStyle}>
            How often you must check in. 86400 = 24 h, 604800 = 7 days.
          </span>
        </label>

        <label>
          <span style={labelStyle}>Token contract address</span>
          <input
            type="text"
            value={draft.tokenAddress}
            placeholder="Leave blank to use native XLM"
            onChange={(e) => onChange({ tokenAddress: e.target.value })}
            style={inputStyle}
          />
        </label>

        <label>
          <span style={labelStyle}>Max deposit amount (stroops, optional)</span>
          <input
            type="text"
            value={draft.maxDepositAmountStroops}
            placeholder="Leave blank for no cap"
            pattern="[0-9]*"
            onChange={(e) =>
              onChange({ maxDepositAmountStroops: e.target.value })
            }
            style={inputStyle}
          />
        </label>
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

const hintStyle: React.CSSProperties = {
  fontSize: 12,
  color: "#757575",
  marginTop: 4,
  display: "block",
};
