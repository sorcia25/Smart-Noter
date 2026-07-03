import { type ButtonHTMLAttributes, type ReactNode, forwardRef } from 'react';
import styles from './Chip.module.css';

type Variant = 'default' | 'accent';

export interface ChipProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  variant?: Variant;
  icon?: ReactNode;
  children?: ReactNode;
}

// Disabled chips are used as static badges, including inside other <button>
// elements (e.g. a "Recomendado" badge inside a selectable device card).
// Rendering a <button disabled> there would nest interactive controls, which
// is invalid HTML and trips React's validateDOMNesting warning. When
// disabled, render a non-interactive <span> with the same visual style
// instead of a real <button>.
export const Chip = forwardRef<HTMLButtonElement, ChipProps>(function Chip(
  { variant = 'default', icon, children, className, type = 'button', disabled, ...rest },
  ref
) {
  const classes = [
    styles.chip,
    variant === 'accent' && styles.accent,
    disabled && styles.disabled,
    className,
  ]
    .filter(Boolean)
    .join(' ');

  if (disabled) {
    const { onClick: _onClick, ...spanRest } = rest;
    return (
      <span aria-disabled="true" className={classes} {...spanRest}>
        {icon}
        {children}
      </span>
    );
  }

  return (
    <button ref={ref} type={type} className={classes} disabled={disabled} {...rest}>
      {icon}
      {children}
    </button>
  );
});
