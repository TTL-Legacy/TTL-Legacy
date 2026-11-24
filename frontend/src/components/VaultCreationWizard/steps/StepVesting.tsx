/**
 * Step 4: Vesting schedule configuration.
 *
 * Also exposes the cliff grace period added in Issue #2065.
 *
 * Issue #2067.
 */

import React from "react";
import { VestingConfig, WizardDraft } from "../types";

interface StepVestingProps {
  draft: WizardDraft;
  onChange: (patch: Partial<WizardDraft>) => void;
}

export function StepVesting({ draft, onChange }: StepVestingProps) {
  const { vesting } = draft;

  function updateVesting(patch: Partial<VestingConfig>) {
    onChange({ vesting: { ...vesting, ...patch } });
  }

  return (
    <section aria-labelledby="step-vesting-heading">
      <h2 id="step-vesting-heading" style={{ marginTop: 0 }}>
        Vesting Schedule
      </h2>
      <p style={{ color: "#555", marginBottom: 24 }}>
        Optionally release funds in installments over time rather than all at
        once.
      </p>

      <label style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 20 }}>
        <input
          type="checkbox"
          checked={vesting.enabled}
          onChange={(e) => updateVesting({ enabled: e.target.checked })}
        />
        <span style={{ fontWeight: 600, fontSize: 14 }}>Enable vesting schedule</span>
      </label>

      {vesting.enabled && (
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <label>
            <span style={labelStyle}>Start date (Unix timestamp, seconds)</span>
            <input
              type="number"
              value={vesting.startTime || ""}
              min={0}
              placeholder={String(Math.floor(Date.now() / 1000))}
              onChange={(e) => updateVesting({ startTime: Number(e.target.value) })}
              style={inputStyle}
            />
            <span style={hintStyle}>Leave as 0 to use the vault creation timestamp.</span>
          </label>

          <label>
            <span style={labelStyle}>Interval between installments (seconds)</span>
            <input
              type="number"
              value={vesting.intervalSeconds}
              min={1}
              onChange={(e) => updateVesting({ intervalSeconds: Number(e.target.value) })}
              style={inputStyle}
            />
            <span style={hintStyle}>2592000 = 30 days.</span>
          </label>

          <label>
            <span style={labelStyle}>Number of installments</span>
            <input
              type="number"
              value={vesting.numInstallments}
              min={1}
              onChange={(e) => updateVesting({ numInstallments: Number(e.target.value) })}
              style={inputStyle}
            />
          </label>

          <label>
            <span style={labelStyle}>Total amount to vest (stroops)</span>
            <input
              type="text"
              value={vesting.totalAmountStroops}
              placeholder="e.g. 10000000000"
              pattern="[0-9]*"
              onChange={(e) => updateVesting({ totalAmountStroops: e.target.value })}
              style={inputStyle}
            />
            <span style={hintStyle}>1 XLM = 10 000 000 stroops.</span>
          </label>

          <label>
            <span style={labelStyle}>Cliff period (seconds, 0 = no cliff)</span>
            <input
              type="number"
              value={vesting.cliffPeriodSeconds}
              min={0}
              onChange={(e) =>
                updateVesting({ cliffPeriodSeconds: Number(e.target.value) })
              }
              style={inputStyle}
            />
            <span style={hintStyle}>No funds can be claimed until this many seconds after the start time.</span>
          </label>

          {vesting.cliffPeriodSeconds > 0 && (
            <label>
              <span style={labelStyle}>Cliff grace period (seconds, 0 = disabled)</span>
              <input
                type="number"
                value={vesting.cliffGracePeriodSeconds}
                min={0}
                onChange={(e) =>
                  updateVesting({ cliffGracePeriodSeconds: Number(e.target.value) })
                }
                style={inputStyle}
              />
              <span style={hintStyle}>
                Claims made within this window after the cliff date are treated as
                on-time and are not subject to any late-claim penalty. (Issue #2065)
              </span>
            </label>
          )}
        </div>
      )}
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
