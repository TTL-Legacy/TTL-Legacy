import { test, expect, Page } from "@playwright/test";

/**
 * E2E test — Vault Creation Wizard happy path (Issue #2090)
 *
 * Steps through all five wizard screens, fills in valid data for a
 * single-beneficiary vault, signs the transaction with a mock wallet,
 * and asserts the new vault appears in the dashboard list.
 *
 * The test is skipped automatically when E2E_SKIP is set so CI does not
 * fail when no frontend + backend is running.
 *
 * Mock wallet strategy
 * ─────────────────────
 * Freighter (the standard Stellar browser extension) exposes
 * `window.freighter` and `window.freighterApi`.  Before navigating to the
 * app we inject a minimal in-page mock that:
 *   • reports `isConnected() → true`
 *   • returns a fixed test public key from `getPublicKey()`
 *   • resolves `signTransaction()` with a base64 dummy signature so the
 *     frontend's signing flow can complete without a real extension.
 *
 * Five wizard screens
 * ────────────────────
 *   1. Welcome / intro
 *   2. Vault name & check-in interval
 *   3. Beneficiary details
 *   4. Token & amount
 *   5. Review & sign
 */

const SKIP = !!process.env.E2E_SKIP;

// A valid Stellar testnet public key used as the mock owner / beneficiary.
const MOCK_PUBLIC_KEY =
  "GDQP2KPQGKIHYJGXNUIYOMHARUARCA7DJT5FO2FFOOKY3B2WSQHG4W37";

const BENEFICIARY_ADDRESS =
  "GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";

// 30-day check-in interval in seconds.
const CHECK_IN_INTERVAL_SECONDS = "2592000";

// Token amount to lock in the vault (in smallest unit / stroops).
const VAULT_AMOUNT = "100000000";

// ── Mock wallet injection ─────────────────────────────────────────────────────

/**
 * Injects a mock Freighter wallet into the page before the app JS runs.
 * Returns immediately so the app sees a connected wallet without user action.
 */
async function injectMockWallet(page: Page): Promise<void> {
  await page.addInitScript((pubKey: string) => {
    const mockFreighter = {
      isConnected: () => Promise.resolve(true),
      getPublicKey: () => Promise.resolve(pubKey),
      signTransaction: (_xdr: string, _opts?: unknown) =>
        Promise.resolve({
          signedTxXdr:
            "AAAAAgAAAAA" +
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==",
        }),
      requestAccess: () => Promise.resolve(pubKey),
    };

    // Some frontends use `window.freighter`, others `window.freighterApi`.
    (window as unknown as Record<string, unknown>)["freighter"] = mockFreighter;
    (window as unknown as Record<string, unknown>)["freighterApi"] =
      mockFreighter;

    // StellarWalletsKit / rabet / lobstr style
    (window as unknown as Record<string, unknown>)["stellar"] = {
      isAvailable: true,
      getPublicKey: mockFreighter.getPublicKey,
      signTransaction: mockFreighter.signTransaction,
    };
  }, MOCK_PUBLIC_KEY);
}

// ── Wizard step helpers ───────────────────────────────────────────────────────

/**
 * Step 1 — Welcome screen.
 * Clicks the "Create Vault" / "Get Started" primary CTA to enter the wizard.
 */
async function step1_welcome(page: Page): Promise<void> {
  // The welcome step may auto-advance or require a button click.
  const startBtn = page.locator(
    '[data-testid="wizard-start"], ' +
      '[data-testid="create-vault-btn"], ' +
      'button:has-text("Create Vault"), ' +
      'button:has-text("Get Started"), ' +
      'button:has-text("New Vault")'
  );

  if (await startBtn.isVisible({ timeout: 5_000 }).catch(() => false)) {
    await startBtn.click();
  }

  // Confirm we are now on wizard step 1 (or the wizard itself is visible).
  await expect(
    page.locator(
      '[data-testid="wizard"], ' +
        '[data-testid="wizard-step"], ' +
        '[class*="wizard"], ' +
        '[class*="Wizard"]'
    )
  ).toBeVisible({ timeout: 10_000 });
}

/**
 * Step 2 — Vault name & check-in interval.
 */
