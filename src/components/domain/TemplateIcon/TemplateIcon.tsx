import { Icon, type IconName } from '@/components/primitives/Icon/Icon';
import styles from './TemplateIcon.module.css';

const COLOR_MAP: Record<string, string | undefined> = {
  ejecutiva: styles.tEjecutiva,
  discovery: styles.tDiscovery,
  conferencia: styles.tConferencia,
  tecnica: styles.tTecnica,
  webinar: styles.tWebinar,
  daily: styles.tDaily,
  retro: styles.tRetro,
  entrevista: styles.tEntrevista,
  coaching: styles.tCoaching,
};

const ICON_MAP: Record<string, IconName> = {
  ejecutiva: 'briefcase',
  discovery: 'search',
  conferencia: 'megaphone',
  tecnica: 'cpu',
  webinar: 'monitor',
  daily: 'sun',
  retro: 'refresh',
  entrevista: 'user',
  coaching: 'compass',
};

export interface TemplateIconProps {
  templateId: string;
  size?: number;
}

export function TemplateIcon({ templateId, size = 44 }: TemplateIconProps) {
  const color = COLOR_MAP[templateId] ?? styles.tDefault;
  const iconName = ICON_MAP[templateId] ?? 'list';
  return (
    <div
      className={`${styles.tmplIcon} ${color}`}
      style={{ width: size, height: size, fontSize: Math.round(size * 0.4) }}
    >
      <Icon name={iconName} size={Math.round(size * 0.5)} stroke="white" />
    </div>
  );
}
