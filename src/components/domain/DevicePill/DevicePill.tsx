import { useT } from '@/i18n/useT';
import type { AudioDevice } from '@/ipc/bindings';
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
        <div className={styles.name}>{device.name}</div>
        <div className={styles.desc}>
          {device.kind === 'loopback'
            ? lang === 'es'
              ? 'Audio del sistema'
              : 'System audio'
            : lang === 'es'
              ? 'Micrófono'
              : 'Microphone'}
          {device.isDefault ? ` · ${lang === 'es' ? 'Predeterminado' : 'Default'}` : ''}
        </div>
      </div>
      {trailing}
    </div>
  );
}
