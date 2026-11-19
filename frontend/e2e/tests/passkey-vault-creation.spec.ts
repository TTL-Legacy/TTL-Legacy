import { test, expect } from "@playwright/test";

/**
 * E2E test — passkey registration and vault creation on testnet (issue #1765)
 *
 * Registers a WebAuthn credential through a Chromium virtual authenticator
 * (the browser credential API is mocked at the CDP layer, so no hardware key
 * is needed), creates a vault with a 1-hour check-in interval, and asserts the
 * new vault appears on the dashboard.
 *
 * Skipped automatically when E2E_SKIP is set so CI does not fail when no
 * frontend + testnet backend is running.
 */

const SKIP = !!process.env.E2E_SKIP;

const BENEFICIARY_ADDRESS =
  "GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
const ONE_HOUR_SECONDS = "3600";

test.describe("Passkey and vault creation (testnet)", () => {
  test("registers a passkey and creates a 1-hour vault visible on the dashboard", async ({
    page,
  }) => {
    test.skip(SKIP, "E2E_SKIP is set — skipping tests that require a running server");

    // Attach a virtual CTAP2 authenticator that supports resident keys and
    // user verification, so navigator.credentials.create() resolves without
    // a physical device.
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("WebAuthn.enable");
    const { authenticatorId } = await cdp.send("WebAuthn.addVirtualAuthenticator", {
      options: {
        protocol: "ctap2",
        transport: "internal",
        hasResidentKey: true,
        hasUserVerification: true,
        isUserVerified: true,
        automaticPresenceSimulation: true,
      },
    });

    // ── Passkey registration ────────────────────────────────────────────────
    await page.goto("/");

    await page.click(
      '[data-testid="register-passkey"], button:has-text("Register Passkey"), button:has-text("Add Passkey")'
    );

    await page.fill(
      '[data-testid="passkey-name-input"], input[name="passkeyName"], input[placeholder*="passkey" i]',
      "e2e-testnet-passkey"
    );

    await page.click(
      '[data-testid="register-btn"], button[type="submit"]:has-text("Register"), button:has-text("Register")'
    );

    // The registered credential should be listed, and no error shown.
    await expect(
      page.locator(
        '[data-testid="registered-passkeys"] li, [data-testid^="passkey-"], :text-matches("e2e-testnet-passkey", "i")'
      ).first()
    ).toBeVisible();
    await expect(
      page.locator('[data-testid="error-message"], [role="alert"]')
    ).toHaveCount(0);

    // Confirm the virtual authenticator actually stored the credential.
    const { credentials } = await cdp.send("WebAuthn.getCredentials", {
      authenticatorId,
    });
    expect(credentials.length).toBeGreaterThan(0);

    // ── Vault creation ──────────────────────────────────────────────────────
    await page.goto("/");

    await page.fill(
      '[data-testid="beneficiary-address"], input[name="beneficiary"], input[placeholder*="beneficiary" i]',
      BENEFICIARY_ADDRESS
    );

    await page.fill(
      '[data-testid="check-in-interval"], input[name="checkInInterval"], input[placeholder*="interval" i]',
      ONE_HOUR_SECONDS
    );

    await page.click(
      '[data-testid="create-vault-submit"], button[type="submit"], button:has-text("Create Vault")'
    );

    // ── Dashboard assertion ─────────────────────────────────────────────────
    await page.goto("/dashboard");

    const vaultCard = page.locator(
      '[data-testid^="vault-card-"], [data-testid^="vault-"], .vault-card'
    );
    await expect(vaultCard.first()).toBeVisible();
    await expect(
      page.locator(
        ':text-matches("(1 ?h(our)?|3600)", "i")'
      ).first()
    ).toBeVisible();
  });
});
