import { test, expect } from "@playwright/test";

/**
 * E2E tests — Check-In Flow (issue #1548)
 *
 * These tests exercise the check-in flow against a running TTL-Legacy
 * frontend + backend with a mocked wallet.  Set E2E_SKIP=1 to skip them
 * when no server is available (e.g. during unit-test CI jobs).
 */

const SKIP = !!process.env.E2E_SKIP;

test.describe("Check-In Flow", () => {
  // ── Successful check-in ────────────────────────────────────────────────

  test("check-in succeeds — TTL extended confirmation is shown", async ({
    page,
  }) => {
    test.skip(SKIP, "E2E_SKIP is set — skipping tests that require a running server");

    // Intercept the check-in API call and return a successful response so the
    // test does not depend on a live Stellar node.
    await page.route("**/api/vaults/*/check-in", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ success: true, ttlExtended: true }),
      })
    );

    await page.goto("/vault/1");

    // Click the check-in button.
    await page.click(
      '[data-testid="check-in-button"], button:has-text("Check In"), button:has-text("Check-In")'
    );

    // Confirmation / TTL-extended indicator must become visible.
    await expect(
      page.locator(
        '[data-testid="checkin-confirmation"], .ttl-extended, :text-matches("(ttl extended|checked in|check.in successful)", "i")'
      )
    ).toBeVisible();
  });

  // ── Failed transaction ─────────────────────────────────────────────────

  test("check-in fails — error message is shown when transaction is rejected", async ({
    page,
  }) => {
    test.skip(SKIP, "E2E_SKIP is set — skipping tests that require a running server");

    // Mock the wallet to reject the transaction (simulates user cancellation or
    // a node-level rejection).
    await page.route("**/api/vaults/*/check-in", (route) =>
      route.fulfill({
        status: 400,
        contentType: "application/json",
        body: JSON.stringify({ success: false, error: "Transaction rejected" }),
      })
    );

    await page.goto("/vault/1");

    await page.click(
      '[data-testid="check-in-button"], button:has-text("Check In"), button:has-text("Check-In")'
    );

    // An error / failure indicator must become visible.
    await expect(
      page.locator(
        '[data-testid="checkin-error"], .checkin-error, :text-matches("(failed|error|rejected)", "i")'
      )
    ).toBeVisible();
  });
});
