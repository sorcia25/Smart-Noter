import { SubjectAvatar } from '@/components/primitives/Avatar/Avatar';
import { Chip } from '@/components/primitives/Chip/Chip';
import { Icon } from '@/components/primitives/Icon/Icon';
import { useT } from '@/i18n/useT';
import type { Action, Participant } from '@/ipc/bindings';
import { useToggleActionMutation } from '@/store/api/meetings.api';
import { pickL } from '@/utils/format';
import { useMemo } from 'react';
import styles from './ActionsTab.module.css';

export interface ActionsTabProps {
  actions: Action[];
  participants: Participant[];
}

function ownerLabel(p: Participant | undefined, lang: 'es' | 'en'): string {
  if (!p) return '';
  if (p.name) return p.name;
  const suffix = p.label.replace(/^[a-zA-Z]+/, '');
  return lang === 'es'
    ? `Sujeto${suffix ? ` ${suffix}` : ''}`
    : `Subject${suffix ? ` ${suffix}` : ''}`;
}

export function ActionsTab({ actions, participants }: ActionsTabProps) {
  const { t, lang } = useT();
  const [toggleAction] = useToggleActionMutation();

  const byId = useMemo(() => {
    const map = new Map<string, Participant>();
    for (const p of participants) map.set(p.id, p);
    return map;
  }, [participants]);

  return (
    <div className={styles.card}>
      <div className={styles.cardHead}>
        <div className={styles.cardHeadLeft}>
          <Icon name="check" size={14} stroke="var(--accent)" />
          <span>{t('actions')}</span>
          <Chip disabled>
            {actions.length} {lang === 'es' ? 'total' : 'total'}
          </Chip>
        </div>
      </div>
      {actions.length === 0 ? (
        <div className={styles.empty}>
          {lang === 'es' ? 'Sin acciones registradas.' : 'No actions on this meeting.'}
        </div>
      ) : (
        actions.map((a) => {
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
              <div>
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
            </div>
          );
        })
      )}
    </div>
  );
}
