import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { SegmentedControl } from '@/components/primitives/SegmentedControl/SegmentedControl';
import { toast } from '@/components/primitives/Toast/Toast';
import { Toggle } from '@/components/primitives/Toggle/Toggle';
import { useT } from '@/i18n/useT';
import type { AppSettings, AvatarStyle, CaptureMode, Language, Theme } from '@/ipc/bindings';
import { useListAudioDevicesQuery } from '@/store/api/devices.api';
import { useGetSettingsQuery, useUpdateSettingsMutation } from '@/store/api/settings.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { useAppDispatch } from '@/store/hooks';
import { setAccent, setLanguage, setTheme } from '@/store/slices/ui.slice';
import { pickL } from '@/utils/format';
import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import { useAppUpdater } from '../updater/useAppUpdater';
import { AiModelPanel } from './AiModelPanel';
import { DiarizationPanel } from './DiarizationPanel';
import { ProviderPanel } from './ProviderPanel';
import styles from './SettingsPage.module.css';
import { TranscriptionPanel } from './TranscriptionPanel';

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
  transcriptionModel: 'large-v3',
  autoTranscribe: false,
  nativeLanguage: 'es',
  defaultTemplate: 'tecnica',
};

const ACCENT_SWATCHES = ['#10b981', '#3b82f6', '#8b5cf6', '#ec4899', '#f59e0b'] as const;

export default function SettingsPage() {
  const { t, lang, setLang } = useT();
  const dispatch = useAppDispatch();

  const { data: serverSettings } = useGetSettingsQuery();
  const { data: devices = [] } = useListAudioDevicesQuery();
  const { data: templates = [] } = useListTemplatesQuery();
  const [updateSettings] = useUpdateSettingsMutation();
  const updater = useAppUpdater();

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
    setDraft((prev: AppSettings) => ({ ...prev, ...p }));
  }

  // Audio storage location — managed by dedicated commands (the move + DB repoint
  // happen in the backend), kept in sync with the draft so the debounce-save
  // doesn't revert it.
  const [storageDir, setStorageDir] = useState<string>('');
  useEffect(() => {
    void invoke<string>('get_storage_dir')
      .then(setStorageDir)
      .catch(() => {});
  }, []);
  async function changeStorageDir() {
    try {
      const next = await invoke<string | null>('set_storage_dir');
      if (next) {
        setStorageDir(next);
        patch({ storageDir: next });
        toast.success(lang === 'es' ? 'Ubicación actualizada' : 'Location updated', {
          description: next,
        });
      }
    } catch (e) {
      toast.error(lang === 'es' ? 'No se pudo cambiar la ubicación' : 'Could not change location', {
        description: String(e),
      });
    }
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

        {/* Whisper model management */}
        <div className={styles.group}>
          <TranscriptionPanel draft={draft} patch={patch} />
        </div>

        {/* Diarization model management */}
        <div className={styles.group}>
          <DiarizationPanel />
        </div>

        {/* AI model management + auto-summary toggle */}
        <div className={styles.group}>
          <AiModelPanel draft={draft} patch={patch} />
        </div>

        {/* AI Provider configuration */}
        <div className={styles.group}>
          <ProviderPanel />
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
                {storageDir || (lang === 'es' ? 'Cargando…' : 'Loading…')}
              </div>
            </div>
            <Button onClick={() => void changeStorageDir()}>
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
              <select
                aria-label={lang === 'es' ? 'Plantilla por defecto' : 'Default template'}
                value={draft.defaultTemplate}
                onChange={(e) => patch({ defaultTemplate: e.target.value })}
                style={{
                  padding: '7px 12px',
                  background: 'var(--bg-surface)',
                  border: '1px solid var(--stroke-strong)',
                  borderRadius: 'var(--radius)',
                  fontSize: 13,
                  color: 'inherit',
                  fontFamily: 'inherit',
                  minWidth: 200,
                }}
              >
                {templates.map((tpl) => (
                  <option key={tpl.id} value={tpl.id}>
                    {pickL(tpl.name, lang) || tpl.id}
                  </option>
                ))}
              </select>
            </div>
          </div>
        )}

        {/* Updates */}
        <div className={styles.group}>
          <div className={styles.groupHead}>{t('updateSection')}</div>
          <div className={styles.row}>
            <div className={styles.rowLeft}>
              <div className={styles.rowLabel}>{t('updateCheck')}</div>
              <div className={styles.rowDesc}>
                {updater.status.kind === 'checking' && t('updateChecking')}
                {updater.status.kind === 'upToDate' && t('updateUpToDate')}
                {updater.status.kind === 'error' && t('updateError')}
              </div>
            </div>
            <Button
              onClick={() => void updater.check()}
              loading={updater.status.kind === 'checking'}
            >
              {t('updateCheck')}
            </Button>
          </div>
          {(updater.status.kind === 'available' || updater.status.kind === 'downloading') && (
            <div className={styles.row}>
              <div className={styles.rowLeft}>
                <div className={styles.rowLabel}>
                  {updater.status.kind === 'available'
                    ? t('updateAvailable', { version: updater.status.version })
                    : t('updateDownloading')}
                </div>
              </div>
              <Button
                variant="primary"
                loading={updater.status.kind === 'downloading'}
                disabled={updater.status.kind === 'downloading'}
                onClick={() =>
                  updater.status.kind === 'available' && void updater.install(updater.status.update)
                }
              >
                {t('updateInstall')}
              </Button>
            </div>
          )}
        </div>

        <div className={styles.footer}>{'Smart Noter v1.0.0'}</div>
      </div>
    </div>
  );
}
