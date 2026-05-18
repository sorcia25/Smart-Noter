import styles from './StatRow.module.css';

export interface Stat {
  label: string;
  value: string;
  delta: string;
  deltaTone?: 'accent' | 'warn';
}

export function StatRow({ stats }: { stats: Stat[] }) {
  return (
    <div className={styles.row}>
      {stats.map((s) => (
        <div key={s.label} className={styles.stat}>
          <div className={styles.label}>{s.label}</div>
          <div className={styles.value}>{s.value}</div>
          <div
            className={[styles.delta, s.deltaTone === 'warn' && styles.warn]
              .filter(Boolean)
              .join(' ')}
          >
            {s.delta}
          </div>
        </div>
      ))}
    </div>
  );
}
