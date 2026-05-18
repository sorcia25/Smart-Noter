import { useT } from '@/i18n/useT';
import type { AudioDevice } from '@/ipc/bindings';
import { pickL } from '@/utils/format';
import type { ReactNode } from 'react';
import styles from './DevicePill.module.css';

export interface DevicePillProps {
  device: AudioDevice | undefined;
  trailing?: ReactNode;
}

export function DevicePill({ device, trailing }: DevicePillProps) {
  const { lang } = useT();
  if (!device) {
    return (
      <div className={styles.pill}>
        <div className={styles.dot} />
        <div className={styles.meta}>
          <div className={styles.name}>—</div>
          <div className={styles.desc}>{lang === 'es' ? 'Sin dispositivo' : 'No device'}</div>
        </div>
      </div>
    );
  }
  return (
    <div className={styles.pill}>
      <div className={styles.dot} />
      <div className={styles.meta}>
        <div className={styles.name}>{pickL(device.name, lang)}</div>
        <div className={styles.desc}>{pickL(device.desc, lang)}</div>
      </div>
      {trailing}
    </div>
  );
}
