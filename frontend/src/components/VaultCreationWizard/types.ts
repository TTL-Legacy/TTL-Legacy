/**
 * Shared types for the vault creation wizard.
 *
 * Issue #2067 — multi-step vault creation with draft persistence.
 */

/** Step identifiers for the five-step wizard. */
export type WizardStep =
  | "basic"
  | "beneficiaries"
  | "releaseConditions"
  | "vesting"
  | "review";

export const WIZARD_STEPS: WizardStep[] = [
  "basic",
  "beneficiaries",
  "releaseConditions",
  "vesting",
  "review",
];

export interface BeneficiaryEntry {
  address: string;
  /** Percentage share (1–100). All entries must sum to 100. */
  percentage: number;
}

export type ReleaseCondition = "ttl_expiry" | "owner_initiated" | "oracle";

export interface VestingConfig {
  enabled: boolean;
  /** Unix timestamp (seconds) when the first installment becomes claimable. */
  startTime: number;
  /** Seconds between consecutive installments. */
  intervalSeconds: number;
  /** Total number of installments. */
  numInstallments: number;
  /** Total amount to vest in stroops. */
  totalAmountStroops: string;
  /** Cliff period in seconds (0 = no cliff). */
  cliffPeriodSeconds: number;
  /** Cliff grace period in seconds (Issue #2065, 0 = disabled). */
  cliffGracePeriodSeconds: number;
}

/** The full wizard draft state persisted to localStorage. */
export interface WizardDraft {
  /** Wizard step the user was on when they last saved. */
  currentStep: WizardStep;

  // ── Step 1: Basic config ──────────────────────────────────────────────
  label: string;
  /** Check-in interval in seconds. */
  checkInIntervalSeconds: number;
  tokenAddress: string;
  /** Max deposit amount in stroops (empty = no cap). */
  maxDepositAmountStroops: string;

  // ── Step 2: Beneficiaries ─────────────────────────────────────────────
  beneficiaries: BeneficiaryEntry[];

  // ── Step 3: Release conditions ────────────────────────────────────────
  releaseCondition: ReleaseCondition;
  /** Oracle address (only used when releaseCondition === "oracle"). */
  oracleAddress: string;

  // ── Step 4: Vesting ───────────────────────────────────────────────────
  vesting: VestingConfig;

  /** ISO 8601 string — when the draft was last saved. */
  savedAt: string;
}

export const DEFAULT_VESTING: VestingConfig = {
  enabled: false,
  startTime: 0,
  intervalSeconds: 2592000, // 30 days
  numInstallments: 12,
  totalAmountStroops: "",
  cliffPeriodSeconds: 0,
  cliffGracePeriodSeconds: 0,
};

export const DEFAULT_DRAFT: WizardDraft = {
  currentStep: "basic",
  label: "",
  checkInIntervalSeconds: 86400, // 24 hours
  tokenAddress: "",
  maxDepositAmountStroops: "",
  beneficiaries: [{ address: "", percentage: 100 }],
  releaseCondition: "ttl_expiry",
  oracleAddress: "",
  vesting: DEFAULT_VESTING,
  savedAt: new Date().toISOString(),
};
