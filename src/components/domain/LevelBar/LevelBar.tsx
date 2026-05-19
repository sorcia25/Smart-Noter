import styles from './LevelBar.module.css';

export interface LevelBarProps {
  /** 0..1 (values outside the range are clamped). */
  level: number;
  className?: string;
}

export function LevelBar({ level, className }: LevelBarProps) {
  const pct = Math.max(0, Math.min(1, level)) * 100;
  return (
    <div
      className={[styles.bar, className].filter(Boolean).join(' ')}
      role="meter"
      aria-valuenow={Math.round(pct)}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div className={styles.fill} style={{ width: `${pct}%` }} />
    </div>
  );
}
