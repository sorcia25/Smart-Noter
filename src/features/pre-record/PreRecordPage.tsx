import { EqBar } from '@/components/domain/EqBar/EqBar';
import { LevelBar } from '@/components/domain/LevelBar/LevelBar';
import { TemplateIcon } from '@/components/domain/TemplateIcon/TemplateIcon';
import { Button } from '@/components/primitives/Button/Button';
import { Chip } from '@/components/primitives/Chip/Chip';
import { Icon, type IconName } from '@/components/primitives/Icon/Icon';
import { Input } from '@/components/primitives/Input/Input';
import { toast } from '@/components/primitives/Toast/Toast';
import { Toggle } from '@/components/primitives/Toggle/Toggle';
import { useT } from '@/i18n/useT';
import type { AudioDevice, AudioDeviceKind, CaptureMode, Template } from '@/ipc/bindings';
import { errorMessage, toAppError } from '@/ipc/error';
import { Paths } from '@/router/paths';
import { useListAudioDevicesQuery } from '@/store/api/devices.api';
import { useGetSettingsQuery, useUpdateSettingsMutation } from '@/store/api/settings.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { pickL } from '@/utils/format';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import styles from './PreRecordPage.module.css';

function genSessionId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return `sess-${crypto.randomUUID()}`;
  }
  return `sess-${Date.now()}-${Math.floor(Math.random() * 1e6)}`;
}

const iconFor = (kind: AudioDeviceKind): IconName =>
  kind === 'loopback' ? 'monitor' : 'headphones';

// Virtual card id for the Mix option — not a real AudioDevice.id from the backend.
const MIX_CARD_ID = '__mix__';

// Sentinel device id: tells the backend to resolve the CURRENT default render
// endpoint at stream-open time, instead of pinning whatever device happened to
// be the default when this page loaded. Must match `stream.rs::DEFAULT_RENDER_LOOPBACK`.
// v1.0.1 F3: without this, switching Windows' default output (e.g. speakers →
// headphones) between page load and recording start left the Mix card capturing
// a silent endpoint nothing renders to.
const DEFAULT_RENDER_LOOPBACK = '__default_render__';

