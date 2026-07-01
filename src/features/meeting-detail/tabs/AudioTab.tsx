import { Button } from '@/components/primitives/Button/Button';
import { Chip } from '@/components/primitives/Chip/Chip';
import { Icon } from '@/components/primitives/Icon/Icon';
import type { IconName } from '@/components/primitives/Icon/icons';
import { SegmentedControl } from '@/components/primitives/SegmentedControl/SegmentedControl';
import { useT } from '@/i18n/useT';
import type { Marker, MeetingAudioInfo, MeetingDetail } from '@/ipc/bindings';
import {
  useCreateMarkerMutation,
  useDeleteMarkerMutation,
  useListMarkersQuery,
} from '@/store/api/markers.api';
import { fmtDuration } from '@/utils/format';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { useEffect, useMemo, useRef, useState } from 'react';
import styles from './AudioTab.module.css';

type Speed = '0.5x' | '1x' | '1.5x' | '2x';

const SPEED_OPTIONS = [
  { value: '0.5x' as Speed, label: '0.5×' },
  { value: '1x' as Speed, label: '1×' },
  { value: '1.5x' as Speed, label: '1.5×' },
  { value: '2x' as Speed, label: '2×' },
];

interface KindMeta {
  label: { es: string; en: string };
  icon: IconName;
  color: string;
}

const KIND_META: Record<string, KindMeta> = {
  decision: { label: { es: 'Decisión', en: 'Decision' }, icon: 'check', color: 'var(--accent)' },
  action: { label: { es: 'Acción', en: 'Action' }, icon: 'zap', color: '#c99a2e' },
  blocker: { label: { es: 'Bloqueo', en: 'Blocker' }, icon: 'flag', color: '#d1453b' },
  highlight: { label: { es: 'Destacado', en: 'Highlight' }, icon: 'sparkles', color: '#7c5cff' },
  manual: { label: { es: 'Manual', en: 'Manual' }, icon: 'pin', color: 'var(--text-muted)' },
};

const FALLBACK_KIND_META: KindMeta = {
  label: { es: 'Manual', en: 'Manual' },
  icon: 'pin',
  color: 'var(--text-muted)',
};

function kindMeta(kind: string): KindMeta {
  return KIND_META[kind] ?? FALLBACK_KIND_META;
}

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

function formatLabel(path: string): string {
  return path.toLowerCase().endsWith('.flac') ? 'FLAC' : 'WAV';
}

export interface AudioTabProps {
  meeting: MeetingDetail;
  onExport: () => void;
}

