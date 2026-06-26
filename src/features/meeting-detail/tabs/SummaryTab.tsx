import { Button } from '@/components/primitives/Button/Button';
import { Icon, type IconName } from '@/components/primitives/Icon/Icon';
import { Modal } from '@/components/primitives/Modal/Modal';
import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import type { Bilingual, MeetingDetail, Template } from '@/ipc/bindings';
import { useGenerateSummaryMutation, useUpdateSummaryTextMutation } from '@/store/api/ai.api';
import {
  useCreateBlockerMutation,
  useCreateDecisionMutation,
  useDeleteBlockerMutation,
  useDeleteDecisionMutation,
  useUpdateBlockerMutation,
  useUpdateDecisionMutation,
} from '@/store/api/meetings.api';
import { pickL } from '@/utils/format';
import { useState } from 'react';
import { useAiSummary } from '../useAiSummary';
import { EditableItems } from './EditableItems';
import styles from './SummaryTab.module.css';

interface SectionConfig {
  titleKey:
    | 'secSummary'
    | 'secDecisions'
    | 'secMetrics'
    | 'secRisks'
    | 'secBlockers'
    | 'secArchitecture'
    | 'secTechDecisions'
    | 'secDeliverables';
  icon: IconName;
  render: () => JSX.Element | null;
}

export interface SummaryTabProps {
  meeting: MeetingDetail;
  template: Template | undefined;
}

// ---------------------------------------------------------------------------
// Summary section — editable + Regenerar + empty/loading states
// ---------------------------------------------------------------------------

interface SummaryBodyProps {
  meeting: MeetingDetail;
}

function SummaryBody({ meeting }: SummaryBodyProps) {
  const { t, lang } = useT();
  const { status, pct } = useAiSummary(meeting.id);
  const [generateSummary] = useGenerateSummaryMutation();
  const [updateSummaryText] = useUpdateSummaryTextMutation();

  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState('');
  const [confirmRegen, setConfirmRegen] = useState(false);

  const currentText = pickL(meeting.summary, lang) ?? '';

  function handleEditStart() {
    setEditValue(currentText);
    setEditing(true);
  }

  async function handleSave() {
    const updated: Bilingual = {
      es: lang === 'es' ? editValue : (meeting.summary?.es ?? editValue),
      en: lang === 'en' ? editValue : (meeting.summary?.en ?? null),
    };
    try {
      await updateSummaryText({ meetingId: meeting.id, summary: updated }).unwrap();
    } catch {
      toast.error(t('errorTitle'));
    }
    setEditing(false);
  }

  async function handleGenerate() {
    try {
      await generateSummary({ meetingId: meeting.id }).unwrap();
    } catch {
      toast.error(t('summaryFailed'));
    }
  }

  // ---- loading state ----
  if (status === 'running') {
    return (
      <div className={styles.summaryState}>
        <Icon name="refresh" size={16} className={styles.spin} />
        <span>
          {t('summarizing')} {pct > 0 ? `${pct}%` : ''}
        </span>
      </div>
    );
  }

  // ---- empty state ----
  if (!meeting.summary) {
    return (
      <div className={styles.summaryState}>
        <p className={styles.emptyHint}>{t('summaryEmpty')}</p>
        <Button variant="primary" size="sm" onClick={() => void handleGenerate()}>
          {t('generateSummary')}
        </Button>
      </div>
    );
  }

  // ---- editing state ----
  if (editing) {
    return (
      <div className={styles.summaryEdit}>
        <textarea
          className={styles.summaryTextarea}
          value={editValue}
          onChange={(e) => setEditValue(e.target.value)}
          rows={8}
          // biome-ignore lint/a11y/noAutofocus: intentional — user clicked Edit
          autoFocus
        />
        <div className={styles.summaryActions}>
          <Button variant="ghost" size="sm" onClick={() => setEditing(false)}>
            {t('cancel')}
          </Button>
          <Button variant="primary" size="sm" onClick={() => void handleSave()}>
            {t('save')}
          </Button>
        </div>
      </div>
    );
  }

  // ---- read state (has summary) ----
  return (
    <>
      <p>{currentText}</p>
      <div className={styles.summaryActions}>
        <Button variant="ghost" size="sm" onClick={handleEditStart}>
          <Icon name="edit" size={13} />
          {t('editSummary')}
        </Button>
        <Button variant="ghost" size="sm" onClick={() => setConfirmRegen(true)}>
          <Icon name="refresh" size={13} />
          {t('regenerate')}
        </Button>
      </div>

      <Modal
        open={confirmRegen}
        onClose={() => setConfirmRegen(false)}
        title={t('confirmRegenerateTitle')}
        subtitle={t('confirmRegenerateBody')}
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirmRegen(false)}>
              {t('cancel')}
            </Button>
            <Button
              variant="primary"
              onClick={async () => {
                setConfirmRegen(false);
                await handleGenerate();
              }}
            >
              {t('regenerate')}
            </Button>
          </>
        }
      />
    </>
  );
}

