import { Button } from '@/components/primitives/Button/Button';
import { Input } from '@/components/primitives/Input/Input';
import { Toggle } from '@/components/primitives/Toggle/Toggle';
import { useT } from '@/i18n/useT';
import type { AppSettings, WhisperModelInfo } from '@/ipc/bindings';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useRef, useState } from 'react';
import styles from './SettingsPage.module.css';

const LANGUAGE_LABELS: Record<string, string> = { es: 'Español', en: 'English' };

type TranscriptionProvider = 'local' | 'openai' | 'azure';

export function TranscriptionPanel({
  draft,
  patch,
}: { draft: AppSettings; patch: (p: Partial<AppSettings>) => void }) {
  const { t } = useT();
  const [models, setModels] = useState<WhisperModelInfo[]>([]);
  const [dl, setDl] = useState<{ id: string; pct: number } | null>(null);

  // Transcription provider selection — initialized from draft once (useRef guard)
  const initializedProvider = useRef(false);
  const [selectedProvider, setSelectedProvider] = useState<TranscriptionProvider>('local');
  useEffect(() => {
    if (!initializedProvider.current && draft.transcriptionProvider) {
      setSelectedProvider(draft.transcriptionProvider as TranscriptionProvider);
      initializedProvider.current = true;
    }
  }, [draft.transcriptionProvider]);

  // Azure deployment name — local state, synced from draft
  const [azureDeployment, setAzureDeployment] = useState<string>('');
  useEffect(() => {
    if (selectedProvider === 'azure') {
      setAzureDeployment(draft.transcriptionModels?.azure ?? '');
    }
  }, [draft.transcriptionModels, selectedProvider]);

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

  function handleProviderChange(p: TranscriptionProvider) {
    setSelectedProvider(p);
    // Persist immediately so the backend factory sees the new value right away.
    // For local, there is no Save button so we must always persist on change.
    patch({ transcriptionProvider: p });
  }

  return (
    <div>
      {/* Transcription provider selector */}
      <div className={styles.groupHead}>{t('transcriptionProviderLabel')}</div>
      <div className={styles.row}>
        <div className={styles.rowLeft}>
          <div className={styles.rowLabel}>{t('transcriptionProviderLabel')}</div>
        </div>
        <select
          aria-label={t('transcriptionProviderLabel')}
          value={selectedProvider}
          onChange={(e) => handleProviderChange(e.target.value as TranscriptionProvider)}
          style={{
            padding: '7px 12px',
            background: 'var(--bg-surface)',
            border: '1px solid var(--stroke-strong)',
            borderRadius: 'var(--radius)',
            fontSize: 13,
            color: 'inherit',
            fontFamily: 'inherit',
            minWidth: 160,
          }}
        >
          <option value="local">{t('providerLocal')}</option>
          <option value="openai">{t('providerOpenAi')}</option>
          <option value="azure">{t('providerAzure')}</option>
        </select>
      </div>

      {/* OpenAI: fixed model whisper-1, key reused from AI Providers */}
      {selectedProvider === 'openai' && (
        <div className={styles.row}>
          <div className={styles.rowLeft} style={{ maxWidth: '100%' }}>
            <div className={styles.rowDesc}>
              {t('sttKeyHint')} {t('providerOpenAi')}: whisper-1.
            </div>
          </div>
        </div>
      )}

      {/* Azure: deployment name input + hint */}
      {selectedProvider === 'azure' && (
        <>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>{t('azureWhisperDeployment')}</div>
            </div>
            <div style={{ flex: 1, maxWidth: 340 }}>
              <Input
                type="text"
                placeholder={t('azureDeploymentHint')}
                value={azureDeployment}
                onChange={(e) => setAzureDeployment(e.target.value)}
                onBlur={() => {
                  patch({
                    transcriptionModels: {
                      ...draft.transcriptionModels,
                      azure: azureDeployment.trim(),
                    },
                  });
                }}
              />
            </div>
          </div>
          <div className={styles.row}>
            <div className={styles.rowLeft} style={{ maxWidth: '100%' }}>
              <div className={styles.rowDesc}>{t('sttKeyHint')}</div>
            </div>
          </div>
        </>
      )}

      {/* Local: Whisper model selection */}
      {selectedProvider === 'local' && (
        <>
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
        </>
      )}

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
