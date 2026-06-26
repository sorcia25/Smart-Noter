import { SubjectAvatar } from '@/components/primitives/Avatar/Avatar';
import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { Input } from '@/components/primitives/Input/Input';
import { useT } from '@/i18n/useT';
import type { Participant } from '@/ipc/bindings';
import { useMergeSpeakersMutation, useRenameParticipantMutation } from '@/store/api/meetings.api';
import { useAppSelector } from '@/store/hooks';
import { type KeyboardEvent, useRef, useState } from 'react';
import { useChatStream } from '../useChatStream';
import styles from './SidePanel.module.css';

export interface SidePanelProps {
  participants: Participant[];
  meetingId: string;
}

function fallbackName(p: Participant, lang: 'es' | 'en'): string {
  if (p.name) return p.name;
  const suffix = p.label.replace(/^[a-zA-Z]+/, '');
  return lang === 'es'
    ? `Sujeto${suffix ? ` ${suffix}` : ''}`
    : `Subject${suffix ? ` ${suffix}` : ''}`;
}

export function SidePanel({ participants, meetingId }: SidePanelProps) {
  const { t, lang } = useT();
  const aiChatVisible = useAppSelector((s) => s.ui.aiChatVisible);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [menuFor, setMenuFor] = useState<string | null>(null);
  const [aiOpen, setAiOpen] = useState(true);
  const [inputValue, setInputValue] = useState('');
  const bodyRef = useRef<HTMLDivElement>(null);
  const [renameParticipant] = useRenameParticipantMutation();
  const [mergeSpeakers] = useMergeSpeakersMutation();
  const { messages, ask, busy } = useChatStream(meetingId);

  function handleSend() {
    const q = inputValue.trim();
    if (!q || busy) return;
    setInputValue('');
    void ask(q);
    // Scroll to bottom after next render
    setTimeout(() => {
      bodyRef.current?.scrollTo({ top: bodyRef.current.scrollHeight, behavior: 'smooth' });
    }, 50);
  }

  function handleInputKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === 'Enter') handleSend();
  }

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
              <div className={styles.aiBody} ref={bodyRef}>
                {messages.length === 0 && !busy && (
                  <div className={`${styles.aiMsg} ${styles.aiMsgBot}`}>{t('aiAsk')}</div>
                )}
                {messages.map((m, i) => (
                  <div
                    // biome-ignore lint/suspicious/noArrayIndexKey: messages are append-only; index is stable per session
                    key={i}
                    className={`${styles.aiMsg} ${m.role === 'user' ? styles.aiMsgUser : styles.aiMsgBot}`}
                  >
                    {m.error
                      ? t('chatError')
                      : m.content === '' && m.role === 'assistant'
                        ? t('chatThinking')
                        : m.content}
                  </div>
                ))}
              </div>
              {!busy && (
                <div className={styles.aiSuggested}>
                  {([t('suggestedQ1'), t('suggestedQ2'), t('suggestedQ3')] as const).map((q) => (
                    <Button
                      key={q}
                      variant="ghost"
                      size="sm"
                      onClick={() => {
                        void ask(q);
                        setTimeout(() => {
                          bodyRef.current?.scrollTo({
                            top: bodyRef.current.scrollHeight,
                            behavior: 'smooth',
                          });
                        }, 50);
                      }}
                    >
                      {q}
                    </Button>
                  ))}
                </div>
              )}
              <div className={styles.aiFooter}>
                <Input
                  placeholder={t('chatPlaceholder')}
                  value={inputValue}
                  onChange={(e) => setInputValue(e.target.value)}
                  onKeyDown={handleInputKeyDown}
                  disabled={busy}
                />
                <Button
                  variant="primary"
                  size="icon"
                  disabled={busy || !inputValue.trim()}
                  onClick={handleSend}
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
