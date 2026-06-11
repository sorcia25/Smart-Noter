import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { Input } from '@/components/primitives/Input/Input';
import { SegmentedControl } from '@/components/primitives/SegmentedControl/SegmentedControl';
import { Toggle } from '@/components/primitives/Toggle/Toggle';
import { useT } from '@/i18n/useT';
import type { AppSettings, AvatarStyle, CaptureMode, Language, Theme } from '@/ipc/bindings';
import { useListAudioDevicesQuery } from '@/store/api/devices.api';
import { useGetSettingsQuery, useUpdateSettingsMutation } from '@/store/api/settings.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { useAppDispatch } from '@/store/hooks';
import { setAccent, setLanguage, setTheme } from '@/store/slices/ui.slice';
import { pickL } from '@/utils/format';
import { useEffect, useState } from 'react';
import styles from './SettingsPage.module.css';
import { PROVIDERS, type ProviderId } from './providers';

const DEFAULT_SETTINGS: AppSettings = {
  theme: 'light',
  accent: '#10b981',
  language: 'es',
  avatarStyle: 'circle',
  aiChatVisible: true,
  captureMode: 'system',
  defaultDevice: 'system-loopback',
  recordingQuality: 'WAV 48k',
  runLocal: true,
  autoDeleteAudio: false,
  transcriptionProvider: 'local',
  transcriptionModel: 'Whisper Large v3',
  defaultTemplate: 'tecnica',
};

const ACCENT_SWATCHES = ['#10b981', '#3b82f6', '#8b5cf6', '#ec4899', '#f59e0b'] as const;

const PROVIDER_METRICS: Record<
  ProviderId,
  { label: { es: string; en: string }; value: string | { es: string; en: string } }[]
> = {
  local: [
    { label: { es: 'Fidelidad', en: 'Fidelity' }, value: '99.2%' },
    { label: { es: 'Latencia', en: 'Latency' }, value: '1.8s' },
    { label: { es: 'Costo', en: 'Cost' }, value: { es: 'Gratis', en: 'Free' } },
    { label: { es: 'Privacidad', en: 'Privacy' }, value: { es: 'Máxima', en: 'Maximum' } },
  ],
  openai: [
    { label: { es: 'Fidelidad', en: 'Fidelity' }, value: '99.5%' },
    { label: { es: 'Latencia', en: 'Latency' }, value: '0.6s' },
    { label: { es: 'Costo', en: 'Cost' }, value: '~$0.006/min' },
    { label: { es: 'Privacidad', en: 'Privacy' }, value: { es: 'Estándar', en: 'Standard' } },
  ],
  azure: [
    { label: { es: 'Fidelidad', en: 'Fidelity' }, value: '99.4%' },
    { label: { es: 'Latencia', en: 'Latency' }, value: '0.8s' },
    { label: { es: 'Costo', en: 'Cost' }, value: { es: 'Consumo Azure', en: 'Azure consumption' } },
    { label: { es: 'Privacidad', en: 'Privacy' }, value: { es: 'Tu tenant', en: 'Your tenant' } },
  ],
  custom: [
    { label: { es: 'Fidelidad', en: 'Fidelity' }, value: '—' },
    { label: { es: 'Latencia', en: 'Latency' }, value: '—' },
    { label: { es: 'Costo', en: 'Cost' }, value: { es: 'Variable', en: 'Variable' } },
    {
      label: { es: 'Privacidad', en: 'Privacy' },
      value: { es: 'Tu endpoint', en: 'Your endpoint' },
    },
  ],
};

