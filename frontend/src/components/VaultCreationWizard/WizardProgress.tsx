/**
 * Step progress indicator for the vault creation wizard.
 *
 * Issue #2067.
 */

import React from "react";
import { WIZARD_STEPS, WizardStep } from "./types";

const STEP_LABELS: Record<WizardStep, string> = {
  basic: "Basic Config",
  beneficiaries: "Beneficiaries",
  releaseConditions: "Release Conditions",
  vesting: "Vesting",
  review: "Review & Sign",
};

interface WizardProgressProps {
  currentStep: WizardStep;
  onStepClick?: (step: WizardStep) => void;
}

export function WizardProgress({ currentStep, onStepClick }: WizardProgressProps) {
  const currentIndex = WIZARD_STEPS.indexOf(currentStep);

  return (
    <nav aria-label="Vault creation steps">
      <ol
        style={{
          display: "flex",
          gap: 0,
          listStyle: "none",
          padding: 0,
          margin: "0 0 24px 0",
          borderBottom: "2px solid #e0e0e0",
        }}
      >
        {WIZARD_STEPS.map((step, idx) => {
          const isDone = idx < currentIndex;
          const isActive = idx === currentIndex;
          const isClickable = isDone && onStepClick != null;

          return (
            <li
              key={step}
              style={{
                flex: 1,
                textAlign: "center",
                padding: "8px 4px 10px",
                borderBottom: isActive ? "3px solid #1976d2" : "3px solid transparent",
                marginBottom: -2,
                fontWeight: isActive ? 600 : 400,
                color: isActive ? "#1976d2" : isDone ? "#2e7d32" : "#757575",
                cursor: isClickable ? "pointer" : "default",
                fontSize: 13,
              }}
              aria-current={isActive ? "step" : undefined}
            >
              <button
                type="button"
                disabled={!isClickable}
                onClick={() => isClickable && onStepClick?.(step)}
                style={{
                  background: "none",
                  border: "none",
                  cursor: isClickable ? "pointer" : "default",
                  color: "inherit",
                  fontWeight: "inherit",
                  fontSize: "inherit",
                  padding: 0,
                }}
              >
                <span
                  style={{
                    display: "block",
                    width: 24,
                    height: 24,
                    borderRadius: "50%",
                    background: isActive ? "#1976d2" : isDone ? "#2e7d32" : "#e0e0e0",
                    color: isActive || isDone ? "#fff" : "#757575",
                    lineHeight: "24px",
                    margin: "0 auto 4px",
                    fontSize: 12,
                  }}
                  aria-hidden="true"
                >
                  {isDone ? "✓" : idx + 1}
                </span>
                {STEP_LABELS[step]}
              </button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
