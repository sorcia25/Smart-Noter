import { relaunch } from '@tauri-apps/plugin-process';
import type { Update } from '@tauri-apps/plugin-updater';
import { useCallback, useState } from 'react';
import { checkForUpdate } from './updater';

export type UpdateStatus =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'upToDate' }
  | { kind: 'available'; version: string; notes: string; update: Update }
  | { kind: 'downloading'; downloaded: number; total: number | null }
  | { kind: 'error'; message: string };

export function useAppUpdater() {
  const [status, setStatus] = useState<UpdateStatus>({ kind: 'idle' });

  const check = useCallback(async () => {
    setStatus({ kind: 'checking' });
    try {
      const update = await checkForUpdate();
      if (!update) {
        setStatus({ kind: 'upToDate' });
        return;
      }
      setStatus({ kind: 'available', version: update.version, notes: update.body ?? '', update });
    } catch (e) {
      setStatus({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    }
  }, []);

  const install = useCallback(async (update: Update) => {
    let total: number | null = null;
    let downloaded = 0;
    setStatus({ kind: 'downloading', downloaded: 0, total: null });
    try {
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            total = event.data.contentLength ?? null;
            setStatus({ kind: 'downloading', downloaded: 0, total });
            break;
          case 'Progress':
            downloaded += event.data.chunkLength;
            setStatus({ kind: 'downloading', downloaded, total });
            break;
          case 'Finished':
            setStatus({ kind: 'downloading', downloaded, total });
            break;
        }
      });
      await relaunch();
    } catch (e) {
      setStatus({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    }
  }, []);

  return { status, check, install };
}
