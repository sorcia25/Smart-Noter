import { TemplateIcon } from '@/components/domain/TemplateIcon/TemplateIcon';
import { useT } from '@/i18n/useT';
import type { Template } from '@/ipc/bindings';
import { pickL } from '@/utils/format';
import styles from './QuickStartCard.module.css';

const PRESET_IDS = ['daily', 'ejecutiva', 'tecnica'] as const;
const PRESET_DURATIONS: Record<string, string> = {
  daily: '15 min',
  ejecutiva: '60 min',
  tecnica: '45 min',
};

export interface QuickStartCardProps {
  templates: Template[];
  onPick: (templateId: string) => void;
}

export function QuickStartCard({ templates, onPick }: QuickStartCardProps) {
  const { lang } = useT();
  const byId = new Map(templates.map((t) => [t.id, t] as const));

  const titleText = lang === 'es' ? 'Inicio rápido' : 'Quick start';
  const subText =
    lang === 'es'
      ? 'Empieza una sesión con plantilla preconfigurada.'
      : 'Start a session with a preset template.';

  return (
    <div className={styles.card}>
      <h3 className={styles.title}>{titleText}</h3>
      <div className={styles.sub}>{subText}</div>
      <div className={styles.list}>
        {PRESET_IDS.map((id) => {
          const tpl = byId.get(id);
          const label = tpl ? pickL(tpl.name, lang) : id;
          return (
            <button key={id} type="button" className={styles.row} onClick={() => onPick(id)}>
              <TemplateIcon templateId={id} size={28} />
              <div>
                <div className={styles.rowName}>{label}</div>
                <div className={styles.rowSub}>{PRESET_DURATIONS[id]}</div>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
