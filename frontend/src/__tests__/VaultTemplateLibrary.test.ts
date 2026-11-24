/**
 * Tests for VaultTemplateLibrary — #2072
 */
import {
  VAULT_TEMPLATE_LIBRARY,
  SimpleSingleBeneficiary,
  FamilyTrust,
  BusinessContinuity,
  VestingGrant,
  getTemplateById,
  cloneTemplate,
} from "../vaultTemplates";

describe("VAULT_TEMPLATE_LIBRARY", () => {
  it("contains exactly four templates", () => {
    expect(VAULT_TEMPLATE_LIBRARY).toHaveLength(4);
  });

  it("contains all four named templates", () => {
    const ids = VAULT_TEMPLATE_LIBRARY.map((t) => t.id);
    expect(ids).toContain("simple-single-beneficiary");
    expect(ids).toContain("family-trust");
    expect(ids).toContain("business-continuity");
    expect(ids).toContain("vesting-grant");
  });

  it("every template has a non-empty name and description", () => {
    for (const tpl of VAULT_TEMPLATE_LIBRARY) {
      expect(tpl.name.length).toBeGreaterThan(0);
      expect(tpl.description.length).toBeGreaterThan(0);
    }
  });

  it("every template has beneficiaries whose BPS sum to 10 000", () => {
    for (const tpl of VAULT_TEMPLATE_LIBRARY) {
      const total = tpl.beneficiaries.reduce((sum, b) => sum + b.bps, 0);
      expect(total).toBe(10_000);
    }
  });

  it("every template has a positive check-in interval", () => {
    for (const tpl of VAULT_TEMPLATE_LIBRARY) {
      expect(tpl.checkInIntervalSeconds).toBeGreaterThan(0);
    }
  });
});

describe("SimpleSingleBeneficiary", () => {
  it("has exactly one beneficiary holding 10 000 bps", () => {
    expect(SimpleSingleBeneficiary.beneficiaries).toHaveLength(1);
    expect(SimpleSingleBeneficiary.beneficiaries[0].bps).toBe(10_000);
  });

  it("does not enable multi-sig", () => {
    expect(SimpleSingleBeneficiary.enableMultiSig).toBe(false);
  });

  it("has no spending limit", () => {
    expect(SimpleSingleBeneficiary.spendingLimitStroops).toBeNull();
  });

  it("uses a 90-day check-in interval", () => {
    expect(SimpleSingleBeneficiary.checkInIntervalSeconds).toBe(90 * 24 * 60 * 60);
  });
});

describe("FamilyTrust", () => {
  it("has exactly three beneficiaries", () => {
    expect(FamilyTrust.beneficiaries).toHaveLength(3);
  });

  it("enables multi-sig", () => {
    expect(FamilyTrust.enableMultiSig).toBe(true);
  });

  it("uses a 180-day check-in interval", () => {
    expect(FamilyTrust.checkInIntervalSeconds).toBe(180 * 24 * 60 * 60);
  });
});

describe("BusinessContinuity", () => {
  it("splits evenly between two beneficiaries", () => {
    expect(BusinessContinuity.beneficiaries).toHaveLength(2);
    expect(BusinessContinuity.beneficiaries[0].bps).toBe(5_000);
    expect(BusinessContinuity.beneficiaries[1].bps).toBe(5_000);
  });

  it("enables multi-sig", () => {
    expect(BusinessContinuity.enableMultiSig).toBe(true);
  });

  it("has a positive spending limit", () => {
    expect(BusinessContinuity.spendingLimitStroops).not.toBeNull();
    expect(BusinessContinuity.spendingLimitStroops!).toBeGreaterThan(0);
  });
});

describe("VestingGrant", () => {
  it("has exactly one beneficiary", () => {
    expect(VestingGrant.beneficiaries).toHaveLength(1);
  });

  it("does not enable multi-sig", () => {
    expect(VestingGrant.enableMultiSig).toBe(false);
  });

  it("has a positive spending limit", () => {
    expect(VestingGrant.spendingLimitStroops).not.toBeNull();
    expect(VestingGrant.spendingLimitStroops!).toBeGreaterThan(0);
  });

  it("uses a 365-day check-in interval", () => {
    expect(VestingGrant.checkInIntervalSeconds).toBe(365 * 24 * 60 * 60);
  });
});

describe("getTemplateById", () => {
  it("returns the correct template for a known id", () => {
    const tpl = getTemplateById("vesting-grant");
    expect(tpl).toBeDefined();
    expect(tpl!.id).toBe("vesting-grant");
  });

  it("returns undefined for an unknown id", () => {
    expect(getTemplateById("does-not-exist")).toBeUndefined();
  });
});

describe("cloneTemplate", () => {
  it("returns a structurally equal but reference-independent copy", () => {
    const original = FamilyTrust;
    const copy = cloneTemplate(original);

    expect(copy).toEqual(original);
    expect(copy).not.toBe(original);
    expect(copy.beneficiaries).not.toBe(original.beneficiaries);
  });

  it("mutations to the clone do not affect the original", () => {
    const copy = cloneTemplate(SimpleSingleBeneficiary);
    copy.beneficiaries[0].addressPlaceholder = "0xMUTATED";
    expect(SimpleSingleBeneficiary.beneficiaries[0].addressPlaceholder).not.toBe("0xMUTATED");
  });
});
