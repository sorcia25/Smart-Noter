import { useRef } from 'react';
import styles from './Waveform.module.css';

export interface WaveformProps {
  /** Number of bars (default 36). */
  bars?: number;
  /** When true, freezes the animation and dims the bars to ~30% opacity. */
  paused?: boolean;
  className?: string;
  /** When provided, overrides the internal random heights with real audio data (0..1 per bin). */
  externalBins?: number[];
}

/**
 * Animated equalizer-style waveform used by the live recording screen.
 * Bar heights are randomized once per mount so the visual stays stable
 * for the session. When `externalBins` is provided, real audio data is
 * used instead.
 */
export function Waveform({ bars = 36, paused = false, className, externalBins }: WaveformProps) {
  const heightsRef = useRef<number[] | null>(null);
  if (!heightsRef.current || heightsRef.current.length !== bars) {
    heightsRef.current = Array.from({ length: bars }, () => 0.25 + Math.random() * 0.75);
  }
  const heights = externalBins ?? heightsRef.current;

  return (
    <div
      className={[styles.wave, paused && styles.paused, className].filter(Boolean).join(' ')}
      aria-label="audio waveform"
    >
      {heights.map((h, i) => (
        <span
          // biome-ignore lint/suspicious/noArrayIndexKey: bars are positional and never reorder
          key={i}
          style={{
            height: `${Math.round((paused ? 0.2 : h) * 100)}%`,
            animationDelay: `${(i * 60) % 1200}ms`,
            opacity: paused ? 0.3 : 1,
          }}
        />
      ))}
    </div>
  );
}
