import { type ButtonHTMLAttributes, type ReactNode, forwardRef } from 'react';
import styles from './Button.module.css';

type Variant = 'default' | 'primary' | 'ghost' | 'danger';
type Size = 'sm' | 'md' | 'icon';

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  variant?: Variant;
  size?: Size;
  loading?: boolean;
  icon?: ReactNode;
  children?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  {
    variant = 'default',
    size = 'md',
    loading = false,
    icon,
    children,
    className,
    disabled,
    type = 'button',
    ...rest
  },
  ref
) {
  const classes = [
    styles.btn,
    variant !== 'default' && styles[variant],
    size === 'icon' && styles.iconOnly,
    size === 'sm' && styles.sm,
    className,
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <button
      ref={ref}
      type={type}
      className={classes}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {icon}
      {children}
    </button>
  );
});
