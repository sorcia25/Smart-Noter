import { TemplateIcon } from '@/components/domain/TemplateIcon/TemplateIcon';
import { AvatarStack } from '@/components/primitives/Avatar/Avatar';
import { useT } from '@/i18n/useT';
import type { MeetingSummary } from '@/ipc/bindings';
import { fmtDate, fmtDuration, pickL } from '@/utils/format';
import styles from './MeetingRow.module.css';

export interface MeetingRowProps {
  meeting: MeetingSummary;
  onClick?: () => void;
}

export function MeetingRow({ meeting, onClick }: MeetingRowProps) {
  const { lang } = useT();
  return (
    <button type="button" className={styles.row} onClick={onClick}>
      <TemplateIcon templateId={meeting.template} size={44} />
      <div className={styles.meta}>
        <div className={styles.title}>{pickL(meeting.title, lang)}</div>
        <div className={styles.sub}>
          <span>{meeting.template}</span>
          <span className={styles.sep} />
          <span>{fmtDate(meeting.date, lang)}</span>
          <span className={styles.sep} />
          <span>
            {meeting.participants.length} {lang === 'es' ? 'participantes' : 'participants'}
          </span>
        </div>
      </div>
      <AvatarStack participants={meeting.participants} size={26} max={4} />
      <div className={styles.duration}>{fmtDuration(meeting.durationSec)}</div>
    </button>
  );
}
