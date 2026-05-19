import { expect, test } from '@playwright/test';

const captures = [
  { path: '/', name: 'dashboard-light' },
  { path: '/meetings', name: 'meetings-list-light' },
  { path: '/record/new', name: 'pre-record-light' },
  { path: '/record/live/sess-test', name: 'live-recording-light' },
  { path: '/meetings/m-001', name: 'detail-summary-light' },
  { path: '/templates', name: 'templates-light' },
  { path: '/participants', name: 'participants-light' },
  { path: '/settings', name: 'settings-local-light' },
];

for (const { path, name } of captures) {
  test(`visual: ${name}`, async ({ page }) => {
    await page.goto(path);
    await page.waitForLoadState('networkidle');
    // Pause animations + transitions for a stable screenshot.
    await page.addStyleTag({
      content: `
        *, *::before, *::after {
          animation: none !important;
          transition: none !important;
        }
      `,
    });
    // Give React one frame to settle layout after the style injection.
    await page.waitForTimeout(100);
    await expect(page).toHaveScreenshot(`${name}.png`, { fullPage: true });
  });
}
