import { type HTMLAttributes, type ReactNode, forwardRef } from 'react';
import styles from './Card.module.css';

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  padded?: boolean;
  children?: ReactNode;
}

export const Card = forwardRef<HTMLDivElement, CardProps>(function Card(
  { padded = false, className, children, ...rest },
  ref
) {
  const classes = [styles.card, padded && styles.pad, className].filter(Boolean).join(' ');
  return (
    <div ref={ref} className={classes} {...rest}>
      {children}
    </div>
  );
});