export default function PreRecordPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { t, lang } = useT();

  // v1.0.1 F3: refetch on mount — a cached list can serve stale `isDefault`
  // flags after the user switches Windows' default output/input device between
  // visits to this page, which would silently mis-seed the auto-select effect
  // below (and, before the sentinel fix, pin a dead loopback endpoint).
  const { data: devices = [] } = useListAudioDevicesQuery(undefined, {
    refetchOnMountOrArgChange: true,
  });
  const { data: templates = [] } = useListTemplatesQuery();
  const { data: settings, isSuccess: settingsLoaded } = useGetSettingsQuery();
  const [updateSettings] = useUpdateSettingsMutation();

  const [deviceId, setDeviceId] = useState<string>('');
  const [micDeviceId, setMicDeviceId] = useState<string | null>(null);
  // Native Windows AEC (v1.2): the OS cancels speaker echo in Mix mode when on.
  // Default off until the ship commit flips the persisted default (post-smoke).
  const [aecEnabled, setAecEnabled] = useState(false);
  useEffect(() => {
    if (settings) setAecEnabled(settings.aecEnabled ?? false);
  }, [settings]);
  const isMix = deviceId === MIX_CARD_ID;
  const selectedDevice = devices.find((d) => d.id === deviceId);
  // Used ONLY to gate the Mix card's disabled state (no render device at all →
  // Mix can't work). NOT used to pick a device to preview/record — that's the
  // DEFAULT_RENDER_LOOPBACK sentinel below, resolved by the backend at stream-open
  // time so it always reflects the CURRENT default, not this (possibly stale) pick.
  const defaultLoopback =
    devices.find((d) => d.kind === 'loopback' && d.isDefault) ??
    devices.find((d) => d.kind === 'loopback');
  const inputDevices = devices.filter((d) => d.kind === 'input');
  // The mix card previews (and records) the system loopback lane; the mic lane
  // has no preview. Single devices preview themselves.
  const previewDeviceId = isMix ? DEFAULT_RENDER_LOOPBACK : deviceId;
  const previewMode: CaptureMode = !isMix && selectedDevice?.kind === 'input' ? 'mic' : 'system';
  const recordMode: CaptureMode = isMix ? 'mix' : previewMode;

  const [templateId, setTemplateId] = useState<string>(searchParams.get('tpl') ?? '');
  const [name, setName] = useState('');
  const [speakerHint, setSpeakerHint] = useState<number | null>(null);
  const [detectLang, setDetectLang] = useState(true);
  const [saveAudio, setSaveAudio] = useState(true);

  // Seed the template from Settings' default once, unless a ?tpl= param was passed.
  const tplInitialized = useRef(false);
  useEffect(() => {
    if (tplInitialized.current) return;
    if (searchParams.get('tpl')) {
      tplInitialized.current = true;
      return;
    }
    if (settings) {
      setTemplateId(settings.defaultTemplate || 'tecnica');
      tplInitialized.current = true;
    }
  }, [settings, searchParams]);

  // Seed the initial device selection once devices AND settings are both loaded: honor
  // the Settings-level Mix preference by preselecting the mix card, otherwise fall back
  // to the default device — mirrors the tplInitialized once-guard above.
  //
  // Guarded by a ref rather than `!deviceId`, and also flipped by selectDevice() below:
  // list_audio_devices and get_settings resolve independently, so a user can click a
  // card before this effect ever runs. Without the manual-selection flag here, the
  // effect would fire once settings finally arrives and clobber that manual choice.
  const deviceInitialized = useRef(false);
  const selectDevice = (id: string) => {
    deviceInitialized.current = true;
    setDeviceId(id);
  };
  useEffect(() => {
    if (deviceInitialized.current) return;
    if (devices.length === 0 || !settingsLoaded) return;
    if (settings?.captureMode === 'mix') {
      setDeviceId(MIX_CARD_ID);
    } else {
      const def = devices.find((d) => d.isDefault) ?? devices[0];
      if (def) setDeviceId(def.id);
    }
    deviceInitialized.current = true;
  }, [devices, settings, settingsLoaded]);

  // `t` is only used inside the error-toast path; this codebase's `t` is
  // referentially unstable across re-renders (see LiveRecordingPage), and having
  // it in the deps makes background re-renders (e.g. a transcription finishing)
  // restart the preview — racing the un-awaited stop_preview and toasting
  // AlreadyRecording. Re-run only on real device/mode changes.
  // biome-ignore lint/correctness/useExhaustiveDependencies: t is unstable; effect must re-run only on device/mode change
  useEffect(() => {
    if (!previewDeviceId) return;
    let cancelled = false;
    invoke('start_preview', { deviceId: previewDeviceId, captureMode: previewMode }).catch(
      (err) => {
        if (!cancelled) {
          const ae = toAppError(err);
          toast.error(t('audioErrorTitle'), {
            id: `audio-error:${ae.code}`,
            description: errorMessage(ae, t),
          });
        }
      }
    );
    return () => {
      cancelled = true;
      void invoke('stop_preview');
    };
  }, [previewDeviceId, previewMode]);

  const speakerIdOn = settings?.identifySpeakers ?? true;

  function start() {
    navigate(Paths.LiveRecording(genSessionId()), {
      state: {
        name: name.trim() || (lang === 'es' ? 'Reunión sin título' : 'Untitled meeting'),
        templateId,
        deviceId: isMix ? DEFAULT_RENDER_LOOPBACK : deviceId,
        captureMode: recordMode,
        micDeviceId: isMix ? micDeviceId : null,
        aecEnabled: isMix ? aecEnabled : false,
        format: settings?.recordingQuality === 'FLAC' ? 'flac' : 'wav',
        speakerHint: speakerIdOn ? speakerHint : null,
      },
    });
  }

  return (
    <div className={styles.page} data-screen-label="03 Pre-record">
      <div className={styles.header}>
        <button type="button" className={styles.back} onClick={() => navigate(Paths.Dashboard)}>
          <Icon name="chevLeft" size={14} />
          {lang === 'es' ? 'Volver' : 'Back'}
        </button>
        <h1 className={styles.title}>{t('preTitle')}</h1>
        <div className={styles.sub}>{t('preSub')}</div>
      </div>
      <div className={styles.scroll}>
        <div className={styles.body}>
          <div className={styles.section}>
            <Input
              label={t('meetingNameLabel')}
              placeholder={t('meetingNamePh')}
              value={name}
              onChange={(e) => setName(e.target.value)}
              style={{ fontSize: 14, padding: '11px 14px' }}
            />
          </div>

          <div className={styles.section}>
            <h2 className={styles.sectionTitle}>{t('deviceSection')}</h2>
            <div className={styles.sectionHint}>{t('deviceHint')}</div>
            <div className={styles.grid2}>
              <MixCard
                selected={isMix}
                disabled={!defaultLoopback}
                onSelect={() => selectDevice(MIX_CARD_ID)}
              />
              {devices.map((d) => (
                <DeviceCard
                  key={d.id}
                  device={d}
                  selected={deviceId === d.id}
                  onSelect={() => selectDevice(d.id)}
                />
              ))}
            </div>
            {isMix && (
              <>
                <div className={styles.micPickerRow}>
                  <label htmlFor="mix-mic">{t('mixMicLabel')}</label>
                  <select
                    id="mix-mic"
                    value={micDeviceId ?? ''}
                    onChange={(e) => setMicDeviceId(e.target.value === '' ? null : e.target.value)}
                  >
                    <option value="">{t('mixMicDefault')}</option>
                    {inputDevices.map((d) => (
                      <option key={d.id} value={d.id}>
                        {d.name}
                      </option>
                    ))}
                  </select>
                </div>
                <label className={styles.aecToggleRow}>
                  <input
                    type="checkbox"
                    checked={aecEnabled}
                    onChange={(e) => {
                      const v = e.target.checked;
                      setAecEnabled(v);
                      if (settings) void updateSettings({ ...settings, aecEnabled: v });
                    }}
                  />
                  <span>{t('aecToggleLabel')}</span>
                </label>
                <div className={styles.modeHint}>{t('aecToggleHint')}</div>
                <div className={styles.modeHint}>{t('mixHeadphonesHint')}</div>
              </>
            )}
            <AudioPreviewCard />
          </div>

          <div className={styles.section}>
            <h2 className={styles.sectionTitle}>{t('templateSection')}</h2>
            <div className={styles.sectionHint}>{t('templateHint')}</div>
            <div className={styles.tmplGrid}>
              {templates.map((tpl) => (
                <TemplateCard
                  key={tpl.id}
                  template={tpl}
                  selected={templateId === tpl.id}
                  onSelect={() => setTemplateId(tpl.id)}
                />
              ))}
            </div>
          </div>

          <div className={styles.section}>
            <h2 className={styles.sectionTitle}>{t('advancedSection')}</h2>
            <div className={styles.advCard}>
              <SettingRow
                label={t('autoIdSpeakers')}
                desc={t('autoIdSpeakersDesc')}
                on={speakerIdOn}
                onChange={(v) => {
                  if (settings) void updateSettings({ ...settings, identifySpeakers: v });
                }}
              />
              {speakerIdOn && (
                <label className={styles.hintRow}>
                  {t('diarize.expectedCount')}
                  <input
                    type="number"
                    min={1}
                    max={8}
                    value={speakerHint ?? ''}
                    onChange={(e) =>
                      setSpeakerHint(e.target.value === '' ? null : Number(e.target.value))
                    }
                    placeholder="auto"
                  />
                </label>
              )}
              <SettingRow
                label={t('detectLang')}
                desc={t('detectLangDesc')}
                on={detectLang}
                onChange={setDetectLang}
              />
              <SettingRow
                label={t('saveAudio')}
                desc={t('saveAudioDesc')}
                on={saveAudio}
                onChange={setSaveAudio}
              />
            </div>
          </div>

          <div className={styles.footer}>
            <div className={styles.privacy}>
              <Icon name="shield" size={16} stroke="var(--text-muted)" />
              <span>
                {lang === 'es'
                  ? 'Procesamiento 100% local. Tu audio nunca sale de tu PC.'
                  : '100% local processing. Your audio never leaves your PC.'}
              </span>
            </div>
            <div className={styles.footerActions}>
              <Button variant="default" onClick={() => navigate(Paths.Dashboard)}>
                {t('cancel')}
              </Button>
              <Button
                variant="primary"
                icon={<Icon name="record" size={12} />}
                onClick={start}
                disabled={!deviceId || (isMix && !defaultLoopback)}
              >
                {t('startRecording')}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function DeviceCard({
  device,
  selected,
  onSelect,
}: {
  device: AudioDevice;
  selected: boolean;
  onSelect: () => void;
}) {
  const { lang } = useT();
  return (
    <button
      type="button"
      className={`${styles.optCard} ${selected ? styles.optCardSelected : ''}`}
      onClick={onSelect}
    >
      <div className={styles.iconBox}>
        <Icon name={iconFor(device.kind)} size={18} />
      </div>
      <div className={styles.optMeta}>
        <div className={styles.optName}>
          <span>{device.name}</span>
          {device.recommended && (
            <Chip variant="accent" disabled>
              {lang === 'es' ? 'Recomendado' : 'Recommended'}
            </Chip>
          )}
        </div>
        <div className={styles.optDesc}>
          {device.sampleRate / 1000}kHz · {device.channels === 1 ? 'mono' : 'stereo'}
        </div>
      </div>
      <div className={styles.radio} />
    </button>
  );
}

function MixCard({
  selected,
  disabled,
  onSelect,
}: {
  selected: boolean;
  disabled: boolean;
  onSelect: () => void;
}) {
  const { t, lang } = useT();
  return (
    <button
      type="button"
      className={`${styles.optCard} ${styles.mixCard} ${selected ? styles.optCardSelected : ''}`}
      onClick={onSelect}
      disabled={disabled}
    >
      <div className={styles.iconBox}>
        <Icon name="monitor" size={18} />
      </div>
      <div className={styles.optMeta}>
        <div className={styles.optName}>
          <span>{t('mixCardTitle')}</span>
          <Chip variant="accent" disabled>
            {lang === 'es' ? 'Recomendado' : 'Recommended'}
          </Chip>
        </div>
        <div className={styles.optDesc}>{t('mixCardDesc')}</div>
      </div>
      <div className={styles.radio} />
    </button>
  );
}

function TemplateCard({
  template,
  selected,
  onSelect,
}: {
  template: Template;
  selected: boolean;
  onSelect: () => void;
}) {
  const { lang } = useT();
  return (
    <button
      type="button"
      className={`${styles.tmplCard} ${selected ? styles.tmplCardSelected : ''}`}
      onClick={onSelect}
    >
      {selected && (
        <div className={styles.tmplCheck}>
          <Icon name="check" size={12} stroke="white" />
        </div>
      )}
      <div className={styles.tmplIconWrap}>
        <TemplateIcon templateId={template.id} size={36} />
      </div>
      <div className={styles.tmplName}>{pickL(template.name, lang)}</div>
      <div className={styles.tmplDesc}>{pickL(template.desc, lang)}</div>
    </button>
  );
}

function AudioPreviewCard() {
  const { lang } = useT();
  const [level, setLevel] = useState(0);
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    listen<{ rms: number; peak: number }>('audio:level', (e) => {
      if (!cancelled) setLevel(e.payload.rms);
    })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {}); // mirror of App.tsx M4 fix — suppress unhandled rejection on early unmount
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return (
    <div className={styles.previewCard}>
      <div className={styles.previewHead}>
        <div>
          <div className={styles.previewLabel}>
            {lang === 'es' ? 'Vista previa del audio' : 'Audio preview'}
          </div>
          <div className={styles.previewSub}>
            {lang === 'es'
              ? 'Reproduce algo en tu PC para verificar la señal'
              : 'Play something on your PC to check the signal'}
          </div>
        </div>
        <EqBar bars={8} />
      </div>
      <LevelBar level={level} />
    </div>
  );
}

function SettingRow({
  label,
  desc,
  on,
  onChange,
}: {
  label: string;
  desc: string;
  on: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <div className={styles.settingRow}>
      <div>
        <div className={styles.settingLabel}>{label}</div>
        <div className={styles.settingDesc}>{desc}</div>
      </div>
      <Toggle on={on} onChange={onChange} aria-label={label} />
    </div>
  );
}
