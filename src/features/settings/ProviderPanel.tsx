import { Button } from '@/components/primitives/Button/Button';
import { Input } from '@/components/primitives/Input/Input';
import { useT } from '@/i18n/useT';
import {
  useGetProviderConfigQuery,
  useTestApiKeyMutation,
  useUpdateProviderConfigMutation,
} from '@/store/api/providers.api';
import { useGetSettingsQuery, useUpdateSettingsMutation } from '@/store/api/settings.api';
import { useEffect, useRef, useState } from 'react';
import styles from './SettingsPage.module.css';

const AI_PROVIDERS = ['local', 'openai', 'anthropic', 'azure'] as const;
type AiProvider = (typeof AI_PROVIDERS)[number];

// Azure is omitted: it uses the deployment name (no fixed default). The azure
// branch falls back to the t('azureDeploymentHint') label instead.
const DEFAULT_MODELS: Record<'openai' | 'anthropic', string> = {
  openai: 'gpt-4o-mini',
  anthropic: 'claude-3-5-sonnet-latest',
};

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
  const [azureEndpoint, setAzureEndpoint] = useState('');
  const [testStatus, setTestStatus] = useState<'idle' | 'ok' | 'error'>('idle');
  const [testError, setTestError] = useState('');

  const [updateProviderConfig, { isLoading: isSaving }] = useUpdateProviderConfigMutation();
  const [testApiKey, { isLoading: isTesting }] = useTestApiKeyMutation();
  const { data: settings } = useGetSettingsQuery();
  const [updateSettings] = useUpdateSettingsMutation();

  const currentConfig = aiConfigs.find((c) => c.provider === selectedProvider);
  const isCloud = selectedProvider !== 'local';

  // Open the panel on the ACTIVE provider: sync the selection from settings.aiProvider
  // ONCE when settings first load, then respect the user's manual changes.
  const initializedFromSettings = useRef(false);
  useEffect(() => {
    if (!initializedFromSettings.current && settings?.aiProvider) {
      setSelectedProvider(settings.aiProvider as AiProvider);
      initializedFromSettings.current = true;
    }
  }, [settings?.aiProvider]);

  // Sync the Azure endpoint field from settings whenever settings load or provider changes to azure
  useEffect(() => {
    if (selectedProvider === 'azure') {
      setAzureEndpoint(settings?.azureEndpoint ?? '');
    }
  }, [settings?.azureEndpoint, selectedProvider]);

  // Compute model input placeholder: configured model > provider default > deployment hint for azure
  const modelPlaceholder =
    currentConfig?.model ||
    (selectedProvider === 'azure'
      ? t('azureDeploymentHint')
      : selectedProvider === 'openai' || selectedProvider === 'anthropic'
        ? DEFAULT_MODELS[selectedProvider]
        : '');

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
    // Persist aiProvider for ALL cloud providers (not just azure): the backend
    // factory reads settings.ai_provider to pick local-vs-cloud, and
    // update_provider_config only writes ai_provider when a model is passed —
    // which the user usually leaves blank (placeholder default). Write settings
    // first (spread preserves the existing providerModels map), then
    // update_provider_config writes the per-provider model into provider_models[provider]
    // if the user typed one.
    if (settings) {
      await updateSettings({
        ...settings,
        aiProvider: selectedProvider,
        ...(selectedProvider === 'azure' ? { azureEndpoint: azureEndpoint.trim() } : {}),
      });
    }
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
            const p = e.target.value as AiProvider;
            setSelectedProvider(p);
            setApiKey('');
            setModelId('');
            setTestStatus('idle');
            setTestError('');
            // Local has no Save button, so persist the switch back to local immediately.
            if (p === 'local' && settings) {
              void updateSettings({ ...settings, aiProvider: 'local' });
            }
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

          {/* Azure endpoint (only for azure provider) */}
          {selectedProvider === 'azure' && (
            <div className={styles.row}>
              <div className={styles.rowLeft}>
                <div className={styles.rowLabel}>{t('azureEndpoint')}</div>
              </div>
              <div style={{ flex: 1, maxWidth: 340 }}>
                <Input
                  type="text"
                  placeholder={t('azureEndpointPlaceholder')}
                  value={azureEndpoint}
                  onChange={(e) => setAzureEndpoint(e.target.value)}
                />
              </div>
            </div>
          )}

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
                placeholder={modelPlaceholder}
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
