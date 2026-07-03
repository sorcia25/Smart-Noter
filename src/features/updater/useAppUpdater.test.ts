import type { DownloadEvent } from '@tauri-apps/plugin-updater';
import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useAppUpdater } from './useAppUpdater';

vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));

describe('useAppUpdater install', () => {
  it('accumulates download progress from updater events', async () => {
    const update = {
      version: '1.1.0',
      body: '',
      downloadAndInstall: vi.fn(async (cb: (e: DownloadEvent) => void) => {
        cb({ event: 'Started', data: { contentLength: 1000 } });
        cb({ event: 'Progress', data: { chunkLength: 400 } });
        cb({ event: 'Progress', data: { chunkLength: 600 } });
        cb({ event: 'Finished' });
      }),
    };
    const { result } = renderHook(() => useAppUpdater());
    await act(async () => {
      await result.current.install(update as never);
    });
    expect(update.downloadAndInstall).toHaveBeenCalled();
    expect(update.downloadAndInstall).toHaveBeenCalledWith(expect.any(Function));
  });
});
