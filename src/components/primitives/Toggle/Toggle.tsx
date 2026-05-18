import styles from './Toggle.module.css';

export interface ToggleProps {
  on: boolean;
  onChange: (next: boolean) => void;
  'aria-label'?: string;
  disabled?: boolean;
  className?: string;
}

export function Toggle({ on, onChange, disabled, className, ...rest }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={rest['aria-label']}
      disabled={disabled}
      className={[styles.toggle, on && styles.on, className].filter(Boolean).join(' ')}
      onClick={() => onChange(!on)}
    />
  );
}