// ---------------------------------------------------------------------------
// Main tab
// ---------------------------------------------------------------------------

export function SummaryTab({ meeting, template }: SummaryTabProps) {
  const { t, lang } = useT();
  const [createDecision] = useCreateDecisionMutation();
  const [updateDecision] = useUpdateDecisionMutation();
  const [deleteDecision] = useDeleteDecisionMutation();
  const [createBlocker] = useCreateBlockerMutation();
  const [updateBlocker] = useUpdateBlockerMutation();
  const [deleteBlocker] = useDeleteBlockerMutation();
  const mId = meeting.id;

  const sectionsForTemplate = template?.sections ?? [
    'summary',
    'decisions',
    'metrics',
    'actions',
    'risks',
  ];

  const sections: Record<string, SectionConfig> = {
    summary: {
      titleKey: 'secSummary',
      icon: 'sparkles',
      render: () => <SummaryBody meeting={meeting} />,
    },
    decisions: {
      titleKey: 'secDecisions',
      icon: 'check',
      render: () => (
        <EditableItems
          items={meeting.decisions}
          addLabel={t('addDecision')}
          testIdPrefix="decision"
          onCreate={(text) => createDecision({ meetingId: mId, text }).unwrap()}
          onUpdate={(id, text) => updateDecision({ meetingId: mId, id, text }).unwrap()}
          onDelete={(id) => deleteDecision({ meetingId: mId, id }).unwrap()}
        />
      ),
    },
    blockers: {
      titleKey: 'secBlockers',
      icon: 'flag',
      render: () => (
        <EditableItems
          items={meeting.blockers}
          addLabel={t('addBlocker')}
          testIdPrefix="blocker"
          onCreate={(text) => createBlocker({ meetingId: mId, text }).unwrap()}
          onUpdate={(id, text) => updateBlocker({ meetingId: mId, id, text }).unwrap()}
          onDelete={(id) => deleteBlocker({ meetingId: mId, id }).unwrap()}
        />
      ),
    },
    risks: {
      titleKey: 'secRisks',
      icon: 'flag',
      render: () =>
        meeting.blockers.length === 0 ? null : (
          <ul>
            {meeting.blockers.map((b) => (
              <li key={b.id}>{pickL(b.text, lang)}</li>
            ))}
          </ul>
        ),
    },
  };

  return (
    <div>
      {sectionsForTemplate.map((key: string) => {
        if (key === 'actions') return null;
        const conf = sections[key];
        if (!conf) return null;
        const body = conf.render();
        if (!body) return null;
        return (
          <div className={styles.block} key={key}>
            <h3>
              <Icon name={conf.icon} size={14} />
              <span>{t(conf.titleKey)}</span>
            </h3>
            {body}
          </div>
        );
      })}
    </div>
  );
}
