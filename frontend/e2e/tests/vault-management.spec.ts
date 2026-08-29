import { test, expect } from "@playwright/test";

/**
 * E2E tests — Dashboard Vault Management (issue #1196)
 *
 * These tests exercise the core vault-management flows against a running
 * TTL-Legacy frontend + backend.  They are skipped automatically when the
 * E2E_SKIP environment variable is set so that CI does not fail when no
 * frontend server is available.
 */

const SKIP = !!process.env.E2E_SKIP;

test.describe("Vault Management", () => {
  // ── Create Vault Flow ────────────────────────────────────────────────────

  test("create vault flow — fills form and asserts vault ID appears", async ({
    page,
  }) => {
    test.skip(SKIP, "E2E_SKIP is set — skipping tests that require a running server");

    // Navigate to the dashboard home page.
    await page.goto("/");

    // Fill in the beneficiary Stellar address.
    await page.fill(
      '[data-testid="beneficiary-address"], input[name="beneficiary"], input[placeholder*="beneficiary" i]',
      "GBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    );

    // Set the check-in interval (e.g. 30 days in seconds).
    await page.fill(
      '[data-testid="check-in-interval"], input[name="checkInInterval"], input[placeholder*="interval" i]',
      "2592000"
    );

    // Submit the create-vault form.
    await page.click(
      '[data-testid="create-vault-submit"], button[type="submit"], button:has-text("Create Vault")'
    );

    // Assert that a vault ID is displayed after successful creation.
    await expect(
      page.locator(
        '[data-testid="vault-id"], .vault-id, [class*="vaultId"], :text-matches("vault.*id", "i")'
      )
    ).toBeVisible();
  });

  // ── Check-In Flow ─────────────────────────────────────────────────────────

  test("check-in flow — clicks check-in button and asserts TTL extended", async ({
    page,
  }) => {
    test.skip(SKIP, "E2E_SKIP is set — skipping tests that require a running server");

    // Navigate directly to a vault detail page.
    // In a real test suite this vault ID would be obtained from a previous
    // test or a test-fixture API call; here we use a placeholder.
    await page.goto("/vault/1");

    // Click the check-in button.
    await page.click(
      '[data-testid="check-in-button"], button:has-text("Check In"), button:has-text("Check-In")'
    );

    // Assert that the confirmation message / TTL-extended indicator is visible.
    await expect(
      page.locator(
        '[data-testid="checkin-confirmation"], .ttl-extended, :text-matches("(ttl extended|checked in|check.in successful)", "i")'
      )
    ).toBeVisible();
  });

  // ── View Vault Status ─────────────────────────────────────────────────────

  test("view vault status — status badge is visible on vault detail page", async ({
    page,
  }) => {
    test.skip(SKIP, "E2E_SKIP is set — skipping tests that require a running server");

    await page.goto("/vault/1");

    // Assert that a status badge (e.g. "Active", "Released", "Expired") is shown.
    await expect(
      page.locator(
        '[data-testid="vault-status-badge"], .status-badge, [class*="statusBadge"], [class*="status"]'
      )
    ).toBeVisible();
  });
});
