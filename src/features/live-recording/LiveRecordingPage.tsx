import { TemplateIcon } from '@/components/domain/TemplateIcon/TemplateIcon';
import { AvatarStack } from '@/components/primitives/Avatar/Avatar';
import { Icon, type IconName } from '@/components/primitives/Icon/Icon';
import { useT } from '@/i18n/useT';
import type { Participant } from '@/ipc/bindings';
import { Paths } from '@/router/paths';
import { useListAudioDevicesQuery } from '@/store/api/devices.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { fmtDuration, pickL } from '@/utils/format';
import { useMemo, useRef } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import styles from './LiveRecordingPage.module.css';
import { useLiveTimer } from './useLiveTimer';

interface NavState {
  name?: string;
  templateId?: string;
}

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
  const navigate = useNavigate();
  const location = useLocation();
  const { t, lang } = useT();
  const navState = (location.state ?? {}) as NavState;

  const { elapsed, paused, togglePause } = useLiveTimer(143); // start at 2:23 like the prototype

  const { data: devices = [] } = useListAudioDevicesQuery();
  const { data: templates = [] } = useListTemplatesQuery();

  // Stable waveform heights for this session.
  const barsRef = useRef<number[] | null>(null);
  if (!barsRef.current) {
    barsRef.current = Array.from({ length: 36 }, () => 0.25 + Math.random() * 0.75);
  }
  const bars = barsRef.current;

  const tmpl = useMemo(
    () =>
      templates.find((t) => t.id === navState.templateId) ??
      templates.find((t) => t.id === 'tecnica') ??
      templates[0],
    [templates, navState.templateId]
  );

  const device =
    devices.find((d) => d.id === 'system-loopback') ?? devices.find((d) => d.active) ?? devices[0];
  const deviceIcon: IconName = (device?.icon as IconName) ?? 'monitor';

  const meetingName = navState.name ?? (lang === 'es' ? 'Reunión sin título' : 'Untitled meeting');

  return (
    <div className={styles.page} data-screen-label="04 Live recording">
      <div className={styles.header}>
        <div className={styles.headerLeft}>
          <div className={styles.pill}>
            <div className={`${styles.recDot} ${paused ? styles.recDotPaused : ''}`} />
            {paused ? (lang === 'es' ? 'PAUSADO' : 'PAUSED') : t('liveStatus')}
          </div>
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
          <div className={`${styles.waveform} ${paused ? styles.waveformPaused : ''}`}>
            {bars.map((b, i) => (
              <span
                // biome-ignore lint/suspicious/noArrayIndexKey: bars are positional and never reorder
                key={i}
                style={{
                  height: `${Math.round((paused ? 0.2 : b) * 100)}%`,
                  animationDelay: `${(i * 60) % 1200}ms`,
                  opacity: paused ? 0.3 : 1,
                }}
              />
            ))}
          </div>
          <div className={styles.controls}>
            <button
              type="button"
              className={styles.ctrlBtn}
              onClick={togglePause}
              title={paused ? t('play') : t('livePauseHint')}
              aria-label={paused ? 'Resume' : 'Pause'}
            >
              <Icon name={paused ? 'play' : 'pause'} size={22} />
            </button>
            <button
              type="button"
              className={`${styles.ctrlBtn} ${styles.ctrlStop}`}
              onClick={() => navigate(Paths.MeetingDetail('m-001'))}
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

      <div className={styles.meta}>
        <div className={styles.metaBlock}>
          <Icon name={deviceIcon} size={14} />
          <span>{t('sourceLabel')}:</span>
          <span className={styles.metaStrong}>
            {device ? pickL(device.name, lang) : lang === 'es' ? 'Sin dispositivo' : 'No device'}
          </span>
        </div>
        <div className={styles.metaBlock}>
          <Icon name="user" size={14} />
          <span>{t('speakersDetected')}:</span>
          <AvatarStack participants={MOCK_SPEAKERS} size={22} max={5} />
        </div>
        <div className={styles.metaBlock}>
          <Icon name="globe" size={14} />
          <span>ES ·</span>
          <span className={styles.metaSubtle}>auto</span>
        </div>
        <div className={styles.metaBlock}>
          <Icon name="shield" size={14} />
          <span>{lang === 'es' ? 'Local · cifrado' : 'Local · encrypted'}</span>
        </div>
      </div>
    </div>
  );
}
