import { type Update, check } from '@tauri-apps/plugin-updater';

/**
 * Check GitHub Releases for a newer version. Returns the Update handle when one
 * is available, null when up to date. Never throws outside a Tauri context
 * (vitest / e2e run in a plain browser with no IPC) — returns null instead.
 */
export async function checkForUpdate(): Promise<Update | null> {
  if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) return null;
  return (await check()) ?? null;
}
