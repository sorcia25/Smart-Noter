import { Button } from '@/components/primitives/Button/Button';
import { Input } from '@/components/primitives/Input/Input';
import { useT } from '@/i18n/useT';
import {
  useGetProviderConfigQuery,
  useTestApiKeyMutation,
  useUpdateProviderConfigMutation,
} from '@/store/api/providers.api';
import { useState } from 'react';
import styles from './SettingsPage.module.css';

const AI_PROVIDERS = ['local', 'openai', 'anthropic', 'azure'] as const;
type AiProvider = (typeof AI_PROVIDERS)[number];

export function ProviderPanel() {
  const { t } = useT();
  const { data: rawConfigs } = useGetProviderConfigQuery();
  const configs = rawConfigs ?? [];

  // Derive the active AI provider from the data (domain === 'ai')
  const aiConfigs = configs.filter((c) => c.domain === 'ai');
  const firstCloud = aiConfigs.find((c) => c.configured);
  const initialProvider: AiProvider = (firstCloud?.provider as AiProvider | undefined) ?? 'local';

  const [selectedProvider, setSelectedProvider] = useState<AiProvider>(initialProvider);
  const [apiKey, setApiKey] = useState('');
  const [modelId, setModelId] = useState('');
  const [testStatus, setTestStatus] = useState<'idle' | 'ok' | 'error'>('idle');
  const [testError, setTestError] = useState('');

  const [updateProviderConfig, { isLoading: isSaving }] = useUpdateProviderConfigMutation();
  const [testApiKey, { isLoading: isTesting }] = useTestApiKeyMutation();

  const currentConfig = aiConfigs.find((c) => c.provider === selectedProvider);
  const isCloud = selectedProvider !== 'local';

  async function handleTest() {
    setTestStatus('idle');
    setTestError('');
    const result = await testApiKey({ provider: selectedProvider });
    if ('error' in result && result.error) {
      const err = result.error as { message?: string };
      setTestStatus('error');
      setTestError(typeof err.message === 'string' ? err.message : String(err));
    } else {
      setTestStatus('ok');
    }
  }

  async function handleSave() {
    await updateProviderConfig({
      provider: selectedProvider,
      key: apiKey.trim() !== '' ? apiKey.trim() : undefined,
      model: modelId.trim() !== '' ? modelId.trim() : undefined,
    });
    setApiKey('');
  }

  return (
    <div>
      <div className={styles.groupHead}>{t('providersTitle')}</div>

      {/* Provider selector */}
      <div className={styles.row}>
        <div className={styles.rowLeft}>
          <div className={styles.rowLabel}>{t('providersTitle')}</div>
        </div>
        <select
          value={selectedProvider}
          onChange={(e) => {
            setSelectedProvider(e.target.value as AiProvider);
            setApiKey('');
            setModelId('');
            setTestStatus('idle');
            setTestError('');
          }}
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
          <option value="anthropic">{t('providerAnthropic')}</option>
          <option value="azure">{t('providerAzure')}</option>
        </select>
      </div>

      {/* Cloud provider fields */}
      {isCloud && (
        <>
          {/* Privacy disclaimer */}
          <div className={styles.row} style={{ borderBottom: '1px solid var(--stroke)' }}>
            <div className={styles.rowLeft} style={{ maxWidth: '100%' }}>
              <div className={styles.rowDesc}>{t('cloudPrivacyDisclaimer')}</div>
            </div>
          </div>

          {/* API key input (write-only — never pre-filled) */}
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>{t('apiKey')}</div>
            </div>
            <div style={{ flex: 1, maxWidth: 340 }}>
              <Input
                type="password"
                placeholder={
                  currentConfig?.configured && currentConfig.keyLast4
                    ? `••••${currentConfig.keyLast4}`
                    : '••••••••'
                }
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                autoComplete="new-password"
              />
            </div>
          </div>

          {/* Model id input */}
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>{t('aiModelLabel')}</div>
            </div>
            <div style={{ flex: 1, maxWidth: 340 }}>
              <Input
                type="text"
                placeholder={currentConfig?.model ?? ''}
                value={modelId}
                onChange={(e) => setModelId(e.target.value)}
              />
            </div>
          </div>

          {/* Actions row */}
          <div className={styles.row} style={{ gap: 10, justifyContent: 'flex-end' }}>
            {/* Test connection */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Button
                variant="default"
                size="sm"
                loading={isTesting}
                onClick={() => void handleTest()}
              >
                {t('testConnection')}
              </Button>
              {testStatus === 'ok' && (
                <span style={{ color: 'var(--color-success, #10b981)', fontSize: 13 }}>
                  ✓ {t('connectionOk')}
                </span>
              )}
              {testStatus === 'error' && (
                <span style={{ color: 'var(--color-danger, #ef4444)', fontSize: 13 }}>
                  ✗ {testError}
                </span>
              )}
            </div>

            {/* Save */}
            <Button
              variant="primary"
              size="sm"
              loading={isSaving}
              onClick={() => void handleSave()}
            >
              {t('save')}
            </Button>
          </div>
        </>
      )}

      {/* Local — no extra config needed */}
      {!isCloud && (
        <div className={styles.row}>
          <div className={styles.rowLeft}>
            <div className={styles.rowDesc}>{t('providerLocal')}</div>
          </div>
        </div>
      )}
    </div>
  );
}
