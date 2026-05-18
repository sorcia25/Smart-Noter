import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { useT } from '@/i18n/useT';
import { useListTemplatesQuery, useSetDefaultTemplateMutation } from '@/store/api/templates.api';
import styles from './TemplatesPage.module.css';
import { TemplateGalleryCard } from './components/TemplateGalleryCard/TemplateGalleryCard';

export default function TemplatesPage() {
  const { t, lang } = useT();
  const { data: templates = [] } = useListTemplatesQuery();
  const [setDefaultTemplate] = useSetDefaultTemplateMutation();

  return (
    <div className={styles.page} data-screen-label="06 Templates">
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>{t('tmplTitle')}</h1>
          <div className={styles.sub}>{t('tmplSub')}</div>
        </div>
        <div className={styles.actions}>
          <Button
            icon={<Icon name="download" size={14} />}
            disabled
            title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
          >
            {lang === 'es' ? 'Importar' : 'Import'}
          </Button>
          <Button
            variant="primary"
            icon={<Icon name="plus" size={14} />}
            disabled
            title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
          >
            {lang === 'es' ? 'Crear plantilla' : 'Create template'}
          </Button>
        </div>
      </div>
      <div className={styles.scroll}>
        <div className={styles.gallery}>
          {templates.map((tpl) => (
            <TemplateGalleryCard
              key={tpl.id}
              template={tpl}
              isDefault={tpl.isDefault}
              onUseAsDefault={() => void setDefaultTemplate(tpl.id)}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
