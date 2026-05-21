import type { ReactNode } from 'react';
import styles from './SegmentedControl.module.css';

export interface SegmentedOption<T extends string> {
  value: T;
  label: ReactNode;
  disabled?: boolean;
}

export interface SegmentedControlProps<T extends string> {
  value: T;
  options: SegmentedOption<T>[];
  onChange: (next: T) => void;
  className?: string;
}

export function SegmentedControl<T extends string>({
  value,
  options,
  onChange,
  className,
}: SegmentedControlProps<T>) {
  return (
    <div className={[styles.group, className].filter(Boolean).join(' ')} role="tablist">
      {options.map((o) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="tab"
            aria-selected={active}
            aria-disabled={o.disabled || undefined}
            disabled={o.disabled}
            className={`${styles.btn} ${active ? styles.active : ''}`}
            onClick={() => !o.disabled && onChange(o.value)}
            title={o.disabled ? 'Próximamente' : undefined}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
