import { Button } from '@/components/primitives/Button/Button';
import { useT } from '@/i18n/useT';
import type { DiarizationModelInfo } from '@/ipc/bindings';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';
import styles from './SettingsPage.module.css';

export function DiarizationPanel() {
  const { t } = useT();
  const [models, setModels] = useState<DiarizationModelInfo[]>([]);
  const [dl, setDl] = useState<{ id: string; pct: number } | null>(null);

  const refresh = useCallback(() => {
    void invoke<DiarizationModelInfo[]>('list_diarization_models')
      .then((ms) => {
        if (ms) setModels(ms);
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    let cancelled = false;
    const unsubs: Array<() => void> = [];
    const add = (p: Promise<() => void>) =>
      p
        .then((u) => {
          if (cancelled) u();
          else unsubs.push(u);
        })
        .catch(() => {});

    add(
      listen<{ id: string; pct: number; bytesDownloaded: number; bytesTotal: number }>(
        'diarization-download:progress',
        (e) => {
          if (!cancelled) setDl({ id: e.payload.id, pct: e.payload.pct });
        }
      )
    );
    add(
      listen<{ id: string }>('diarization-download:completed', () => {
        if (!cancelled) {
          setDl(null);
          refresh();
        }
      })
    );
    add(
      listen<{ id: string; code: string; message: string }>('diarization-download:failed', () => {
        if (!cancelled) setDl(null);
      })
    );

    return () => {
      cancelled = true;
      for (const u of unsubs) u();
    };
  }, [refresh]);

  return (
    <div>
      <div className={styles.groupHead}>{t('diarize.modelSection')}</div>
      {models.map((m) => (
        <div key={m.id} className={styles.row}>
          <div className={styles.rowLeft}>
            <div className={styles.rowLabel}>
              {m.name} · {m.sizeMb} MB
            </div>
          </div>
          {m.downloaded ? (
            <Button
              variant="default"
              onClick={() => void invoke('delete_diarization_model', { id: m.id }).then(refresh)}
            >
              {t('diarize.delete')}
            </Button>
          ) : dl?.id === m.id ? (
            <span>{dl.pct}%</span>
          ) : (
            <Button
              variant="default"
              onClick={() => void invoke('download_diarization_model', { id: m.id })}
            >
              {t('diarize.download')}
            </Button>
          )}
        </div>
      ))}
    </div>
  );
}