export function AudioTab({ meeting, onExport }: AudioTabProps) {
  const { t, lang } = useT();
  const audioRef = useRef<HTMLAudioElement>(null);
  // undefined = loading, null = no audio saved, object = ready
  const [info, setInfo] = useState<MeetingAudioInfo | null | undefined>(undefined);
  const [playing, setPlaying] = useState(false);
  const [current, setCurrent] = useState(0);
  const [duration, setDuration] = useState(meeting.durationSec);
  const [speed, setSpeed] = useState<Speed>('1x');

  // Decorative stable waveform (seeded by id). The bar heights are cosmetic; the
  // played/seek behaviour is real — driven by the <audio> element's currentTime.
  const bars = useMemo(() => {
    const seed = meeting.id.length;
    return Array.from({ length: 120 }, (_, i) => {
      const sine = 0.2 + Math.sin(i / 4) * 0.3;
      const noise = ((seed * (i + 1)) % 100) / 200;
      return Math.max(0.08, Math.min(0.95, sine + noise));
    });
  }, [meeting.id]);

  useEffect(() => {
    let cancelled = false;
    invoke<MeetingAudioInfo | null>('get_meeting_audio', { meetingId: meeting.id })
      .then((res) => {
        if (!cancelled) setInfo(res ?? null);
      })
      .catch(() => {
        if (!cancelled) setInfo(null);
      });
    return () => {
      cancelled = true;
    };
  }, [meeting.id]);

  const src = info ? convertFileSrc(info.path) : undefined;

  useEffect(() => {
    if (audioRef.current) audioRef.current.playbackRate = Number.parseFloat(speed);
  }, [speed]);

  const progress = duration > 0 ? current / duration : 0;

  function togglePlay() {
    const el = audioRef.current;
    if (!el) return;
    if (el.paused) void el.play();
    else el.pause();
  }

  function skip(delta: number) {
    const el = audioRef.current;
    if (!el) return;
    el.currentTime = Math.max(0, Math.min(duration, el.currentTime + delta));
  }

  function seekToFraction(fraction: number) {
    const el = audioRef.current;
    if (!el || duration <= 0) return;
    el.currentTime = Math.max(0, Math.min(duration, fraction * duration));
  }

  function seekToSeconds(tSeconds: number) {
    if (audioRef.current) audioRef.current.currentTime = Math.min(tSeconds, duration);
  }

  const { data: markers } = useListMarkersQuery(meeting.id);
  const [createMarker] = useCreateMarkerMutation();
  const [deleteMarker] = useDeleteMarkerMutation();

  function markHere() {
    const tSeconds = Math.floor(audioRef.current?.currentTime ?? 0);
    void createMarker({ meetingId: meeting.id, tSeconds, label: '' });
  }

  const playLabel = lang === 'es' ? 'Reproducir' : 'Play';
  const pauseLabel = lang === 'es' ? 'Pausar' : 'Pause';

  return (
    <div>
      <div className={styles.card}>
        <div className={styles.head}>
          <div className={styles.headLeft}>
            <Icon name="mic" size={14} stroke="var(--accent)" />
            <span>{t('audio')}</span>
          </div>
          {info && (
            <div style={{ display: 'flex', gap: 8 }}>
              <Chip disabled>{formatLabel(info.path)}</Chip>
              <Chip disabled>{fmtBytes(info.sizeBytes)}</Chip>
            </div>
          )}
        </div>

        {info === undefined && (
          <div className={styles.empty}>{lang === 'es' ? 'Cargando audio…' : 'Loading audio…'}</div>
        )}
        {info === null && (
          <div className={styles.empty}>
            {lang === 'es'
              ? 'No se guardó audio para esta reunión.'
              : 'No audio was saved for this meeting.'}
          </div>
        )}

        {info && (
          <>
            {/* biome-ignore lint/a11y/useMediaCaption: a meeting recording has no caption track */}
            <audio
              ref={audioRef}
              src={src}
              preload="metadata"
              onLoadedMetadata={(e) => {
                e.currentTarget.playbackRate = Number.parseFloat(speed);
                const d = e.currentTarget.duration;
                if (Number.isFinite(d) && d > 0) setDuration(d);
              }}
              onTimeUpdate={(e) => setCurrent(e.currentTarget.currentTime)}
              onPlay={() => setPlaying(true)}
              onPause={() => setPlaying(false)}
              onEnded={() => setPlaying(false)}
            />
            <button
              type="button"
              className={styles.waveform}
              aria-label={lang === 'es' ? 'Buscar en el audio' : 'Seek audio'}
              onClick={(e) => {
                const r = e.currentTarget.getBoundingClientRect();
                seekToFraction((e.clientX - r.left) / r.width);
              }}
            >
              {bars.map((b, i) => {
                const isPlayed = i / bars.length < progress;
                return (
                  <div
                    // biome-ignore lint/suspicious/noArrayIndexKey: bars are positional and never reorder
                    key={i}
                    className={`${styles.bar} ${isPlayed ? styles.barPlayed : styles.barUnplayed}`}
                    style={{ height: `${Math.round(b * 100)}%` }}
                  />
                );
              })}
            </button>
            <div className={styles.controls}>
              <Button size="icon" onClick={() => skip(-10)} aria-label="-10s">
                <Icon name="back" size={14} />
              </Button>
              <button
                type="button"
                className={styles.playBtn}
                aria-label={playing ? pauseLabel : playLabel}
                onClick={togglePlay}
              >
                <Icon name={playing ? 'pause' : 'play'} size={18} stroke="currentColor" />
              </button>
              <Button size="icon" onClick={() => skip(10)} aria-label="+10s">
                <Icon name="forward" size={14} />
              </Button>
              <span className={styles.time}>
                {fmtDuration(Math.floor(current))} / {fmtDuration(Math.floor(duration))}
              </span>
              <div className={styles.flex1} />
              <SegmentedControl<Speed> value={speed} options={SPEED_OPTIONS} onChange={setSpeed} />
              <Button
                size="icon"
                onClick={onExport}
                aria-label={lang === 'es' ? 'Descargar / exportar' : 'Download / export'}
                title={lang === 'es' ? 'Descargar / exportar' : 'Download / export'}
              >
                <Icon name="download" size={14} />
              </Button>
            </div>
          </>
        )}
      </div>

      <div className={styles.card}>
        <div className={styles.head}>
          <div className={styles.headLeft}>
            <Icon name="pin" size={14} stroke="var(--accent)" />
            <span>{lang === 'es' ? 'Marcadores' : 'Markers'}</span>
          </div>
          <button type="button" className={styles.markBtn} onClick={markHere}>
            <Icon name="plus" size={13} />
            <span>{lang === 'es' ? 'Marcar aquí' : 'Mark here'}</span>
          </button>
        </div>

        {!markers || markers.length === 0 ? (
          <div className={styles.empty}>
            {lang === 'es' ? 'Sin marcadores aún.' : 'No markers yet.'}
          </div>
        ) : (
          <div>
            {markers.map((m: Marker) => {
              const meta = kindMeta(m.kind);
              return (
                <div className={styles.markerRow} key={m.id}>
                  <span
                    className={styles.markerChip}
                    style={{ color: meta.color, borderColor: meta.color }}
                  >
                    <Icon name={meta.icon} size={12} stroke={meta.color} />
                    {meta.label[lang]}
                  </span>
                  <button
                    type="button"
                    className={styles.markerTime}
                    onClick={() => seekToSeconds(m.tSeconds)}
                  >
                    {fmtDuration(m.tSeconds)}
                  </button>
                  <span className={styles.markerLabel}>{m.label}</span>
                  <button
                    type="button"
                    className={styles.iconBtn}
                    aria-label={lang === 'es' ? 'Eliminar marcador' : 'Delete marker'}
                    onClick={() => void deleteMarker(m.id)}
                  >
                    <Icon name="trash" size={14} />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
