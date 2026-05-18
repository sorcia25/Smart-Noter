import { expect, test } from '@playwright/test';

const screens = [
  { path: '/', label: '01 Dashboard' },
  { path: '/meetings', label: '02 Meetings list' },
  { path: '/record/new', label: '03 Pre-record' },
  { path: '/record/live/sess-test', label: '04 Live recording' },
  { path: '/meetings/m-001', label: '05 Meeting detail' },
  { path: '/templates', label: '06 Templates' },
  { path: '/participants', label: '07 Participants' },
  { path: '/settings', label: '08 Settings' },
];

for (const { path, label } of screens) {
  test(`navigates to ${label}`, async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (msg) => {
      if (msg.type() === 'error') errors.push(msg.text());
    });

    await page.goto(path);
    await expect(page.locator(`[data-screen-label="${label}"]`)).toBeVisible();
    // Filter out RTK Query errors caused by missing Tauri IPC in the dev server context —
    // these are expected: the Vite-only run can't reach the Rust backend.
    const realErrors = errors.filter(
      (e) =>
        !e.includes('IPC') &&
        !e.includes('tauri') &&
        !e.includes('Failed to fetch') &&
        !e.includes('window.__TAURI__')
    );
    expect(realErrors, `console errors on ${path}`).toEqual([]);
  });
}
