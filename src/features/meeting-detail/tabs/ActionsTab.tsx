import { SubjectAvatar } from '@/components/primitives/Avatar/Avatar';
import { Chip } from '@/components/primitives/Chip/Chip';
import { Icon } from '@/components/primitives/Icon/Icon';
import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import type { Action, Participant } from '@/ipc/bindings';
import {
  useCreateActionMutation,
  useDeleteActionMutation,
  useToggleActionMutation,
  useUpdateActionMutation,
} from '@/store/api/meetings.api';
import { pickL } from '@/utils/format';
import { useMemo, useState } from 'react';
import styles from './ActionsTab.module.css';

export interface ActionsTabProps {
  actions: Action[];
  participants: Participant[];
  meetingId: string;
}

interface Draft {
  id: string | null;
  text: string;
  ownerId: string | null;
  due: string | null;
}

function ownerLabel(p: Participant | undefined, lang: 'es' | 'en'): string {
  if (!p) return '';
  if (p.name) return p.name;
  const suffix = p.label.replace(/^[a-zA-Z]+/, '');
  return lang === 'es'
    ? `Sujeto${suffix ? ` ${suffix}` : ''}`
    : `Subject${suffix ? ` ${suffix}` : ''}`;
}

export function ActionsTab({ actions, participants, meetingId }: ActionsTabProps) {
  const { t, lang } = useT();
  const [toggleAction] = useToggleActionMutation();
  const [createAction] = useCreateActionMutation();
  const [updateAction] = useUpdateActionMutation();
  const [deleteAction] = useDeleteActionMutation();
  const [draft, setDraft] = useState<Draft | null>(null);

  const byId = useMemo(() => {
    const map = new Map<string, Participant>();
    for (const p of participants) map.set(p.id, p);
    return map;
  }, [participants]);

  async function save() {
    if (!draft || !draft.text.trim()) return;
    try {
      if (draft.id) {
        await updateAction({
          meetingId,
          actionId: draft.id,
          text: draft.text.trim(),
          ownerParticipantId: draft.ownerId,
          due: draft.due,
        }).unwrap();
      } else {
        await createAction({
          meetingId,
          text: draft.text.trim(),
          ownerParticipantId: draft.ownerId,
          due: draft.due,
        }).unwrap();
      }
    } catch {
      toast.error(t('errorTitle'));
      return;
    }
    setDraft(null);
  }

  async function remove(id: string) {
    try {
      await deleteAction({ meetingId, actionId: id }).unwrap();
    } catch {
      toast.error(t('errorTitle'));
    }
  }

  const editor = (
    <div className={styles.draftRow}>
      <input
        className={styles.draftInput}
        value={draft?.text ?? ''}
        placeholder={t('itemTextPh')}
        onChange={(e) => setDraft((d) => (d ? { ...d, text: e.target.value } : d))}
        // biome-ignore lint/a11y/noAutofocus: focus the editor when it opens
        autoFocus
      />
      <select
        className={styles.draftSelect}
        value={draft?.ownerId ?? ''}
        onChange={(e) => setDraft((d) => (d ? { ...d, ownerId: e.target.value || null } : d))}
        aria-label={t('ownerLabel')}
      >
        <option value="">{t('noOwner')}</option>
        {participants.map((p) => (
          <option key={p.id} value={p.id}>
            {ownerLabel(p, lang)}
          </option>
        ))}
      </select>
      <input
        type="date"
        className={styles.draftDate}
        value={draft?.due?.slice(0, 10) ?? ''}
        onChange={(e) => setDraft((d) => (d ? { ...d, due: e.target.value || null } : d))}
        aria-label={t('dueLabel')}
      />
      <button type="button" className={styles.saveBtn} onClick={() => void save()}>
        {t('saveItem')}
      </button>
      <button type="button" className={styles.cancelBtn} onClick={() => setDraft(null)}>
        {t('cancel')}
      </button>
    </div>
  );

  return (
    <div className={styles.card}>
      <div className={styles.cardHead}>
        <div className={styles.cardHeadLeft}>
          <Icon name="check" size={14} stroke="var(--accent)" />
          <span>{t('actions')}</span>
          <Chip disabled>{actions.length} total</Chip>
        </div>
        <button
          type="button"
          className={styles.addBtn}
          onClick={() => setDraft({ id: null, text: '', ownerId: null, due: null })}
        >
          <Icon name="plus" size={13} />
          <span>{t('addAction')}</span>
        </button>
      </div>

      {actions.length === 0 && !draft ? (
        <div className={styles.empty}>
          {lang === 'es' ? 'Sin acciones registradas.' : 'No actions on this meeting.'}
        </div>
      ) : (
        actions.map((a) => {
          if (draft?.id === a.id) return <div key={a.id}>{editor}</div>;
          const owner = a.ownerParticipantId ? byId.get(a.ownerParticipantId) : undefined;
          const due = a.due ? new Date(a.due) : null;
          return (
            <div className={`${styles.item} ${a.done ? styles.done : ''}`} key={a.id}>
              <button
                type="button"
                className={styles.check}
                onClick={() => void toggleAction(a.id)}
                aria-pressed={a.done}
                aria-label={lang === 'es' ? 'Marcar acción' : 'Toggle action'}
              >
                {a.done ? <Icon name="check" size={11} stroke="white" /> : null}
              </button>
              <div className={styles.body}>
                <div className={styles.text}>{pickL(a.text, lang)}</div>
                <div className={styles.meta}>
                  {owner && <SubjectAvatar participant={owner} size={18} />}
                  {owner && <span>{ownerLabel(owner, lang)}</span>}
                  {owner && due && <span>·</span>}
                  {due && (
                    <>
                      <Icon name="clock" size={11} />
                      <span>
                        {due.toLocaleDateString(lang === 'en' ? 'en-US' : 'es-MX', {
                          day: '2-digit',
                          month: 'short',
                        })}
                      </span>
                    </>
                  )}
                </div>
              </div>
              <div className={styles.rowActions}>
                <button
                  type="button"
                  className={styles.iconBtn}
                  aria-label={t('editItem')}
                  onClick={() =>
                    setDraft({
                      id: a.id,
                      text: a.text.es,
                      ownerId: a.ownerParticipantId,
                      due: a.due,
                    })
                  }
                >
                  <Icon name="edit" size={15} />
                </button>
                <button
                  type="button"
                  className={styles.iconBtn}
                  aria-label={t('deleteItem')}
                  onClick={() => void remove(a.id)}
                >
                  <Icon name="trash" size={15} />
                </button>
              </div>
            </div>
          );
        })
      )}

      {draft && draft.id === null ? editor : null}
    </div>
  );
}
