import { Icon } from '@/components/primitives/Icon/Icon';
import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import type { Bilingual } from '@/ipc/bindings';
import { pickL } from '@/utils/format';
import { useState } from 'react';
import styles from './EditableItems.module.css';

export interface EditableItem {
  id: number;
  text: Bilingual;
}

export interface EditableItemsProps {
  items: EditableItem[];
  addLabel: string;
  /** Used to build stable test ids, e.g. "decision" → delete-decision-1. */
  testIdPrefix: string;
  onCreate(text: string): Promise<unknown>;
  onUpdate(id: number, text: string): Promise<unknown>;
  onDelete(id: number): Promise<unknown>;
}

export function EditableItems({
  items,
  addLabel,
  testIdPrefix,
  onCreate,
  onUpdate,
  onDelete,
}: EditableItemsProps) {
  const { t, lang } = useT();
  const [draft, setDraft] = useState<{ id: number | null; text: string } | null>(null);

  async function save() {
    if (!draft || !draft.text.trim()) return;
    try {
      if (draft.id != null) await onUpdate(draft.id, draft.text.trim());
      else await onCreate(draft.text.trim());
    } catch {
      toast.error(t('errorTitle'));
      return;
    }
    setDraft(null);
  }

  async function remove(id: number) {
    try {
      await onDelete(id);
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
      <button type="button" className={styles.saveBtn} onClick={() => void save()}>
        {t('saveItem')}
      </button>
      <button type="button" className={styles.cancelBtn} onClick={() => setDraft(null)}>
        {t('cancel')}
      </button>
    </div>
  );

  return (
    <div>
      <ul className={styles.list}>
        {items.map((it) =>
          draft?.id === it.id ? (
            <li key={it.id}>{editor}</li>
          ) : (
            <li key={it.id} className={styles.item}>
              <span className={styles.itemText}>{pickL(it.text, lang)}</span>
              <span className={styles.itemActions}>
                <button
                  type="button"
                  className={styles.iconBtn}
                  aria-label={t('editItem')}
                  onClick={() => setDraft({ id: it.id, text: it.text.es })}
                >
                  <Icon name="edit" size={14} />
                </button>
                <button
                  type="button"
                  className={styles.iconBtn}
                  aria-label={`delete-${testIdPrefix}-${it.id}`}
                  onClick={() => void remove(it.id)}
                >
                  <Icon name="trash" size={14} />
                </button>
              </span>
            </li>
          )
        )}
      </ul>
      {draft && draft.id === null ? (
        editor
      ) : (
        <button
          type="button"
          className={styles.addBtn}
          onClick={() => setDraft({ id: null, text: '' })}
        >
          <Icon name="plus" size={12} />
          <span>{addLabel}</span>
        </button>
      )}
    </div>
  );
}
