import { Button } from '@/components/primitives/Button/Button';
import { Chip } from '@/components/primitives/Chip/Chip';
import { Icon, type IconName } from '@/components/primitives/Icon/Icon';
import { SegmentedControl } from '@/components/primitives/SegmentedControl/SegmentedControl';
import { useT } from '@/i18n/useT';
import type { MeetingDetail } from '@/ipc/bindings';
import { fmtDuration } from '@/utils/format';
import { useMemo } from 'react';
import styles from './AudioTab.module.css';

const PROGRESS = 0.32;
type Speed = '0.5x' | '1x' | '1.5x' | '2x';

const SPEED_OPTIONS = [
  { value: '0.5x' as Speed, label: '0.5×' },
  { value: '1x' as Speed, label: '1×' },
  { value: '1.5x' as Speed, label: '1.5×' },
  { value: '2x' as Speed, label: '2×' },
];

export interface AudioTabProps {
  meeting: MeetingDetail;
}

export function AudioTab({ meeting }: AudioTabProps) {
  const { t, lang } = useT();

  const bars = useMemo(() => {
    // Deterministic synthetic waveform per meeting (seeded by id length so it's stable)
    const seed = meeting.id.length;
    return Array.from({ length: 120 }, (_, i) => {
      const sine = 0.2 + Math.sin(i / 4) * 0.3;
      const noise = ((seed * (i + 1)) % 100) / 200;
      return Math.max(0.08, Math.min(0.95, sine + noise));
    });
  }, [meeting.id]);

  const markers: { t: string; txt: string; icon: IconName }[] = [
    {
      t: '00:01:24',
      txt: lang === 'es' ? 'Decisión: agendar sesión con SAP' : 'Decision: schedule SAP session',
      icon: 'check',
    },
    {
      t: '00:01:42',
      txt: lang === 'es' ? 'Confirmación de Go-Live para 18 dic' : 'Go-Live confirmed for Dec 18',
      icon: 'flag',
    },
    {
      t: '00:03:05',
      txt: lang === 'es' ? 'Acción: contratar consultor SAP' : 'Action: hire SAP consultant',
      icon: 'zap',
    },
  ];

  const playedSec = Math.floor(meeting.durationSec * PROGRESS);

  return (
    <div>
      <div className={styles.card}>
        <div className={styles.head}>
          <div className={styles.headLeft}>
            <Icon name="mic" size={14} stroke="var(--accent)" />
            <span>{t('audio')}</span>
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <Chip disabled>WAV · 48 kHz</Chip>
            <Chip disabled>47.2 MB</Chip>
          </div>
        </div>
        <div className={styles.waveform}>
          {bars.map((b, i) => {
            const isPlayed = i / bars.length < PROGRESS;
            return (
              <div
                // biome-ignore lint/suspicious/noArrayIndexKey: bars are positional and never reorder
                key={i}
                className={`${styles.bar} ${isPlayed ? styles.barPlayed : styles.barUnplayed}`}
                style={{ height: `${Math.round(b * 100)}%` }}
              />
            );
          })}
        </div>
        <div className={styles.controls}>
          <Button size="icon" disabled>
            <Icon name="back" size={14} />
          </Button>
          <button
            type="button"
            className={styles.playBtn}
            aria-label={t('play')}
            disabled
            title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
          >
            <Icon name="play" size={18} stroke="currentColor" />
          </button>
          <Button size="icon" disabled>
            <Icon name="forward" size={14} />
          </Button>
          <span className={styles.time}>
            {fmtDuration(playedSec)} / {fmtDuration(meeting.durationSec)}
          </span>
          <div className={styles.flex1} />
          <SegmentedControl<Speed> value="1x" options={SPEED_OPTIONS} onChange={() => {}} />
          <Button size="icon" disabled>
            <Icon name="download" size={14} />
          </Button>
        </div>
      </div>
      <div className={styles.card}>
        <h3 className={styles.markersHead}>{lang === 'es' ? 'Marcadores' : 'Markers'}</h3>
        <div className={styles.markersSub}>
          {lang === 'es'
            ? 'Puntos importantes detectados automáticamente.'
            : 'Important points detected automatically.'}
        </div>
        {markers.map((m) => (
          <div className={styles.markerRow} key={m.t}>
            <span className={styles.markerTime}>{m.t}</span>
            <Icon name={m.icon} size={14} stroke="var(--accent)" />
            <span style={{ fontSize: 13 }}>{m.txt}</span>
            <div className={styles.flex1} />
            <Button size="icon" variant="ghost" disabled>
              <Icon name="play" size={12} />
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}