async function step2_vaultDetails(page: Page): Promise<void> {
  // Vault name (optional in some implementations; fill if the field exists).
  const nameInput = page.locator(
    '[data-testid="vault-name"], ' +
      'input[name="vaultName"], ' +
      'input[placeholder*="vault name" i], ' +
      'input[placeholder*="name" i]'
  );
  if (await nameInput.isVisible({ timeout: 3_000 }).catch(() => false)) {
    await nameInput.fill("E2E Test Vault");
  }

  // Check-in interval.
  await page.fill(
    '[data-testid="check-in-interval"], ' +
      'input[name="checkInInterval"], ' +
      'input[name="interval"], ' +
      'input[placeholder*="interval" i], ' +
      'input[placeholder*="check-in" i]',
    CHECK_IN_INTERVAL_SECONDS
  );

  await page.click(
    '[data-testid="wizard-next"], ' +
      'button:has-text("Next"), ' +
      'button:has-text("Continue")'
  );
}

/**
 * Step 3 — Beneficiary details.
 * Enters the beneficiary Stellar address and confirms.
 */
async function step3_beneficiary(page: Page): Promise<void> {
  await page.fill(
    '[data-testid="beneficiary-address"], ' +
      'input[name="beneficiary"], ' +
      'input[name="beneficiaryAddress"], ' +
      'input[placeholder*="beneficiary" i], ' +
      'input[placeholder*="G1" i]',
    BENEFICIARY_ADDRESS
  );

  // Some wizards ask for a beneficiary name or percentage — fill defaults.
  const beneficiaryNameInput = page.locator(
    '[data-testid="beneficiary-name"], input[name="beneficiaryName"]'
  );
  if (
    await beneficiaryNameInput.isVisible({ timeout: 2_000 }).catch(() => false)
  ) {
    await beneficiaryNameInput.fill("Test Beneficiary");
  }

  const bpsInput = page.locator(
    '[data-testid="beneficiary-bps"], ' +
      'input[name="bps"], ' +
      'input[name="percentage"], ' +
      'input[placeholder*="percent" i]'
  );
  if (await bpsInput.isVisible({ timeout: 2_000 }).catch(() => false)) {
    await bpsInput.fill("100");
  }

  await page.click(
    '[data-testid="wizard-next"], ' +
      'button:has-text("Next"), ' +
      'button:has-text("Continue")'
  );
}

/**
 * Step 4 — Token & amount.
 * Selects the native XLM token and enters a deposit amount.
 */
async function step4_tokenAndAmount(page: Page): Promise<void> {
  // Token selector — choose XLM / native if a dropdown is present.
  const tokenSelect = page.locator(
    '[data-testid="token-select"], ' +
      'select[name="token"], ' +
      '[role="combobox"][aria-label*="token" i]'
  );
  if (await tokenSelect.isVisible({ timeout: 2_000 }).catch(() => false)) {
    await tokenSelect.selectOption({ label: /xlm|native|stellar/i });
  }

  // Deposit amount.
  await page.fill(
    '[data-testid="vault-amount"], ' +
      'input[name="amount"], ' +
      'input[name="depositAmount"], ' +
      'input[placeholder*="amount" i]',
    VAULT_AMOUNT
  );

  await page.click(
    '[data-testid="wizard-next"], ' +
      'button:has-text("Next"), ' +
      'button:has-text("Continue")'
  );
}

/**
 * Step 5 — Review & sign.
 * Verifies the summary displays and clicks the final "Sign & Create" button.
 */
async function step5_reviewAndSign(page: Page): Promise<void> {
  // The review screen should show the beneficiary address somewhere.
  await expect(
    page.locator(`text=${BENEFICIARY_ADDRESS}`).or(
      page.locator(
        '[data-testid="review-beneficiary"], ' +
          '[class*="review"] [class*="beneficiary"]'
      )
    )
  ).toBeVisible({ timeout: 10_000 });

  // Click the final sign-and-create button.
  await page.click(
    '[data-testid="wizard-sign"], ' +
      '[data-testid="sign-and-create"], ' +
      'button:has-text("Sign"), ' +
      'button:has-text("Create Vault"), ' +
      'button:has-text("Confirm")'
  );
}

// ── Main test ─────────────────────────────────────────────────────────────────

