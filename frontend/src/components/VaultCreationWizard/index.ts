/**
 * Public exports for the vault creation wizard.
 *
 * Issue #2067.
 */

export { VaultCreationWizard } from "./VaultCreationWizard";
export type { VaultCreationWizardProps } from "./VaultCreationWizard";
export { useWizardDraft } from "./useWizardDraft";
export type { UseWizardDraftReturn } from "./useWizardDraft";
export type {
  WizardDraft,
  WizardStep,
  BeneficiaryEntry,
  ReleaseCondition,
  VestingConfig,
} from "./types";
export { WIZARD_STEPS, DEFAULT_DRAFT, DEFAULT_VESTING } from "./types";
