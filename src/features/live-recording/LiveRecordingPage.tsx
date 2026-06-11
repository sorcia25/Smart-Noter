import { LivePill } from '@/components/domain/LivePill/LivePill';
import { TemplateIcon } from '@/components/domain/TemplateIcon/TemplateIcon';
import { Waveform } from '@/components/domain/Waveform/Waveform';
import { AvatarStack } from '@/components/primitives/Avatar/Avatar';
import { Icon, type IconName } from '@/components/primitives/Icon/Icon';
import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import type {
  AudioDeviceKind,
  AudioFormat,
  CaptureMode,
  CaptureResult,
  Participant,
  RecordingStartedDto,
} from '@/ipc/bindings';
import { errorMessage, toAppError } from '@/ipc/error';
import { useListAudioDevicesQuery } from '@/store/api/devices.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { fmtDuration, pickL } from '@/utils/format';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useMemo, useState } from 'react';
import { useLocation } from 'react-router-dom';
import styles from './LiveRecordingPage.module.css';
import { StopConfirmModal } from './StopConfirmModal/StopConfirmModal';

interface NavState {
  name?: string;
  templateId?: string;
  deviceId?: string;
  captureMode?: CaptureMode;
  format?: AudioFormat;
}

const DEFAULT_TEMPLATE_ID = 'tecnica';

const MOCK_SPEAKERS: Participant[] = [
  {
    id: 's1',
    meetingId: 'live',
    label: 'S1',
    name: null,
    colorClass: 's-color-1',
    wordCount: 0,
    talkPct: 0,
  },
  {
    id: 's2',
    meetingId: 'live',
    label: 'S2',
    name: null,
    colorClass: 's-color-2',
    wordCount: 0,
    talkPct: 0,
  },
  {
    id: 's3',
    meetingId: 'live',
    label: 'S3',
    name: null,
    colorClass: 's-color-3',
    wordCount: 0,
    talkPct: 0,
  },
];

