/**
 * Step 3: Release condition selection.
 *
 * Issue #2067.
 */

import React from "react";
import { ReleaseCondition, WizardDraft } from "../types";

const RELEASE_OPTIONS: { value: ReleaseCondition; label: string; description: string }[] = [
  {
    value: "ttl_expiry",
    label: "TTL expiry",
    description:
      "Funds release automatically when the check-in deadline passes without a check-in.",
  },
  {
    value: "owner_initiated",
    label: "Owner initiated",
    description:
      "Only the vault owner can manually trigger the release at any time.",
  },
  {
    value: "oracle",
    label: "Oracle",
    description:
      "An external oracle contract determines when the vault is eligible for release.",
  },
];

interface StepReleaseConditionsProps {
  draft: WizardDraft;
  onChange: (patch: Partial<WizardDraft>) => void;
}

export function StepReleaseConditions({ draft, onChange }: StepReleaseConditionsProps) {
  return (
    <section aria-labelledby="step-release-heading">
      <h2 id="step-release-heading" style={{ marginTop: 0 }}>
        Release Conditions
      </h2>
      <p style={{ color: "#555", marginBottom: 24 }}>
        Choose what triggers the vault to release funds to your beneficiaries.
      </p>

      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {RELEASE_OPTIONS.map((opt) => {
          const selected = draft.releaseCondition === opt.value;
          return (
            <label
              key={opt.value}
              style={{
                display: "flex",
                alignItems: "flex-start",
                gap: 12,
                padding: 12,
                border: `2px solid ${selected ? "#1976d2" : "#e0e0e0"}`,
                borderRadius: 6,
                cursor: "pointer",
                background: selected ? "#e3f2fd" : "#fff",
              }}
            >
              <input
                type="radio"
                name="releaseCondition"
                value={opt.value}
                checked={selected}
                onChange={() => onChange({ releaseCondition: opt.value })}
                style={{ marginTop: 2 }}
              />
              <div>
                <div style={{ fontWeight: 600, fontSize: 14, marginBottom: 2 }}>
                  {opt.label}
                </div>
                <div style={{ fontSize: 13, color: "#555" }}>{opt.description}</div>
              </div>
            </label>
          );
        })}
      </div>

      {draft.releaseCondition === "oracle" && (
        <div style={{ marginTop: 16 }}>
          <label>
            <span
              style={{
                display: "block",
                fontSize: 13,
                fontWeight: 600,
                marginBottom: 4,
                color: "#333",
              }}
            >
              Oracle contract address
            </span>
            <input
              type="text"
              value={draft.oracleAddress}
              placeholder="C… (Soroban contract address)"
              onChange={(e) => onChange({ oracleAddress: e.target.value })}
              style={{
                display: "block",
                width: "100%",
                padding: "8px 10px",
                border: "1px solid #ccc",
                borderRadius: 4,
                fontSize: 14,
                boxSizing: "border-box",
              }}
            />
          </label>
        </div>
      )}
    </section>
  );
}
