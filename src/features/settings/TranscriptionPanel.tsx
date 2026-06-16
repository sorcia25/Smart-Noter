import { Button } from '@/components/primitives/Button/Button';
import { Toggle } from '@/components/primitives/Toggle/Toggle';
import { useT } from '@/i18n/useT';
import type { AppSettings, WhisperModelInfo } from '@/ipc/bindings';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';
import styles from './SettingsPage.module.css';

const LANGUAGE_LABELS: Record<string, string> = { es: 'Español', en: 'English' };

export function TranscriptionPanel({
  draft,
  patch,
}: { draft: AppSettings; patch: (p: Partial<AppSettings>) => void }) {
  const { t } = useT();
  const [models, setModels] = useState<WhisperModelInfo[]>([]);
  const [dl, setDl] = useState<{ id: string; pct: number } | null>(null);

  const refresh = useCallback(() => {
    void invoke<WhisperModelInfo[]>('list_whisper_models')
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
      listen<{ id: string; pct: number }>('whisper-download:progress', (e) => {
        if (!cancelled) setDl(e.payload);
      })
    );
    add(
      listen<{ id: string }>('whisper-download:completed', () => {
        if (!cancelled) {
          setDl(null);
          refresh();
        }
      })
    );
    add(
      listen<{ id: string }>('whisper-download:failed', () => {
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
      <div className={styles.groupHead}>{t('transcribe.modelSection')}</div>
      {models.map((m) => (
        <div key={m.id} className={styles.row}>
          <div className={styles.rowLeft}>
            <div className={styles.rowLabel}>
              {m.name} · {m.sizeMb} MB
            </div>
          </div>
          {m.downloaded ? (
            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <input
                type="radio"
                name="model"
                aria-label={m.name}
                checked={draft.transcriptionModel === m.id}
                onChange={() => patch({ transcriptionModel: m.id })}
              />
              <Button
                variant="default"
                onClick={() => void invoke('delete_whisper_model', { id: m.id }).then(refresh)}
              >
                {t('transcribe.delete')}
              </Button>
            </div>
          ) : dl?.id === m.id ? (
            <span>{dl.pct}%</span>
          ) : (
            <Button
              variant="default"
              onClick={() => void invoke('download_whisper_model', { id: m.id })}
            >
              {t('transcribe.download')}
            </Button>
          )}
        </div>
      ))}
      <div className={styles.row}>
        <div className={styles.rowLeft}>
          <div className={styles.rowLabel}>{t('transcribe.autoLabel')}</div>
        </div>
        <Toggle
          on={draft.autoTranscribe}
          onChange={(v) => patch({ autoTranscribe: v })}
          aria-label={t('transcribe.autoLabel')}
        />
      </div>
      <div className={styles.row}>
        <div className={styles.rowLeft}>
          <div className={styles.rowLabel}>{t('transcribe.nativeLanguage')}</div>
        </div>
        <select
          value={draft.nativeLanguage}
          onChange={(e) => patch({ nativeLanguage: e.target.value })}
        >
          {Object.entries(LANGUAGE_LABELS).map(([code, label]) => (
            <option key={code} value={code}>
              {label}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