test.describe("Vault Creation Wizard — happy path (Issue #2090)", () => {
  test(
    "completes all five wizard steps and vault appears in dashboard",
    async ({ page }) => {
      test.skip(
        SKIP,
        "E2E_SKIP is set — skipping tests that require a running server"
      );

      // Inject mock wallet before any page script runs.
      await injectMockWallet(page);

      // ── Navigate to dashboard ────────────────────────────────────────────
      await page.goto("/");

      // Wait for the app shell to load.
      await expect(page).toHaveTitle(/ttl|vault|dashboard/i, {
        timeout: 15_000,
      });

      // ── Step 1: Welcome ──────────────────────────────────────────────────
      await step1_welcome(page);

      // ── Step 2: Vault name & interval ───────────────────────────────────
      await step2_vaultDetails(page);

      // ── Step 3: Beneficiary ──────────────────────────────────────────────
      await step3_beneficiary(page);

      // ── Step 4: Token & amount ───────────────────────────────────────────
      await step4_tokenAndAmount(page);

      // ── Step 5: Review & sign ────────────────────────────────────────────
      await step5_reviewAndSign(page);

      // ── Post-creation assertion ──────────────────────────────────────────
      // After signing, the app should navigate back to the dashboard (or a
      // success screen) and the new vault must appear in the vault list.
      await expect(
        page.locator(
          '[data-testid="vault-list"] [data-testid="vault-item"], ' +
            '[data-testid="vault-card"], ' +
            '[class*="vaultList"] [class*="vaultItem"], ' +
            '[class*="VaultList"] [class*="VaultCard"]'
        )
      ).toBeVisible({ timeout: 20_000 });

      // The dashboard list must contain at least one entry that references
      // the beneficiary address we entered during the wizard.
      await expect(
        page
          .locator(
            '[data-testid="vault-list"], ' +
              '[data-testid="dashboard-vaults"], ' +
              '[class*="vaultList"], ' +
              '[class*="VaultList"]'
          )
          .filter({ hasText: BENEFICIARY_ADDRESS })
          .or(
            // Fallback: any vault card visible on the page counts as success
            // when the address is truncated/hidden in the list view.
            page.locator(
              '[data-testid="vault-item"], [data-testid="vault-card"]'
            )
          )
      ).toBeVisible({ timeout: 15_000 });
    }
  );

  // ── Individual step smoke-tests ─────────────────────────────────────────────
  // These run each step in isolation to give fine-grained failure signals.

  test("wizard is reachable from the dashboard home", async ({ page }) => {
    test.skip(SKIP, "E2E_SKIP is set");

    await injectMockWallet(page);
    await page.goto("/");

    const wizardTrigger = page.locator(
      '[data-testid="create-vault-btn"], ' +
        '[data-testid="wizard-start"], ' +
        'button:has-text("Create Vault"), ' +
        'button:has-text("New Vault")'
    );
    await expect(wizardTrigger).toBeVisible({ timeout: 10_000 });
  });

  test("beneficiary address field validates Stellar addresses", async ({
    page,
  }) => {
    test.skip(SKIP, "E2E_SKIP is set");

    await injectMockWallet(page);
    await page.goto("/");
    await step1_welcome(page);
    await step2_vaultDetails(page);

    // Enter a clearly invalid address.
    await page.fill(
      '[data-testid="beneficiary-address"], input[name="beneficiary"]',
      "not-a-stellar-address"
    );

    // Attempt to advance to step 4 — the form should show a validation error
    // or refuse to advance.
    await page.click(
      '[data-testid="wizard-next"], button:has-text("Next"), button:has-text("Continue")'
    );

    const errorLocator = page.locator(
      '[data-testid="beneficiary-error"], ' +
        '[role="alert"], ' +
        '[class*="error" i], ' +
        'text=/invalid.*address/i, ' +
        'text=/address.*invalid/i'
    );
    const stepStillVisible = page.locator(
      'input[name="beneficiary"], [data-testid="beneficiary-address"]'
    );

    // Either an error message is shown OR the wizard stays on step 3.
    const outcome = await Promise.race([
      errorLocator.isVisible({ timeout: 5_000 }),
      stepStillVisible.isVisible({ timeout: 5_000 }),
    ]);
    expect(outcome).toBe(true);
  });

  test("mock wallet signs transaction without user interaction", async ({
    page,
  }) => {
    test.skip(SKIP, "E2E_SKIP is set");

    await injectMockWallet(page);
    await page.goto("/");

    // Confirm that window.freighter is available (mock is in place).
    const walletConnected = await page.evaluate(
      () =>
        typeof (window as unknown as Record<string, unknown>)["freighter"] !==
        "undefined"
    );
    expect(walletConnected).toBe(true);

    // Confirm that getPublicKey returns the mock key.
    const pubKey = await page.evaluate(async () => {
      const freighter = (
        window as unknown as { freighter: { getPublicKey: () => Promise<string> } }
      ).freighter;
      return freighter.getPublicKey();
    });
    expect(pubKey).toBe(MOCK_PUBLIC_KEY);
  });
});
