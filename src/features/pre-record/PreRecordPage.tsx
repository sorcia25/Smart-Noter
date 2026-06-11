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
import { useGetSettingsQuery } from '@/store/api/settings.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { pickL } from '@/utils/format';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useState } from 'react';
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

export default function PreRecordPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { t, lang } = useT();

  const { data: devices = [] } = useListAudioDevicesQuery();
  const { data: templates = [] } = useListTemplatesQuery();
  const { data: settings } = useGetSettingsQuery();

  const [deviceId, setDeviceId] = useState<string>('');
  const selectedDevice = devices.find((d) => d.id === deviceId);
  const previewMode: CaptureMode = selectedDevice?.kind === 'input' ? 'mic' : 'system';
  // The Settings-level Mix preference applies to the RECORDING only — preview stays
  // single-source (the level bar meters the selected device). Mix needs the selected
  // device to be a loopback: picking an input device is an explicit mic-only choice.
  const recordMode: CaptureMode =
    settings?.captureMode === 'mix' && previewMode === 'system' ? 'mix' : previewMode;

  const [templateId, setTemplateId] = useState<string>(searchParams.get('tpl') ?? 'tecnica');
  const [name, setName] = useState('');
  const [autoId, setAutoId] = useState(true);
  const [detectLang, setDetectLang] = useState(true);
  const [saveAudio, setSaveAudio] = useState(true);

  useEffect(() => {
    if (!deviceId && devices.length > 0) {
      const def = devices.find((d) => d.isDefault) ?? devices[0];
      if (def) setDeviceId(def.id);
    }
  }, [deviceId, devices]);

  useEffect(() => {
    if (!deviceId) return;
    let cancelled = false;
    invoke('start_preview', { deviceId, captureMode: previewMode }).catch((err) => {
      if (!cancelled) {
        const ae = toAppError(err);
        toast.error(t('audioErrorTitle'), {
          id: `audio-error:${ae.code}`,
          description: errorMessage(ae, t),
        });
      }
    });
    return () => {
      cancelled = true;
      void invoke('stop_preview');
    };
  }, [deviceId, previewMode, t]);

  function start() {
    navigate(Paths.LiveRecording(genSessionId()), {
      state: {
        name: name.trim() || (lang === 'es' ? 'Reunión sin título' : 'Untitled meeting'),
        templateId,
        deviceId,
        captureMode: recordMode,
        format: settings?.recordingQuality === 'FLAC' ? 'flac' : 'wav',
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
              {devices.map((d) => (
                <DeviceCard
                  key={d.id}
                  device={d}
                  selected={deviceId === d.id}
                  onSelect={() => setDeviceId(d.id)}
                />
              ))}
            </div>
            {recordMode === 'mix' && <div className={styles.modeHint}>{t('mixRecordHint')}</div>}
            {settings?.captureMode === 'mix' && recordMode === 'mic' && (
              <div className={styles.modeHint}>{t('mixOverrideHint')}</div>
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
                on={autoId}
                onChange={setAutoId}
              />
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
                disabled={!deviceId}
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
