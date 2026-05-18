import { SubjectAvatar } from '@/components/primitives/Avatar/Avatar';
import { Chip } from '@/components/primitives/Chip/Chip';
import { Icon } from '@/components/primitives/Icon/Icon';
import { useT } from '@/i18n/useT';
import type { MeetingDetail, Participant, TranscriptLine } from '@/ipc/bindings';
import { pickL } from '@/utils/format';
import { useMemo } from 'react';
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

  const byId = useMemo(() => {
    const map = new Map<string, Participant>();
    for (const p of meeting.participants) map.set(p.id, p);
    return map;
  }, [meeting.participants]);

  // Synthesize sample lines when the meeting has none (Foundation: most meetings have empty transcripts)
  const lines: TranscriptLine[] = useMemo(() => {
    if (meeting.transcript.length > 0) return meeting.transcript;
    const [p0, p1] = meeting.participants;
    if (!p0 || !p1) return [];
    return [
      {
        t: '00:00:04',
        speakerId: p0.id,
        text: {
          es: 'Bienvenidos. Vamos a comenzar la sesión hoy revisando los puntos pendientes.',
          en: "Welcome. Let's start today's session reviewing pending items.",
        },
      },
      {
        t: '00:00:20',
        speakerId: p1.id,
        text: {
          es: 'Gracias. Tengo varios puntos importantes que compartir con el equipo.',
          en: 'Thanks. I have several important points to share with the team.',
        },
      },
    ];
  }, [meeting.transcript, meeting.participants]);

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
      {lines.length === 0 ? (
        <div className={styles.empty}>
          {lang === 'es'
            ? 'Sin transcripción para esta reunión.'
            : 'No transcript for this meeting.'}
        </div>
      ) : (
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
      )}
    </div>
  );
}
