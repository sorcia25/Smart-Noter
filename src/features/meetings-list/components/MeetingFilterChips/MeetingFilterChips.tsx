import { Chip } from '@/components/primitives/Chip/Chip';
import { useT } from '@/i18n/useT';
import type { Template } from '@/ipc/bindings';
import { pickL } from '@/utils/format';
import styles from './MeetingFilterChips.module.css';

export interface MeetingFilterChipsProps {
  templates: Template[];
  selected: string; // 'all' or a template id
  totalCount: number;
  onChange: (next: string) => void;
}

export function MeetingFilterChips({
  templates,
  selected,
  totalCount,
  onChange,
}: MeetingFilterChipsProps) {
  const { lang } = useT();
  return (
    <div className={styles.chips}>
      <Chip variant={selected === 'all' ? 'accent' : 'default'} onClick={() => onChange('all')}>
        {lang === 'es' ? 'Todas' : 'All'} · {totalCount}
      </Chip>
      {templates.map((tpl) => (
        <Chip
          key={tpl.id}
          variant={selected === tpl.id ? 'accent' : 'default'}
          onClick={() => onChange(tpl.id)}
        >
          {pickL(tpl.name, lang)}
        </Chip>
      ))}
    </div>
  );
}
