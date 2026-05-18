import styles from './EqBar.module.css';

export interface EqBarProps {
  bars?: number;
  className?: string;
}

export function EqBar({ bars = 5, className }: EqBarProps) {
  return (
    <div className={[styles.bar, className].filter(Boolean).join(' ')} aria-label="audio activity">
      {Array.from({ length: bars }, (_, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: bars are positionally-meaningful and never reorder
        <span key={i} />
      ))}
    </div>
  );
}
