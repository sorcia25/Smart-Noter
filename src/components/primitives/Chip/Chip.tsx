import { type ButtonHTMLAttributes, type ReactNode, forwardRef } from 'react';
import styles from './Chip.module.css';

type Variant = 'default' | 'accent';

export interface ChipProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  variant?: Variant;
  icon?: ReactNode;
  children?: ReactNode;
}

export const Chip = forwardRef<HTMLButtonElement, ChipProps>(function Chip(
  { variant = 'default', icon, children, className, type = 'button', ...rest },
  ref
) {
  const classes = [styles.chip, variant === 'accent' && styles.accent, className]
    .filter(Boolean)
    .join(' ');
  return (
    <button ref={ref} type={type} className={classes} {...rest}>
      {icon}
      {children}
    </button>
  );
});
