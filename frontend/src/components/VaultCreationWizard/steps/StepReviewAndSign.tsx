/**
 * Step 5: Review and sign.
 *
 * Displays a summary of all wizard choices and provides the submit action.
 *
 * Issue #2067.
 */

import React from "react";
import { WizardDraft } from "../types";

interface StepReviewAndSignProps {
  draft: WizardDraft;
  onSubmit: () => void;
  isSubmitting: boolean;
  submitError?: string;
}

export function StepReviewAndSign({
  draft,
  onSubmit,
  isSubmitting,
  submitError,
}: StepReviewAndSignProps) {
  const totalBps = draft.beneficiaries.reduce((s, b) => s + b.percentage, 0);

  return (
    <section aria-labelledby="step-review-heading">
      <h2 id="step-review-heading" style={{ marginTop: 0 }}>
        Review &amp; Sign
      </h2>
      <p style={{ color: "#555", marginBottom: 24 }}>
        Check everything below before signing. Use the step navigation above
        to go back and make changes.
      </p>

      <dl style={dlStyle}>
        <Section title="Basic Config">
          <Row label="Label" value={draft.label || "(none)"} />
          <Row label="Check-in interval" value={`${draft.checkInIntervalSeconds} s`} />
          <Row label="Token address" value={draft.tokenAddress || "Native XLM"} />
          <Row
            label="Max deposit"
            value={
              draft.maxDepositAmountStroops
                ? `${draft.maxDepositAmountStroops} stroops`
                : "No cap"
            }
          />
        </Section>

        <Section title="Beneficiaries">
          {draft.beneficiaries.map((b, i) => (
            <Row
              key={i}
              label={`Beneficiary ${i + 1}`}
              value={`${b.address} — ${b.percentage}%`}
            />
          ))}
          <Row
            label="Total"
            value={
              <span style={{ color: totalBps === 100 ? "#2e7d32" : "#d32f2f" }}>
                {totalBps}%
              </span>
            }
          />
        </Section>

        <Section title="Release Condition">
          <Row label="Condition" value={draft.releaseCondition} />
          {draft.releaseCondition === "oracle" && (
            <Row label="Oracle address" value={draft.oracleAddress || "(not set)"} />
          )}
        </Section>

        <Section title="Vesting">
          {draft.vesting.enabled ? (
            <>
              <Row label="Start time" value={`${draft.vesting.startTime} (Unix s)`} />
              <Row label="Interval" value={`${draft.vesting.intervalSeconds} s`} />
              <Row label="Installments" value={String(draft.vesting.numInstallments)} />
              <Row label="Total" value={`${draft.vesting.totalAmountStroops} stroops`} />
              <Row
                label="Cliff"
                value={
                  draft.vesting.cliffPeriodSeconds > 0
                    ? `${draft.vesting.cliffPeriodSeconds} s`
                    : "None"
                }
              />
              {draft.vesting.cliffPeriodSeconds > 0 && (
                <Row
                  label="Cliff grace"
                  value={
                    draft.vesting.cliffGracePeriodSeconds > 0
                      ? `${draft.vesting.cliffGracePeriodSeconds} s`
                      : "Disabled"
                  }
                />
              )}
            </>
          ) : (
            <Row label="Vesting" value="Disabled" />
          )}
        </Section>
      </dl>

      {submitError && (
        <p
          role="alert"
          style={{
            color: "#d32f2f",
            background: "#ffebee",
            border: "1px solid #ef9a9a",
            borderRadius: 4,
            padding: "10px 14px",
            marginBottom: 16,
            fontSize: 14,
          }}
        >
          {submitError}
        </p>
      )}

      <button
        type="button"
        onClick={onSubmit}
        disabled={isSubmitting || totalBps !== 100}
        style={{
          padding: "10px 24px",
          background: totalBps === 100 ? "#1976d2" : "#bdbdbd",
          color: "#fff",
          border: "none",
          borderRadius: 4,
          cursor: totalBps === 100 && !isSubmitting ? "pointer" : "not-allowed",
          fontSize: 15,
          fontWeight: 600,
        }}
      >
        {isSubmitting ? "Submitting…" : "Sign and Create Vault"}
      </button>
    </section>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <>
      <div
        style={{
          gridColumn: "1 / -1",
          fontWeight: 700,
          fontSize: 13,
          textTransform: "uppercase",
          letterSpacing: "0.06em",
          color: "#1976d2",
          marginTop: 16,
          marginBottom: 4,
          borderBottom: "1px solid #e3f2fd",
          paddingBottom: 4,
        }}
      >
        {title}
      </div>
      {children}
    </>
  );
}

function Row({
  label,
  value,
}: {
  label: string;
  value: React.ReactNode;
}) {
  return (
    <>
      <dt style={{ fontWeight: 600, fontSize: 13, color: "#555", paddingBottom: 4 }}>
        {label}
      </dt>
      <dd style={{ margin: 0, fontSize: 14, color: "#222", paddingBottom: 4 }}>{value}</dd>
    </>
  );
}

const dlStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "160px 1fr",
  gap: "2px 16px",
  background: "#fafafa",
  border: "1px solid #e0e0e0",
  borderRadius: 6,
  padding: "12px 16px",
  marginBottom: 24,
};
