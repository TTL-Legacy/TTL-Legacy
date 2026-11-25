/**
 * VaultTemplateLibrary — #2072
 *
 * Predefined vault configurations for common use cases.
 * Each template supplies a full set of creation-wizard defaults that the user
 * can inspect and customise before saving.
 */

/** A single beneficiary entry as used by the creation wizard. */
export interface TemplateBeneficiary {
  /** Placeholder label shown in the UI before the user fills in a real address. */
  addressPlaceholder: string;
  /** Allocation in basis points (10 000 = 100 %). */
  bps: number;
}

/** All fields that can be pre-filled by a template. */
export interface VaultTemplateConfig {
  /** Machine-readable identifier for the template. */
  id: string;
  /** Human-readable display name shown in the template picker. */
  name: string;
  /** One-line description of the intended use case. */
  description: string;
  /**
   * Check-in interval in seconds.
   * The wizard converts this to a human-friendly label (days / weeks).
   */
  checkInIntervalSeconds: number;
  /** Pre-populated beneficiary rows.  Users replace the placeholder labels with real addresses. */
  beneficiaries: TemplateBeneficiary[];
  /**
   * Short metadata label pre-filled in the "Vault label" field.
   * Users may overwrite it freely.
   */
  metadataLabel: string;
  /**
   * Whether the multi-sig withdrawal approval threshold should be enabled by
   * default (true for templates that benefit from extra signing requirements).
   */
  enableMultiSig: boolean;
  /**
   * Optional spending limit in stroops (1 XLM = 10 000 000 stroops).
   * `null` means no limit is pre-configured.
   */
  spendingLimitStroops: number | null;
}

// ---------------------------------------------------------------------------
// Template definitions
// ---------------------------------------------------------------------------

/**
 * Simple single-beneficiary vault — #2072
 *
 * The most basic vault configuration: a single heir receives 100 % of the
 * balance when the TTL expires.  A 90-day check-in interval is a sensible
 * default for individuals who want a low-maintenance safety net.
 */
export const SimpleSingleBeneficiary: VaultTemplateConfig = {
  id: "simple-single-beneficiary",
  name: "Simple Single Beneficiary",
  description:
    "One beneficiary receives the full vault balance when the TTL expires. " +
    "Best for individuals designating a single heir.",
  checkInIntervalSeconds: 90 * 24 * 60 * 60, // 90 days
  beneficiaries: [
    { addressPlaceholder: "Beneficiary address", bps: 10_000 },
  ],
  metadataLabel: "Personal vault",
  enableMultiSig: false,
  spendingLimitStroops: null,
};

/**
 * Family trust vault — #2072
 *
 * Splits the balance equally among three family members.  The 180-day check-in
 * interval suits a long-term custodial arrangement; multi-sig is enabled so
 * that withdrawals require additional sign-off during the owner's lifetime.
 */
export const FamilyTrust: VaultTemplateConfig = {
  id: "family-trust",
  name: "Family Trust",
  description:
    "Distributes assets equally among three family members when the TTL expires. " +
    "Multi-sig withdrawals protect funds during the owner's lifetime.",
  checkInIntervalSeconds: 180 * 24 * 60 * 60, // 180 days
  beneficiaries: [
    { addressPlaceholder: "Family member 1", bps: 3_334 },
    { addressPlaceholder: "Family member 2", bps: 3_333 },
    { addressPlaceholder: "Family member 3", bps: 3_333 },
  ],
  metadataLabel: "Family trust",
  enableMultiSig: true,
  spendingLimitStroops: null,
};

/**
 * Business continuity vault — #2072
 *
 * Designed for small businesses where two partners share operational control.
 * A 30-day check-in interval keeps the vault active during normal business
 * operations; a spending limit caps per-withdrawal exposure.
 */
export const BusinessContinuity: VaultTemplateConfig = {
  id: "business-continuity",
  name: "Business Continuity",
  description:
    "Two partners share the vault balance 50/50. " +
    "A spending limit and multi-sig protect against unilateral large withdrawals.",
  checkInIntervalSeconds: 30 * 24 * 60 * 60, // 30 days
  beneficiaries: [
    { addressPlaceholder: "Business partner 1", bps: 5_000 },
    { addressPlaceholder: "Business partner 2", bps: 5_000 },
  ],
  metadataLabel: "Business continuity",
  enableMultiSig: true,
  // 50 000 XLM spending limit expressed in stroops
  spendingLimitStroops: 50_000 * 10_000_000,
};

/**
 * Vesting grant vault — #2072
 *
 * Tailored for token grants that vest to a single recipient over time.  The
 * 365-day check-in interval mirrors a typical annual vesting cliff; the
 * spending limit enforces monthly grant disbursements at a sustainable cadence.
 */
export const VestingGrant: VaultTemplateConfig = {
  id: "vesting-grant",
  name: "Vesting Grant",
  description:
    "Single recipient vesting vault with an annual check-in interval and a " +
    "per-withdrawal spending cap that mirrors a monthly grant disbursement.",
  checkInIntervalSeconds: 365 * 24 * 60 * 60, // 365 days
  beneficiaries: [
    { addressPlaceholder: "Grant recipient", bps: 10_000 },
  ],
  metadataLabel: "Vesting grant",
  enableMultiSig: false,
  // 10 000 XLM monthly cap expressed in stroops
  spendingLimitStroops: 10_000 * 10_000_000,
};

// ---------------------------------------------------------------------------
// Library export
// ---------------------------------------------------------------------------

/** All built-in templates, ordered for display in the template picker. */
export const VAULT_TEMPLATE_LIBRARY: VaultTemplateConfig[] = [
  SimpleSingleBeneficiary,
  FamilyTrust,
  BusinessContinuity,
  VestingGrant,
];

/**
 * Returns the template matching `id`, or `undefined` when no template matches.
 *
 * @example
 * const tpl = getTemplateById("family-trust");
 */
export function getTemplateById(id: string): VaultTemplateConfig | undefined {
  return VAULT_TEMPLATE_LIBRARY.find((t) => t.id === id);
}

/**
 * Returns a deep copy of the given template so the caller can mutate the
 * result (e.g. replace placeholder addresses) without affecting the library
 * originals.
 */
export function cloneTemplate(template: VaultTemplateConfig): VaultTemplateConfig {
  return {
    ...template,
    beneficiaries: template.beneficiaries.map((b) => ({ ...b })),
  };
}
