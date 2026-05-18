import type { Participant } from '@/ipc/bindings';
import styles from './Avatar.module.css';

const COLOR_MAP: Record<string, string | undefined> = {
  's-color-1': styles.s1,
  's-color-2': styles.s2,
  's-color-3': styles.s3,
  's-color-4': styles.s4,
  's-color-5': styles.s5,
  's-color-6': styles.s6,
  's-color-7': styles.s7,
  's-color-8': styles.s8,
};

export interface SubjectAvatarProps {
  participant: Participant;
  size?: number;
}

function initials(p: Participant): string {
  const source = p.name ?? p.label;
  const parts = source.trim().split(/\s+/).filter(Boolean);
  const first = parts[0] ?? '?';
  if (parts.length === 1) return first.slice(0, 2).toUpperCase();
  const last = parts[parts.length - 1] ?? '';
  return ((first[0] ?? '?') + (last[0] ?? '')).toUpperCase();
}

export function SubjectAvatar({ participant, size = 28 }: SubjectAvatarProps) {
  const color = COLOR_MAP[participant.colorClass] ?? styles.s1;
  return (
    <div
      className={`${styles.avatar} ${color}`}
      style={{
        width: size,
        height: size,
        fontSize: Math.max(9, Math.round(size * 0.38)),
      }}
      title={participant.name ?? participant.label}
    >
      {initials(participant)}
    </div>
  );
}

export interface AvatarStackProps {
  participants: Participant[];
  max?: number;
  size?: number;
}

export function AvatarStack({ participants, max = 4, size = 26 }: AvatarStackProps) {
  const shown = participants.slice(0, max);
  const extra = participants.length - shown.length;
  return (
    <div className={styles.stack}>
      {shown.map((p) => (
        <SubjectAvatar key={p.id} participant={p} size={size} />
      ))}
      {extra > 0 && (
        <div
          className={`${styles.avatar} ${styles.overflow}`}
          style={{ width: size, height: size, fontSize: Math.max(9, Math.round(size * 0.34)) }}
        >
          +{extra}
        </div>
      )}
    </div>
  );
}
