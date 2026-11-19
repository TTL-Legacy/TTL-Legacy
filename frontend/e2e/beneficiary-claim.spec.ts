import { test, expect } from '@playwright/test';

test.describe('Beneficiary claim', () => {
  test('beneficiary can claim from a claimable vault', async ({ page }) => {
    await page.goto('/beneficiary');

    const claimableVault = page.getByTestId('claimable-vault').first();
    await expect(claimableVault).toBeVisible();

    await claimableVault.getByRole('button', { name: /claim/i }).click();

    const confirmButton = page.getByRole('button', { name: /confirm/i });
    if (await confirmButton.isVisible().catch(() => false)) {
      await confirmButton.click();
    }

    await expect(page.getByText(/claim (submitted|successful|complete)/i)).toBeVisible();
  });

  test('shows empty state when there are no claimable vaults', async ({ page }) => {
    await page.goto('/beneficiary');

    await expect(page.getByTestId('claimable-vault')).toHaveCount(0);
    await expect(page.getByText(/no claimable vaults/i)).toBeVisible();
  });
});
