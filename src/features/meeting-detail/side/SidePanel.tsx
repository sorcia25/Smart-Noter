import { SubjectAvatar } from '@/components/primitives/Avatar/Avatar';
import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { Input } from '@/components/primitives/Input/Input';
import { useT } from '@/i18n/useT';
import type { Participant } from '@/ipc/bindings';
import { useMergeSpeakersMutation, useRenameParticipantMutation } from '@/store/api/meetings.api';
import { useAppSelector } from '@/store/hooks';
import { type KeyboardEvent, useState } from 'react';
import styles from './SidePanel.module.css';

export interface SidePanelProps {
  participants: Participant[];
}

function fallbackName(p: Participant, lang: 'es' | 'en'): string {
  if (p.name) return p.name;
  const suffix = p.label.replace(/^[a-zA-Z]+/, '');
  return lang === 'es'
    ? `Sujeto${suffix ? ` ${suffix}` : ''}`
    : `Subject${suffix ? ` ${suffix}` : ''}`;
}

export function SidePanel({ participants }: SidePanelProps) {
  const { t, lang } = useT();
  const aiChatVisible = useAppSelector((s) => s.ui.aiChatVisible);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [menuFor, setMenuFor] = useState<string | null>(null);
  const [aiOpen, setAiOpen] = useState(true);
  const [renameParticipant] = useRenameParticipantMutation();
  const [mergeSpeakers] = useMergeSpeakersMutation();

  function commitRename(id: string, value: string) {
    const trimmed = value.trim();
    void renameParticipant({ participantId: id, name: trimmed === '' ? null : trimmed });
    setEditingId(null);
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>, id: string) {
    if (e.key === 'Enter') {
      commitRename(id, (e.target as HTMLInputElement).value);
    } else if (e.key === 'Escape') {
      setEditingId(null);
    }
  }

  return (
    <aside className={styles.side}>
      <div className={styles.partHead}>
        <div className={styles.partTitle}>
          {t('participants')} ({participants.length})
        </div>
        <Button
          variant="ghost"
          size="sm"
          icon={<Icon name="edit" size={12} />}
          disabled
          title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
        >
          {t('rename')}
        </Button>
      </div>
      <div className={styles.partList}>
        {participants.map((p) => (
          // biome-ignore lint/a11y/useKeyWithClickEvents: row click toggles inline edit; Enter/Escape handled inside the input
          <div
            key={p.id}
            className={styles.partRow}
            onClick={() => {
              if (editingId !== p.id) setEditingId(p.id);
            }}
          >
            <SubjectAvatar participant={p} size={36} />
            <div style={{ minWidth: 0 }}>
              {editingId === p.id ? (
                <Input
                  className={styles.editInput}
                  autoFocus
                  defaultValue={p.name ?? ''}
                  placeholder={fallbackName(p, lang)}
                  onBlur={(e) => commitRename(p.id, e.target.value)}
                  onKeyDown={(e) => onKeyDown(e, p.id)}
                />
              ) : (
                <>
                  <div className={styles.partName}>{fallbackName(p, lang)}</div>
                  <div className={styles.partOrig}>
                    {p.name ? p.label : lang === 'es' ? 'click para nombrar' : 'click to name'}
                  </div>
                </>
              )}
            </div>
            <div className={styles.partStats}>{Math.round(p.talkPct)}%</div>
            {participants.length >= 2 && (
              <div className={styles.menuWrap}>
                <button
                  type="button"
                  className={styles.menuBtn}
                  onClick={(e) => {
                    e.stopPropagation();
                    setMenuFor(menuFor === p.id ? null : p.id);
                  }}
                  aria-label={lang === 'es' ? 'Más opciones' : 'More options'}
                >
                  ···
                </button>
                {menuFor === p.id && (
                  <div className={styles.mergeMenu}>
                    {participants
                      .filter((o) => o.id !== p.id)
                      .map((o) => (
                        <button
                          key={o.id}
                          type="button"
                          onClick={() => {
                            void mergeSpeakers({ into: o.id, from: p.id });
                            setMenuFor(null);
                          }}
                        >
                          {t('speaker.merge')} {fallbackName(o, lang)}
                        </button>
                      ))}
                  </div>
                )}
              </div>
            )}
          </div>
        ))}
      </div>
      {aiChatVisible && (
        <div className={styles.aiPanel}>
          <div className={styles.aiHeader}>
            <div className={styles.aiIcon}>
              <Icon name="sparkles" size={12} stroke="white" />
            </div>
            <span>{t('aiAsk')}</span>
            <button
              type="button"
              className={styles.aiToggle}
              onClick={() => setAiOpen((v) => !v)}
              aria-expanded={aiOpen}
            >
              <Icon name={aiOpen ? 'chevDown' : 'chevRight'} size={14} />
            </button>
          </div>
          {aiOpen && (
            <>
              <div className={styles.aiBody}>
                <div className={`${styles.aiMsg} ${styles.aiMsgBot}`}>
                  {lang === 'es'
                    ? '¡Hola! Tengo cargada esta reunión. Puedes preguntar cualquier cosa sobre lo que se dijo.'
                    : 'Hi! I have this meeting loaded. Ask anything about what was said.'}
                </div>
                <div className={`${styles.aiMsg} ${styles.aiMsgUser}`}>
                  {lang === 'es'
                    ? '¿Cuáles fueron los principales bloqueos discutidos?'
                    : 'What were the main blockers discussed?'}
                </div>
                <div className={`${styles.aiMsg} ${styles.aiMsgBot}`}>
                  {lang === 'es'
                    ? 'Dos bloqueos principales: (1) timeout en la API de SAP al cargar > 5k registros, y (2) firma pendiente del cliente para acceso al ambiente productivo. Ambos tienen acciones asignadas.'
                    : 'Two main blockers: (1) SAP API timeout on > 5k records, and (2) pending client signature for production env. Both have assigned actions.'}
                </div>
              </div>
              <div className={styles.aiSuggested}>
                {([t('suggestedQ1'), t('suggestedQ2'), t('suggestedQ3')] as const).map((q) => (
                  <Button
                    key={q}
                    variant="ghost"
                    size="sm"
                    disabled
                    title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
                  >
                    {q}
                  </Button>
                ))}
              </div>
              <div className={styles.aiFooter}>
                <Input disabled placeholder={t('askPlaceholder')} value="" onChange={() => {}} />
                <Button
                  variant="primary"
                  size="icon"
                  disabled
                  title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
                >
                  <Icon name="send" size={14} stroke="currentColor" />
                </Button>
              </div>
            </>
          )}
        </div>
      )}
    </aside>
  );
}
