import { useT } from '@/i18n/useT';
import styles from './LivePill.module.css';

export interface LivePillProps {
  paused?: boolean;
  className?: string;
}

export function LivePill({ paused = false, className }: LivePillProps) {
  const { t } = useT();
  return (
    <div
      // biome-ignore lint/a11y/useSemanticElements: <output> would force inline display and break the rounded-pill layout
      className={[styles.pill, paused && styles.paused, className].filter(Boolean).join(' ')}
      role="status"
    >
      <div className={styles.dot} />
      {paused ? t('liveStatusPaused') : t('liveStatus')}
    </div>
  );
}
