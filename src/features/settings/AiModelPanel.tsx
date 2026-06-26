import { Button } from '@/components/primitives/Button/Button';
import { Toggle } from '@/components/primitives/Toggle/Toggle';
import { useT } from '@/i18n/useT';
import type { AppSettings } from '@/ipc/bindings';
import {
  useCancelLlmDownloadMutation,
  useDeleteLlmModelMutation,
  useDownloadLlmModelMutation,
  useListLlmModelsQuery,
} from '@/store/api/ai.api';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';
import styles from './SettingsPage.module.css';

export function AiModelPanel({
  draft,
  patch,
}: { draft: AppSettings; patch: (p: Partial<AppSettings>) => void }) {
  const { t } = useT();
  const { data: rawModels, refetch } = useListLlmModelsQuery();
  const models = rawModels ?? [];
  const [dl, setDl] = useState<{ id: string; pct: number } | null>(null);

  const [downloadLlmModel] = useDownloadLlmModelMutation();
  const [cancelLlmDownload] = useCancelLlmDownloadMutation();
  const [deleteLlmModel] = useDeleteLlmModelMutation();

  const refresh = useCallback(() => {
    void refetch();
  }, [refetch]);

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
        'llm-download:progress',
        (e) => {
          if (!cancelled) setDl({ id: e.payload.id, pct: e.payload.pct });
        }
      )
    );
    add(
      listen<{ id: string }>('llm-download:completed', () => {
        if (!cancelled) {
          setDl(null);
          refresh();
        }
      })
    );
    add(
      listen<{ id: string; code: string; message: string }>('llm-download:failed', () => {
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
      <div className={styles.groupHead}>{t('aiModel')}</div>
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
              onClick={() => void deleteLlmModel({ id: m.id }).then(refresh)}
            >
              {t('transcribe.delete')}
            </Button>
          ) : dl?.id === m.id ? (
            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <span>{dl.pct}%</span>
              <Button variant="default" onClick={() => void cancelLlmDownload({ id: m.id })}>
                {t('transcribe.cancel')}
              </Button>
            </div>
          ) : (
            <Button variant="default" onClick={() => void downloadLlmModel({ id: m.id })}>
              {t('downloadModel')}
            </Button>
          )}
        </div>
      ))}
      <div className={styles.row}>
        <div className={styles.rowLeft}>
          <div className={styles.rowLabel}>{t('autoSummary')}</div>
        </div>
        <Toggle
          on={draft.autoGenerateSummary ?? false}
          onChange={(v) => patch({ autoGenerateSummary: v })}
          aria-label={t('autoSummary')}
        />
      </div>
    </div>
  );
}
