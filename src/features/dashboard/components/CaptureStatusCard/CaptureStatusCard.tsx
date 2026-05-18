import { DevicePill } from '@/components/domain/DevicePill/DevicePill';
import { EqBar } from '@/components/domain/EqBar/EqBar';
import { useT } from '@/i18n/useT';
import type { AudioDevice } from '@/ipc/bindings';
import styles from './CaptureStatusCard.module.css';

export interface CaptureStatusCardProps {
  device: AudioDevice | undefined;
  levelPct?: number;
  levelDb?: string;
}

export function CaptureStatusCard({
  device,
  levelPct = 62,
  levelDb = '−12 dB',
}: CaptureStatusCardProps) {
  const { t } = useT();

  return (
    <div className={styles.card}>
      <h3 className={styles.title}>{t('captureStatus')}</h3>
      <div className={styles.sub}>{t('captureDesc')}</div>
      <DevicePill
        device={device}
        trailing={
          <div className={styles.eqWrap}>
            <EqBar />
          </div>
        }
      />
      <div className={styles.levelHead}>
        <span>{t('inputLevel')}</span>
        <span className={styles.levelDb}>{levelDb}</span>
      </div>
      <div className={styles.levelBar}>
        <div className={styles.levelFill} style={{ width: `${levelPct}%` }} />
      </div>
    </div>
  );
}
