import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { Input } from '@/components/primitives/Input/Input';
import { Modal } from '@/components/primitives/Modal/Modal';
import { toast } from '@/components/primitives/Toast/Toast';
import { Toggle } from '@/components/primitives/Toggle/Toggle';
import { useT } from '@/i18n/useT';
import { useExportMeetingMutation } from '@/store/api/meetings.api';
import { useState } from 'react';
import styles from './ExportModal.module.css';

type Fmt = 'audio' | 'md' | 'pdf';

export interface ExportModalProps {
  open: boolean;
  onClose: () => void;
  meetingTitle: string;
  meetingId: string;
}

export function ExportModal({ open, onClose, meetingTitle, meetingId }: ExportModalProps) {
  const { t, lang } = useT();
  const [selected, setSelected] = useState<Set<Fmt>>(new Set(['md']));
  const [fileName, setFileName] = useState(meetingTitle.toLowerCase().replace(/\s+/g, '-'));
  const [timestamps, setTimestamps] = useState(true);
  const [bilingual, setBilingual] = useState(false);
  const [exportMeeting, { isLoading: busy }] = useExportMeetingMutation();

  function toggleFmt(f: Fmt) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(f)) next.delete(f);
      else next.add(f);
      return next;
    });
  }

  const formats: { value: Fmt; label: string; desc: string; iconClass: string; tag: string }[] = [
    {
      value: 'audio',
      label: t('exportAudio'),
      desc: t('exportAudioDesc'),
      iconClass: styles.iconMp3 ?? '',
      tag: 'MP3',
    },
    {
      value: 'md',
      label: t('exportMd'),
      desc: t('exportMdDesc'),
      iconClass: styles.iconMd ?? '',
      tag: 'MD',
    },
    {
      value: 'pdf',
      label: t('exportPdf'),
      desc: t('exportPdfDesc'),
      iconClass: styles.iconPdf ?? '',
      tag: 'PDF',
    },
  ];

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('exportTitle')}
      subtitle={t('exportSub')}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button
            variant="primary"
            icon={<Icon name="download" size={14} />}
            disabled={selected.size === 0 || busy}
            onClick={async () => {
              try {
                const written = await exportMeeting({
                  meetingId,
                  formats: [...selected],
                  fileName,
                  timestamps,
                  bilingual,
                }).unwrap();
                if (written.length === 0) {
                  toast.info(t('exportCancelled'));
                  return;
                }
                toast.success(t('exportDone'));
                onClose();
              } catch {
                toast.error(t('exportFailed'));
              }
            }}
          >
            {busy ? t('exporting') : t('exportNow')}
          </Button>
        </>
      }
    >
      {formats.map((f) => (
        <button
          key={f.value}
          type="button"
          className={`${styles.row} ${selected.has(f.value) ? styles.selected : ''}`}
          onClick={() => toggleFmt(f.value)}
        >
          <div className={`${styles.icon} ${f.iconClass}`}>{f.tag}</div>
          <div>
            <div className={styles.formatTitle}>{f.label}</div>
            <div className={styles.formatDesc}>{f.desc}</div>
          </div>
          {selected.has(f.value) ? <Icon name="check" size={18} stroke="var(--accent)" /> : null}
        </button>
      ))}

      <div className={styles.fileNameRow}>
        <Input
          label={lang === 'es' ? 'Nombre de archivo' : 'File name'}
          value={fileName}
          onChange={(e) => setFileName(e.target.value)}
          placeholder={t('fileNamePh')}
        />
      </div>

      <div className={styles.options}>
        <div className={styles.optionRow}>
          <div>
            <div className={styles.optionLabel}>{t('timestampsOn')}</div>
            <div className={styles.optionSub}>
              {lang === 'es'
                ? 'Incluye marcas de tiempo junto a cada línea.'
                : 'Include timestamps next to each line.'}
            </div>
          </div>
          <Toggle on={timestamps} onChange={setTimestamps} aria-label="timestamps" />
        </div>
        <div className={styles.optionRow}>
          <div>
            <div className={styles.optionLabel}>{t('bilingual')}</div>
            <div className={styles.optionSub}>
              {lang === 'es'
                ? 'Exporta también en el otro idioma.'
                : 'Also export in the other language.'}
            </div>
          </div>
          <Toggle on={bilingual} onChange={setBilingual} aria-label="bilingual" />
        </div>
      </div>
    </Modal>
  );
}
