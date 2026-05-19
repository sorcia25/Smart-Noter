import type { CSSProperties } from 'react';
import styles from './Skeleton.module.css';

export interface SkeletonProps {
  width?: number | string;
  height?: number | string;
  /** When true, renders as a circle (height should equal width). */
  round?: boolean;
  className?: string;
  style?: CSSProperties;
}

export function Skeleton({ width, height, round, className, style }: SkeletonProps) {
  return (
    <div
      className={[styles.skeleton, round && styles.round, className].filter(Boolean).join(' ')}
      style={{ width, height, ...style }}
      aria-hidden="true"
    />
  );
}