export default function LiveRecordingPage() {
  const location = useLocation();
  const { t, lang } = useT();
  const navState = (location.state ?? {}) as NavState;

  const [elapsed, setElapsed] = useState(0);
  const [paused, setPaused] = useState(false);
  const [bars, setBars] = useState<number[]>(Array(36).fill(0));
  const [stopResult, setStopResult] = useState<CaptureResult | null>(null);
  const [stopModalOpen, setStopModalOpen] = useState(false);

  // 1. Start on mount, defensive discard on unmount
  // biome-ignore lint/correctness/useExhaustiveDependencies: navState is navigation state captured at mount; intentionally run only once
  useEffect(() => {
    let cancelled = false;
    invoke<RecordingStartedDto>('start_recording', {
      deviceId: navState.deviceId,
      captureMode: navState.captureMode ?? 'system',
      format: navState.format ?? 'wav',
    })
      .then(() => {
        if (cancelled) void invoke('discard_recording').catch(() => {});
      })
      .catch((err) => {
        /* start failures are invoke rejections (never audio:error events) — surface them here */
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
      void invoke('discard_recording').catch(() => {});
    };
  }, []);

  // 2. Subscribe to events
  useEffect(() => {
    let unw: (() => void) | null = null;
    let une: (() => void) | null = null;
    let cancelled = false;
    listen<{ bins: number[] }>('audio:waveform-bin', (e) => {
      if (!cancelled) setBars(e.payload.bins);
    })
      .then((fn) => {
        if (cancelled) fn();
        else unw = fn;
      })
      .catch(() => {}); // mirror of App.tsx M4 fix — suppress unhandled rejection on early unmount
    listen<{ elapsedSec: number }>('audio:elapsed', (e) => {
      if (!cancelled) setElapsed(e.payload.elapsedSec);
    })
      .then((fn) => {
        if (cancelled) fn();
        else une = fn;
      })
      .catch(() => {}); // mirror of App.tsx M4 fix — suppress unhandled rejection on early unmount
    return () => {
      cancelled = true;
      unw?.();
      une?.();
    };
  }, []);

  // Backend pause/resume are state-aware; benign double-click races resolve without rejecting.
  const onPauseToggle = async () => {
    try {
      if (paused) await invoke('resume_recording');
      else await invoke('pause_recording');
      setPaused(!paused);
    } catch (err) {
      const ae = toAppError(err);
      toast.error(t('audioErrorTitle'), {
        id: `audio-error:${ae.code}`,
        description: errorMessage(ae, t),
      });
    }
  };

  const onStop = async () => {
    try {
      const res = await invoke<CaptureResult>('stop_recording');
      setStopResult(res);
      setStopModalOpen(true);
    } catch (err) {
      const ae = toAppError(err);
      // NotRecording is the benign double-click race — second click after the
      // first already stopped the session. Suppress the toast for this case.
      if (ae.code !== 'NotRecording') {
        toast.error(t('audioErrorTitle'), {
          id: `audio-error:${ae.code}`,
          description: errorMessage(ae, t),
        });
      }
    }
  };

  const { data: devices = [] } = useListAudioDevicesQuery();
  const { data: templates = [] } = useListTemplatesQuery();

  const tmpl = useMemo(
    () =>
      templates.find((t) => t.id === navState.templateId) ??
      templates.find((t) => t.id === DEFAULT_TEMPLATE_ID) ??
      templates[0],
    [templates, navState.templateId]
  );

  const device =
    devices.find((d) => d.id === navState.deviceId) ??
    devices.find((d) => d.isDefault) ??
    devices[0];

  const iconFor = (kind: AudioDeviceKind): IconName =>
    kind === 'loopback' ? 'monitor' : 'headphones';

  const meetingName = navState.name ?? (lang === 'es' ? 'Reunión sin título' : 'Untitled meeting');

  return (
    <div className={styles.page} data-screen-label="04 Live recording">
      <div className={styles.header}>
        <div className={styles.headerLeft}>
          <LivePill paused={paused} />
          <div>
            <div className={styles.meetingName}>{meetingName}</div>
            {tmpl && (
              <div className={styles.tmplLine}>
                <TemplateIcon templateId={tmpl.id} size={14} />
                <span>{pickL(tmpl.name, lang)}</span>
              </div>
            )}
          </div>
        </div>
        <div className={styles.headerRight}>
          <div className={styles.engine}>
            <div className={styles.engineDot} />
            <span>{t('transcriptionEngine')}</span>
          </div>
        </div>
      </div>

      <div className={styles.stage}>
        <div className={styles.center}>
          <div className={styles.timer}>{fmtDuration(elapsed)}</div>
          <div className={styles.status}>
            {paused
              ? lang === 'es'
                ? 'Pausado'
                : 'Paused'
              : `${t('speaking')} — ${lang === 'es' ? 'Sujeto 2' : 'Subject 2'}`}
          </div>
          <Waveform paused={paused} bars={36} externalBins={bars} />
          <div className={styles.controls}>
            <button
              type="button"
              className={styles.ctrlBtn}
              onClick={onPauseToggle}
              title={paused ? t('play') : t('livePauseHint')}
              aria-label={paused ? 'Resume' : 'Pause'}
            >
              <Icon name={paused ? 'play' : 'pause'} size={22} />
            </button>
            <button
              type="button"
              className={`${styles.ctrlBtn} ${styles.ctrlStop}`}
              onClick={onStop}
              title={t('liveStopHint')}
              aria-label="Stop"
            >
              <Icon name="stop" size={22} stroke="white" />
            </button>
            <button
              type="button"
              className={styles.ctrlBtn}
              title="Flag"
              aria-label="Flag"
              disabled
            >
              <Icon name="flag" size={18} />
            </button>
          </div>
        </div>
      </div>

      {stopResult && (
        <StopConfirmModal
          open={stopModalOpen}
          onClose={() => setStopModalOpen(false)}
          capture={stopResult}
          suggestedTitle={navState.name ?? ''}
          templateId={navState.templateId ?? DEFAULT_TEMPLATE_ID}
        />
      )}

      <div className={styles.meta}>
        <div className={styles.metaBlock}>
          <Icon name={device ? iconFor(device.kind) : 'monitor'} size={14} />
          <span>{t('sourceLabel')}:</span>
          <span className={styles.metaStrong}>
            {device ? device.name : lang === 'es' ? 'Sin dispositivo' : 'No device'}
          </span>
        </div>
        <div className={styles.metaBlock}>
          <Icon name="user" size={14} />
          <span>{t('speakersDetected')}:</span>
          <AvatarStack participants={MOCK_SPEAKERS} size={22} max={5} />
        </div>
        <div className={styles.metaBlock}>
          <Icon name="globe" size={14} />
          <span>{'ES ·'}</span>
          <span className={styles.metaSubtle}>{'auto'}</span>
        </div>
        <div className={styles.metaBlock}>
          <Icon name="shield" size={14} />
          <span>{lang === 'es' ? 'Local · cifrado' : 'Local · encrypted'}</span>
        </div>
      </div>
    </div>
  );
}
