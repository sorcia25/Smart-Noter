import { TemplateIcon } from '@/components/domain/TemplateIcon/TemplateIcon';
import { Button } from '@/components/primitives/Button/Button';
import { Chip } from '@/components/primitives/Chip/Chip';
import { Icon } from '@/components/primitives/Icon/Icon';
import { useT } from '@/i18n/useT';
import type { Template } from '@/ipc/bindings';
import { pickL } from '@/utils/format';
import { featuresFor } from '../../featuresFor';
import styles from './TemplateGalleryCard.module.css';

export interface TemplateGalleryCardProps {
  template: Template;
  isDefault: boolean;
  onUseAsDefault: () => void;
}

export function TemplateGalleryCard({
  template,
  isDefault,
  onUseAsDefault,
}: TemplateGalleryCardProps) {
  const { t, lang } = useT();
  const features = featuresFor(template.id, lang);

  return (
    <div className={styles.card}>
      <div className={styles.head}>
        <div className={styles.iconLg}>
          <TemplateIcon templateId={template.id} size={44} />
        </div>
        <div style={{ minWidth: 0 }}>
          <h4 className={styles.name}>{pickL(template.name, lang)}</h4>
          <div className={styles.headSub}>
            {template.sections.length} {lang === 'es' ? 'secciones' : 'sections'}
          </div>
        </div>
        {isDefault && (
          <Chip variant="accent" disabled className={styles.defaultBadge}>
            {lang === 'es' ? 'Predeterminada' : 'Default'}
          </Chip>
        )}
      </div>

      <div className={styles.desc}>{pickL(template.desc, lang)}</div>

      <div className={styles.features}>
        {features.map((f) => (
          <div key={f} className={styles.feature}>
            <Icon name="check" size={11} />
            <span>{f}</span>
          </div>
        ))}
      </div>

      <div className={styles.actions}>
        <Button
          className={styles.useDefault}
          size="sm"
          onClick={onUseAsDefault}
          disabled={isDefault}
        >
          {isDefault ? (lang === 'es' ? 'Activa' : 'Active') : t('tmplUseDefault')}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          disabled
          title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
        >
          <Icon name="edit" size={14} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          disabled
          title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
        >
          <Icon name="copy" size={14} />
        </Button>
      </div>
    </div>
  );
}
