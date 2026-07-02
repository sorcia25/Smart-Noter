import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }));
import { check } from '@tauri-apps/plugin-updater';
import { checkForUpdate } from './updater';

describe('checkForUpdate', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
  });

  it('returns null outside a Tauri context (and never calls check)', async () => {
    // biome-ignore lint/performance/noDelete: checkForUpdate() uses the `in` operator, so the key must be truly absent
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    expect(await checkForUpdate()).toBeNull();
    expect(check).not.toHaveBeenCalled();
  });

  it('returns null when up to date', async () => {
    (check as ReturnType<typeof vi.fn>).mockResolvedValue(null);
    expect(await checkForUpdate()).toBeNull();
  });

  it('returns the update handle when one is available', async () => {
    const fake = { version: '1.0.1', body: 'notes' };
    (check as ReturnType<typeof vi.fn>).mockResolvedValue(fake);
    expect(await checkForUpdate()).toBe(fake);
  });
});
