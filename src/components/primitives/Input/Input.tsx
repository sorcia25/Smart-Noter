import { type InputHTMLAttributes, forwardRef } from 'react';
import styles from './Input.module.css';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { label, className, id, ...rest },
  ref
) {
  const inputId = id ?? (label ? `i-${label.replace(/\s+/g, '-').toLowerCase()}` : undefined);
  return (
    <>
      {label && (
        <label className={styles.label} htmlFor={inputId}>
          {label}
        </label>
      )}
      <input
        ref={ref}
        id={inputId}
        className={[styles.input, className].filter(Boolean).join(' ')}
        {...rest}
      />
    </>
  );
});
