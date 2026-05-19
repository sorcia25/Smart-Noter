import { DevicePill } from '@/components/domain/DevicePill/DevicePill';
import { EqBar } from '@/components/domain/EqBar/EqBar';
import { LevelBar } from '@/components/domain/LevelBar/LevelBar';
import { useT } from '@/i18n/useT';
import type { AudioDevice } from '@/ipc/bindings';
import styles from './CaptureStatusCard.module.css';

export interface CaptureStatusCardProps {
  device: AudioDevice | undefined;
  level?: number;
  levelDb?: string;
}

export function CaptureStatusCard({
  device,
  level = 0.62,
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
      <LevelBar level={level} />
    </div>
  );
}