export default function SettingsPage() {
  const { t, lang, setLang } = useT();
  const dispatch = useAppDispatch();

  const { data: serverSettings } = useGetSettingsQuery();
  const { data: devices = [] } = useListAudioDevicesQuery();
  const { data: templates = [] } = useListTemplatesQuery();
  const [updateSettings] = useUpdateSettingsMutation();

  const [draft, setDraft] = useState<AppSettings>(serverSettings ?? DEFAULT_SETTINGS);

  useEffect(() => {
    if (serverSettings) setDraft(serverSettings);
  }, [serverSettings]);

  // Debounce-save the draft.
  useEffect(() => {
    if (!serverSettings) return;
    if (JSON.stringify(draft) === JSON.stringify(serverSettings)) return;
    const handle = setTimeout(() => void updateSettings(draft), 400);
    return () => clearTimeout(handle);
  }, [draft, serverSettings, updateSettings]);

  function patch(p: Partial<AppSettings>) {
    setDraft((prev) => ({ ...prev, ...p }));
  }

  const captureModes: { value: CaptureMode; label: string }[] = [
    { value: 'system', label: lang === 'es' ? 'Sistema' : 'System' },
    { value: 'mic', label: lang === 'es' ? 'Mic' : 'Mic' },
    { value: 'mix', label: lang === 'es' ? 'Mezcla' : 'Mix' },
  ];

  const qualityOptions: { value: string; label: string; disabled?: boolean }[] = [
    { value: 'WAV 48k', label: 'WAV 48k' },
    { value: 'FLAC', label: 'FLAC' },
    { value: 'MP3 192k', label: 'MP3 192k', disabled: true },
    { value: 'MP3 320k', label: 'MP3 320k', disabled: true },
  ];

  const themeOptions: { value: Theme; label: string }[] = [
    { value: 'light', label: lang === 'es' ? 'Claro' : 'Light' },
    { value: 'dark', label: lang === 'es' ? 'Oscuro' : 'Dark' },
  ];

  const langOptions: { value: Language; label: string }[] = [
    { value: 'es', label: 'Español' },
    { value: 'en', label: 'English' },
  ];

  const avatarOptions: { value: AvatarStyle; label: string }[] = [
    { value: 'circle', label: lang === 'es' ? 'Círculo' : 'Circle' },
    { value: 'square', label: lang === 'es' ? 'Cuadrado' : 'Square' },
  ];

  const currentProvider =
    PROVIDERS.find((p) => p.id === (draft.transcriptionProvider as ProviderId)) ?? PROVIDERS[0];
  if (!currentProvider) throw new Error('PROVIDERS is empty');

  return (
    <div className={styles.page} data-screen-label="08 Settings">
      <div className={styles.header}>
        <h1 className={styles.title}>{t('settingsTitle')}</h1>
        <div className={styles.sub}>{t('settingsSub')}</div>
      </div>
      <div className={styles.scroll}>
        {/* Personalización */}
        <div className={styles.group}>
          <div className={styles.groupHead}>
            {lang === 'es' ? 'Personalización' : 'Personalization'}
          </div>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>{lang === 'es' ? 'Tema' : 'Theme'}</div>
              <div className={styles.rowDesc}>
                {lang === 'es'
                  ? 'Claro u oscuro. Cambia al instante.'
                  : 'Light or dark. Switches instantly.'}
              </div>
            </div>
            <SegmentedControl<Theme>
              value={draft.theme}
              options={themeOptions}
              onChange={(v) => {
                patch({ theme: v });
                dispatch(setTheme(v));
              }}
            />
          </div>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>{lang === 'es' ? 'Acento' : 'Accent'}</div>
              <div className={styles.rowDesc}>
                {lang === 'es' ? 'Color de marca y resaltes.' : 'Brand color and highlights.'}
              </div>
            </div>
            <div className={styles.accentRow} role="radiogroup" aria-label="accent">
              {ACCENT_SWATCHES.map((color) => (
                <button
                  key={color}
                  type="button"
                  role="radio"
                  aria-checked={draft.accent === color}
                  aria-label={color}
                  className={`${styles.accentSwatch} ${draft.accent === color ? styles.accentSwatchActive : ''}`}
                  style={{ background: color }}
                  onClick={() => {
                    patch({ accent: color });
                    dispatch(setAccent(color));
                  }}
                />
              ))}
            </div>
          </div>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>{lang === 'es' ? 'Idioma' : 'Language'}</div>
              <div className={styles.rowDesc}>
                {lang === 'es' ? 'Afecta la interfaz y resúmenes.' : 'Affects UI and summaries.'}
              </div>
            </div>
            <SegmentedControl<Language>
              value={draft.language}
              options={langOptions}
              onChange={(v) => {
                patch({ language: v });
                dispatch(setLanguage(v));
                setLang(v);
              }}
            />
          </div>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>
                {lang === 'es' ? 'Estilo de avatar' : 'Avatar style'}
              </div>
              <div className={styles.rowDesc}>
                {lang === 'es'
                  ? 'Forma de los avatares de hablantes.'
                  : 'Shape of speaker avatars.'}
              </div>
            </div>
            <SegmentedControl<AvatarStyle>
              value={draft.avatarStyle}
              options={avatarOptions}
              onChange={(v) => patch({ avatarStyle: v })}
            />
          </div>
        </div>

        {/* Audio capture */}
        <div className={styles.group}>
          <div className={styles.groupHead}>{t('audioCapture')}</div>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>{t('captureMode')}</div>
              <div className={styles.rowDesc}>{t('captureModeDesc')}</div>
            </div>
            {/* AppSettings stores captureMode as plain string (dedup of bindings type);
                'system'|'mic'|'mix' remains the authoritative CaptureMode enum. */}
            <SegmentedControl<CaptureMode>
              value={draft.captureMode as CaptureMode}
              options={captureModes}
              onChange={(v) => patch({ captureMode: v })}
            />
          </div>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>{t('defaultDevice')}</div>
              <div className={styles.rowDesc}>
                {lang === 'es'
                  ? 'Se usará automáticamente al iniciar una nueva grabación.'
                  : 'Used automatically when starting a new recording.'}
              </div>
            </div>
            <div className={styles.selectTrigger}>
              <span>
                {devices.find((d) => d.id === draft.defaultDevice)?.name ??
                  devices[0]?.name ??
                  (lang === 'es' ? 'Sin dispositivo' : 'No device')}
              </span>
              <Icon name="chevDown" size={14} />
            </div>
          </div>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>
                {lang === 'es' ? 'Calidad de grabación' : 'Recording quality'}
              </div>
              <div className={styles.rowDesc}>
                {lang === 'es'
                  ? 'Mayor calidad ocupa más espacio en disco.'
                  : 'Higher quality uses more disk space.'}
              </div>
            </div>
            <SegmentedControl<string>
              value={draft.recordingQuality}
              options={qualityOptions}
              onChange={(v) => patch({ recordingQuality: v })}
            />
          </div>
        </div>

        {/* Transcription engine */}
        <div className={styles.group}>
          <div className={styles.groupHead}>
            <span>{t('transcriptionEngineLabel')}</span>
            <span style={{ textTransform: 'none', letterSpacing: 0, fontWeight: 500 }}>
              {currentProvider.short}
            </span>
          </div>
          <div className={styles.engineGrid}>
            {PROVIDERS.map((p) => {
              const selected = draft.transcriptionProvider === p.id;
              return (
                <button
                  key={p.id}
                  type="button"
                  className={`${styles.engineCard} ${selected ? styles.engineCardSelected : ''}`}
                  onClick={() =>
                    patch({
                      transcriptionProvider: p.id,
                      transcriptionModel: p.models[0] ?? draft.transcriptionModel,
                    })
                  }
                >
                  <div className={styles.engineCardHead}>
                    <div
                      className={styles.engineIcon}
                      style={{ background: `${p.color}22`, color: p.color }}
                    >
                      <Icon name={p.icon} size={16} stroke={p.color} />
                    </div>
                    <div className={styles.engineRadio}>
                      {selected && <div className={styles.engineRadioDot} />}
                    </div>
                  </div>
                  <div className={styles.engineName}>{pickL(p.name, lang)}</div>
                  <div className={styles.engineDesc}>{pickL(p.desc, lang)}</div>
                  <div
                    className={`${styles.engineBadge} ${p.badgeAccent ? styles.engineBadgeAccent : ''}`}
                  >
                    {pickL(p.badge, lang)}
                  </div>
                </button>
              );
            })}
          </div>

          <div className={styles.providerConfig} data-provider={currentProvider.id}>
            <div className={styles.row} style={{ borderBottom: 'none', padding: 0 }}>
              <div className={styles.rowLeft}>
                <div className={styles.rowLabel}>{lang === 'es' ? 'Modelo' : 'Model'}</div>
                <div className={styles.rowDesc}>
                  {lang === 'es'
                    ? 'Modelo activo para este proveedor.'
                    : 'Active model for this provider.'}
                </div>
              </div>
              <div className={styles.selectTrigger}>
                <span>{draft.transcriptionModel}</span>
                <Icon name="chevDown" size={14} />
              </div>
            </div>

            {currentProvider.id === 'openai' && (
              <>
                <Input
                  label={lang === 'es' ? 'API key de OpenAI' : 'OpenAI API key'}
                  type="password"
                  placeholder="sk-proj-..."
                  value=""
                  onChange={() => {}}
                />
                <Button size="sm" disabled title={lang === 'es' ? 'Próximamente' : 'Coming soon'}>
                  {lang === 'es' ? 'Probar conexión' : 'Test connection'}
                </Button>
              </>
            )}

            {currentProvider.id === 'azure' && (
              <>
                <Input
                  label="Endpoint"
                  placeholder="https://acme-noter.openai.azure.com"
                  value=""
                  onChange={() => {}}
                />
                <Input
                  label={lang === 'es' ? 'Despliegue' : 'Deployment'}
                  placeholder="whisper-prod"
                  value=""
                  onChange={() => {}}
                />
                <Input
                  label="API key"
                  type="password"
                  placeholder="..."
                  value=""
                  onChange={() => {}}
                />
                <Button size="sm" disabled title={lang === 'es' ? 'Próximamente' : 'Coming soon'}>
                  {lang === 'es' ? 'Probar conexión' : 'Test connection'}
                </Button>
              </>
            )}

            {currentProvider.id === 'custom' && (
              <>
                <Input
                  label={lang === 'es' ? 'URL base' : 'Base URL'}
                  placeholder="https://api.groq.com/openai/v1"
                  value=""
                  onChange={() => {}}
                />
                <Input
                  label="API key"
                  type="password"
                  placeholder="..."
                  value=""
                  onChange={() => {}}
                />
              </>
            )}

            {currentProvider.id === 'local' && (
              <div className={styles.rowDesc}>
                {lang === 'es'
                  ? 'Procesamiento en este equipo. Whisper Large v3 instalado (2.9 GB).'
                  : 'Processed on this device. Whisper Large v3 installed (2.9 GB).'}
              </div>
            )}
          </div>

          <div className={styles.metrics}>
            {PROVIDER_METRICS[currentProvider.id]?.map((m) => (
              <div key={m.label.es} className={styles.metricCard}>
                <div className={styles.metricLabel}>{pickL(m.label, lang)}</div>
                <div className={styles.metricValue}>
                  {typeof m.value === 'string' ? m.value : pickL(m.value, lang)}
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Privacy + storage */}
        <div className={styles.group}>
          <div className={styles.groupHead}>
            {t('privacy')} & {t('storage')}
          </div>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>{t('autoDeleteAudio')}</div>
              <div className={styles.rowDesc}>{t('autoDeleteAudioDesc')}</div>
            </div>
            <Toggle
              on={draft.autoDeleteAudio}
              onChange={(v) => patch({ autoDeleteAudio: v })}
              aria-label="auto-delete-audio"
            />
          </div>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>
                {lang === 'es' ? 'Ubicación de archivos' : 'File location'}
              </div>
              <div className={styles.rowDesc} style={{ fontFamily: 'var(--font-mono)' }}>
                C:\Users\carlos\Documents\SmartNoter
              </div>
            </div>
            <Button disabled title={lang === 'es' ? 'Próximamente' : 'Coming soon'}>
              {lang === 'es' ? 'Cambiar' : 'Change'}
            </Button>
          </div>
        </div>

        {/* Default template */}
        {templates.length > 0 && (
          <div className={styles.group}>
            <div className={styles.groupHead}>
              {lang === 'es' ? 'Plantilla predeterminada' : 'Default template'}
            </div>
            <div className={styles.row}>
              <div className={styles.rowLeft}>
                <div className={styles.rowLabel}>
                  {lang === 'es' ? 'Plantilla por defecto' : 'Default template'}
                </div>
                <div className={styles.rowDesc}>
                  {lang === 'es'
                    ? 'Se usa al crear una nueva grabación.'
                    : 'Used when starting a new recording.'}
                </div>
              </div>
              <div className={styles.selectTrigger}>
                <span>
                  {pickL(
                    templates.find((tpl) => tpl.id === draft.defaultTemplate)?.name ?? null,
                    lang
                  ) || draft.defaultTemplate}
                </span>
                <Icon name="chevDown" size={14} />
              </div>
            </div>
          </div>
        )}

        <div className={styles.footer}>
          Smart Noter v3.1.4 ·{' '}
          <a href="#updates">{lang === 'es' ? 'Buscar actualizaciones' : 'Check for updates'}</a>
        </div>
      </div>
    </div>
  );
}
