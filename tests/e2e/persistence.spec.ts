import { expect, test } from '@playwright/test';

test('theme persists across reload via localStorage', async ({ page }) => {
  await page.goto('/settings');
  // Click the dark-theme tab in the Theme SegmentedControl.
  await page.getByRole('tab', { name: /Oscuro|Dark/ }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('language persists across reload via localStorage', async ({ page }) => {
  await page.goto('/settings');
  // Click the English tab in the Language SegmentedControl.
  await page.getByRole('tab', { name: 'English' }).click();
  // After switching language, the sidebar nav uses the EN dictionary.
  await expect(page.getByRole('link', { name: 'Home' }).first()).toBeVisible();
  await page.reload();
  await expect(page.getByRole('link', { name: 'Home' }).first()).toBeVisible();
});

test('accent picker writes the selected color to --accent', async ({ page }) => {
  await page.goto('/settings');
  // Click the blue swatch (#3b82f6).
  await page.getByRole('radio', { name: '#3b82f6' }).click();
  const accent = await page.evaluate(() =>
    getComputedStyle(document.documentElement).getPropertyValue('--accent').trim()
  );
  expect(accent.toLowerCase()).toBe('#3b82f6');
});
