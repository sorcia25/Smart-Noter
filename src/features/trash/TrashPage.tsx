import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { Modal } from '@/components/primitives/Modal/Modal';
import { useT } from '@/i18n/useT';
import {
  useListTrashedMeetingsQuery,
  usePurgeMeetingMutation,
  useRestoreMeetingMutation,
} from '@/store/api/meetings.api';
import { pickL } from '@/utils/format';
import { useState } from 'react';
import styles from './TrashPage.module.css';

export default function TrashPage() {
  const { t, lang } = useT();
  const { data: trashed = [] } = useListTrashedMeetingsQuery();
  const [restoreMeeting] = useRestoreMeetingMutation();
  const [purgeMeeting] = usePurgeMeetingMutation();
  const [pendingPurge, setPendingPurge] = useState<string | null>(null);

  return (
    <div className={styles.page} data-screen-label="Trash">
      <div className={styles.header}>
        <h1 className={styles.title}>{t('trashTitle')}</h1>
        <div className={styles.sub}>{t('trashSub')}</div>
      </div>

      <div className={styles.scroll}>
        {trashed.length === 0 ? (
          <div className={styles.empty}>
            <Icon name="trash" size={32} />
            <span>{t('trashEmpty')}</span>
          </div>
        ) : (
          <div className={styles.list}>
            {trashed.map((m) => (
              <div key={m.id} className={styles.row}>
                <span className={styles.rowTitle}>{pickL(m.title, lang)}</span>
                <div className={styles.rowActions}>
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label={`restore-${m.id}`}
                    icon={<Icon name="refresh" size={14} />}
                    onClick={() => restoreMeeting(m.id)}
                  >
                    {t('restoreMeeting')}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label={`purge-${m.id}`}
                    icon={<Icon name="trash" size={14} />}
                    onClick={() => setPendingPurge(m.id)}
                  >
                    {t('deletePermanently')}
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <Modal
        open={pendingPurge !== null}
        onClose={() => setPendingPurge(null)}
        title={t('confirmPurgeTitle')}
        subtitle={t('confirmPurgeBody')}
        footer={
          <>
            <Button variant="ghost" onClick={() => setPendingPurge(null)}>
              {t('cancel')}
            </Button>
            <Button
              variant="primary"
              onClick={async () => {
                if (pendingPurge) await purgeMeeting(pendingPurge).unwrap();
                setPendingPurge(null);
              }}
            >
              {t('deletePermanently')}
            </Button>
          </>
        }
      >
        <div />
      </Modal>
    </div>
  );
}
