/**
 * Multi-step vault creation wizard.
 *
 * Replaces the single-page creation form with a five-step flow:
 *   1. Basic config
 *   2. Beneficiaries
 *   3. Release conditions
 *   4. Vesting
 *   5. Review & sign
 *
 * Draft state is persisted to localStorage so users can resume an incomplete
 * wizard after navigating away.
 *
 * Issue #2067.
 */

import React, { useCallback, useState } from "react";
import { WIZARD_STEPS, WizardDraft, WizardStep } from "./types";
import { useWizardDraft } from "./useWizardDraft";
import { WizardProgress } from "./WizardProgress";
import { StepBasicConfig } from "./steps/StepBasicConfig";
import { StepBeneficiaries } from "./steps/StepBeneficiaries";
import { StepReleaseConditions } from "./steps/StepReleaseConditions";
import { StepVesting } from "./steps/StepVesting";
import { StepReviewAndSign } from "./steps/StepReviewAndSign";

export interface VaultCreationWizardProps {
  /**
   * Called when the user reaches step 5 and clicks "Sign and Create Vault".
   * Receives the final draft state. The host is responsible for building and
   * submitting the Soroban transaction.
   */
  onSubmit: (draft: WizardDraft) => Promise<void>;
  /** Called when the wizard completes or the user explicitly cancels. */
  onCancel?: () => void;
  className?: string;
}

export function VaultCreationWizard({
  onSubmit,
  onCancel,
  className,
}: VaultCreationWizardProps) {
  const { draft, hasSavedDraft, updateDraft, setStep, discardDraft } =
    useWizardDraft();

  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | undefined>();
  const [showDraftBanner, setShowDraftBanner] = useState(hasSavedDraft);

  const currentStepIndex = WIZARD_STEPS.indexOf(draft.currentStep);

  const goNext = useCallback(() => {
    const nextIndex = currentStepIndex + 1;
    if (nextIndex < WIZARD_STEPS.length) {
      setStep(WIZARD_STEPS[nextIndex]);
    }
  }, [currentStepIndex, setStep]);

  const goBack = useCallback(() => {
    const prevIndex = currentStepIndex - 1;
    if (prevIndex >= 0) {
      setStep(WIZARD_STEPS[prevIndex]);
    }
  }, [currentStepIndex, setStep]);

  const handleSubmit = useCallback(async () => {
    setIsSubmitting(true);
    setSubmitError(undefined);
    try {
      await onSubmit(draft);
      discardDraft();
    } catch (err) {
      setSubmitError(err instanceof Error ? err.message : "An error occurred.");
    } finally {
      setIsSubmitting(false);
    }
  }, [draft, discardDraft, onSubmit]);

  function renderStep() {
    switch (draft.currentStep) {
      case "basic":
        return <StepBasicConfig draft={draft} onChange={updateDraft} />;
      case "beneficiaries":
        return <StepBeneficiaries draft={draft} onChange={updateDraft} />;
      case "releaseConditions":
        return <StepReleaseConditions draft={draft} onChange={updateDraft} />;
      case "vesting":
        return <StepVesting draft={draft} onChange={updateDraft} />;
      case "review":
        return (
          <StepReviewAndSign
            draft={draft}
            onSubmit={handleSubmit}
            isSubmitting={isSubmitting}
            submitError={submitError}
          />
        );
    }
  }

  const isFirstStep = currentStepIndex === 0;
  const isLastStep = draft.currentStep === "review";

  return (
    <div
      className={className}
      data-testid="vault-creation-wizard"
      style={{ maxWidth: 680, margin: "0 auto", fontFamily: "system-ui, sans-serif" }}
    >
      {/* Draft resume banner */}
      {showDraftBanner && (
        <div
          role="status"
          style={{
            background: "#fff8e1",
            border: "1px solid #ffe082",
            borderRadius: 6,
            padding: "10px 14px",
            marginBottom: 16,
            fontSize: 14,
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 12,
          }}
        >
          <span>You have an unsaved vault creation draft. Resume or start fresh?</span>
          <div style={{ display: "flex", gap: 8 }}>
            <button
              type="button"
              onClick={() => setShowDraftBanner(false)}
              style={resumeBtnStyle}
            >
              Resume
            </button>
            <button
              type="button"
              onClick={() => {
                discardDraft();
                setShowDraftBanner(false);
              }}
              style={discardBtnStyle}
            >
              Discard
            </button>
          </div>
        </div>
      )}

      <WizardProgress
        currentStep={draft.currentStep}
        onStepClick={(step: WizardStep) => {
          const targetIdx = WIZARD_STEPS.indexOf(step);
          // Only allow navigating back to completed steps.
          if (targetIdx < currentStepIndex) {
            setStep(step);
          }
        }}
      />

      <div style={{ padding: "0 4px" }}>{renderStep()}</div>

      {/* Navigation buttons — not shown on the review step (it has its own submit). */}
      {!isLastStep && (
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            marginTop: 32,
            paddingTop: 16,
            borderTop: "1px solid #e0e0e0",
          }}
        >
          <div>
            {!isFirstStep && (
              <button type="button" onClick={goBack} style={secondaryBtnStyle}>
                ← Back
              </button>
            )}
            {onCancel && (
              <button
                type="button"
                onClick={onCancel}
                style={{ ...secondaryBtnStyle, marginLeft: 8, color: "#757575" }}
              >
                Cancel
              </button>
            )}
          </div>

          <button type="button" onClick={goNext} style={primaryBtnStyle}>
            Next →
          </button>
        </div>
      )}
    </div>
  );
}

export default VaultCreationWizard;

// ── Button styles ─────────────────────────────────────────────────────────────

const primaryBtnStyle: React.CSSProperties = {
  padding: "9px 22px",
  background: "#1976d2",
  color: "#fff",
  border: "none",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: 14,
  fontWeight: 600,
};

const secondaryBtnStyle: React.CSSProperties = {
  padding: "9px 22px",
  background: "#fff",
  color: "#1976d2",
  border: "1px solid #1976d2",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: 14,
};

const resumeBtnStyle: React.CSSProperties = {
  padding: "5px 12px",
  background: "#1976d2",
  color: "#fff",
  border: "none",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: 13,
};

const discardBtnStyle: React.CSSProperties = {
  padding: "5px 12px",
  background: "#fff",
  color: "#d32f2f",
  border: "1px solid #ef9a9a",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: 13,
};
