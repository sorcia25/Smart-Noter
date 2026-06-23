import { TemplateIcon } from '@/components/domain/TemplateIcon/TemplateIcon';
import { Button } from '@/components/primitives/Button/Button';
import { Chip } from '@/components/primitives/Chip/Chip';
import { Icon } from '@/components/primitives/Icon/Icon';
import { SegmentedControl } from '@/components/primitives/SegmentedControl/SegmentedControl';
import { useT } from '@/i18n/useT';
import { Paths } from '@/router/paths';
import { useGetMeetingQuery } from '@/store/api/meetings.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { fmtDate, fmtDuration, pickL } from '@/utils/format';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { ExportModal } from './ExportModal/ExportModal';
import styles from './MeetingDetailPage.module.css';
import { SidePanel } from './side/SidePanel';
import { ActionsTab } from './tabs/ActionsTab';
import { AudioTab } from './tabs/AudioTab';
import { SummaryTab } from './tabs/SummaryTab';
import { TranscriptTab } from './tabs/TranscriptTab';

type Tab = 'summary' | 'transcript' | 'actions' | 'audio';

export default function MeetingDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { t, lang } = useT();
  const [tab, setTab] = useState<Tab>('summary');
  const [exportOpen, setExportOpen] = useState(false);

  const { data: meeting, isLoading } = useGetMeetingQuery(id ?? '', { skip: !id });
  const { data: templates = [] } = useListTemplatesQuery();
  const template = templates.find((tpl) => tpl.id === meeting?.template);

  if (!id) {
    return null;
  }
  if (isLoading || !meeting) {
    return (
      <div className={styles.page} data-screen-label="05 Meeting detail">
        <div className={styles.header} style={{ padding: 32 }}>
          {lang === 'es' ? 'Cargando reunión…' : 'Loading meeting…'}
        </div>
      </div>
    );
  }

  const tabs: { value: Tab; label: string }[] = [
    { value: 'summary', label: t('summary') },
    { value: 'transcript', label: t('transcript') },
    { value: 'actions', label: `${t('actions')} (${meeting.actions.length})` },
    { value: 'audio', label: t('audio') },
  ];

  return (
    <div className={styles.page} data-screen-label="05 Meeting detail">
      <div className={styles.header}>
        <div style={{ minWidth: 0 }}>
          <button type="button" className={styles.back} onClick={() => navigate(Paths.Meetings)}>
            <Icon name="chevLeft" size={14} />
            {t('backToMeetings')}
          </button>
          <div className={styles.titleRow}>
            <TemplateIcon templateId={meeting.template} size={40} />
            <div style={{ minWidth: 0 }}>
              <h1 className={styles.title}>{pickL(meeting.title, lang)}</h1>
              <div className={styles.metaRow}>
                <span>{template ? pickL(template.name, lang) : meeting.template}</span>
                <span className={styles.sep} />
                <span>{fmtDate(meeting.date, lang)}</span>
                <span className={styles.sep} />
                <span>
                  {fmtDuration(meeting.durationSec)} · {meeting.wordCount}{' '}
                  {lang === 'es' ? 'palabras' : 'words'}
                </span>
                <Chip variant="accent" disabled>
                  99.2% {t('fidelity')}
                </Chip>
              </div>
            </div>
          </div>
        </div>
        <div className={styles.actions}>
          <Button
            icon={<Icon name="share" size={14} />}
            disabled
            title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
          >
            {t('share')}
          </Button>
          <Button
            variant="primary"
            icon={<Icon name="download" size={14} />}
            onClick={() => setExportOpen(true)}
          >
            {t('export')}
          </Button>
        </div>
      </div>
      <div className={styles.wrap}>
        <div className={styles.main}>
          <div className={styles.tabsRow}>
            <SegmentedControl<Tab> value={tab} options={tabs} onChange={setTab} />
          </div>
          {tab === 'summary' && <SummaryTab meeting={meeting} template={template} />}
          {tab === 'transcript' && <TranscriptTab meeting={meeting} />}
          {tab === 'actions' && (
            <ActionsTab
              actions={meeting.actions}
              participants={meeting.participants}
              meetingId={meeting.id}
            />
          )}
          {tab === 'audio' && <AudioTab meeting={meeting} />}
        </div>
        <SidePanel participants={meeting.participants} />
      </div>
      <ExportModal
        open={exportOpen}
        onClose={() => setExportOpen(false)}
        meetingTitle={pickL(meeting.title, lang)}
      />
    </div>
  );
}
