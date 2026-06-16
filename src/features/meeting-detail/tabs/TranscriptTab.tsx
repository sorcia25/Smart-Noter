import { SubjectAvatar } from '@/components/primitives/Avatar/Avatar';
import { Button } from '@/components/primitives/Button/Button';
import { Chip } from '@/components/primitives/Chip/Chip';
import { Icon } from '@/components/primitives/Icon/Icon';
import { useT } from '@/i18n/useT';
import type { MeetingDetail, Participant } from '@/ipc/bindings';
import { useGetSettingsQuery } from '@/store/api/settings.api';
import { pickL } from '@/utils/format';
import { useEffect, useMemo, useRef } from 'react';
import { useLocation } from 'react-router-dom';
import { useTranscription } from '../useTranscription';
import styles from './TranscriptTab.module.css';

export interface TranscriptTabProps {
  meeting: MeetingDetail;
}

function speakerLabel(p: Participant | undefined, lang: 'es' | 'en'): string {
  if (!p) return '—';
  if (p.name) return p.name;
  const suffix = p.label.replace(/^[a-zA-Z]+/, '');
  return lang === 'es'
    ? `Sujeto${suffix ? ` ${suffix}` : ''}`
    : `Subject${suffix ? ` ${suffix}` : ''}`;
}

export function TranscriptTab({ meeting }: TranscriptTabProps) {
  const { t, lang } = useT();
  const location = useLocation();
  const justRecorded = (location.state as { justRecorded?: boolean } | null)?.justRecorded ?? false;
  const { data: settings } = useGetSettingsQuery();
  const { status, pct, start, cancel } = useTranscription(meeting.id);

  const byId = useMemo(() => {
    const map = new Map<string, Participant>();
    for (const p of meeting.participants) map.set(p.id, p);
    return map;
  }, [meeting.participants]);

  const lines = meeting.transcript; // real data only — no more mock synthesis

  // Auto-trigger ONLY for a freshly-saved recording with the setting on.
  const autoStarted = useRef(false);
  useEffect(() => {
    if (autoStarted.current) return;
    if (lines.length === 0 && justRecorded && settings?.autoTranscribe && status === 'idle') {
      autoStarted.current = true;
      void start();
    }
  }, [lines.length, justRecorded, settings?.autoTranscribe, status, start]);

  return (
    <div className={styles.card}>
      <div className={styles.cardHead}>
        <div className={styles.cardHeadLeft}>
          <Icon name="mic" size={14} stroke="var(--accent)" />
          <span>{t('transcript')}</span>
          <Chip variant="accent" disabled>
            99.2% {t('fidelity')}
          </Chip>
        </div>
      </div>
      {lines.length > 0 ? (
        <div>
          {lines.map((l) => {
            const sp = byId.get(l.speakerId);
            return (
              <div className={styles.line} key={`${l.t}-${l.speakerId}`}>
                <div className={styles.who}>
                  {sp ? (
                    <SubjectAvatar participant={sp} size={32} />
                  ) : (
                    <div style={{ width: 32, height: 32 }} />
                  )}
                  <div className={styles.time}>{l.t}</div>
                </div>
                <div>
                  <div className={styles.speaker}>{speakerLabel(sp, lang)}</div>
                  <div className={styles.text}>{pickL(l.text, lang)}</div>
                </div>
              </div>
            );
          })}
        </div>
      ) : status === 'running' ? (
        <div className={styles.empty}>
          <div>
            {t('transcribe.running')} {pct}%
          </div>
          <Button variant="default" onClick={() => void cancel()}>
            {t('transcribe.cancel')}
          </Button>
        </div>
      ) : (
        <div className={styles.empty}>
          <div>
            {lang === 'es'
              ? 'Sin transcripción para esta reunión.'
              : 'No transcript for this meeting.'}
          </div>
          <Button variant="primary" onClick={() => void start()}>
            {t('transcribe.cta')}
          </Button>
        </div>
      )}
    </div>
  );
}
