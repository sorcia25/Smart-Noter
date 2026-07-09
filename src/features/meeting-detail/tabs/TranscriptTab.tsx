import { SubjectAvatar } from '@/components/primitives/Avatar/Avatar';
import { Button } from '@/components/primitives/Button/Button';
import { Chip } from '@/components/primitives/Chip/Chip';
import { Icon } from '@/components/primitives/Icon/Icon';
import { Modal } from '@/components/primitives/Modal/Modal';
import { useT } from '@/i18n/useT';
import type { MeetingAudioInfo, MeetingDetail, Participant } from '@/ipc/bindings';
import {
  useCreateSpeakerMutation,
  useMergeSpeakersMutation,
  useReassignLinesMutation,
} from '@/store/api/meetings.api';
import { useGetSettingsQuery } from '@/store/api/settings.api';
import { pickL } from '@/utils/format';
import { invoke } from '@tauri-apps/api/core';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useLocation } from 'react-router-dom';
import { useRediarize } from '../useRediarize';
import { useTranscription } from '../useTranscription';
import styles from './TranscriptTab.module.css';

export interface TranscriptTabProps {
  meeting: MeetingDetail;
}

function speakerLabel(p: Participant | undefined, lang: 'es' | 'en'): string {
  if (!p) return '—';
  if (p.name) return p.name;
  const suffix = p.label.replace(/^[a-zA-Z]+/, '');
  return lang === 'es'
    ? `Sujeto${suffix ? ` ${suffix}` : ''}`
    : `Subject${suffix ? ` ${suffix}` : ''}`;
}

interface SpeakerMenuProps {
  participants: Participant[];
  lang: 'es' | 'en';
  newSpeakerLabel: string;
  onSelect: (speakerId: string) => void;
  onNew: () => void;
  onClose: () => void;
}

function SpeakerMenu({
  participants,
  lang,
  newSpeakerLabel,
  onSelect,
  onNew,
  onClose,
}: SpeakerMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    }
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [onClose]);

  return (
    <div ref={ref} className={styles.speakerMenu}>
      {participants.map((p) => (
        <button
          key={p.id}
          className={styles.speakerMenuItem}
          onClick={() => onSelect(p.id)}
          type="button"
        >
          {speakerLabel(p, lang)}
        </button>
      ))}
      <button
        className={`${styles.speakerMenuItem} ${styles.speakerMenuItemNew}`}
        onClick={onNew}
        type="button"
      >
        {newSpeakerLabel}
      </button>
    </div>
  );
}

export function TranscriptTab({ meeting }: TranscriptTabProps) {
  const { t, lang } = useT();
  const location = useLocation();
  const justRecorded = (location.state as { justRecorded?: boolean } | null)?.justRecorded ?? false;
  const speakerHint =
    (location.state as { speakerHint?: number | null } | null)?.speakerHint ?? null;
  const { data: settings } = useGetSettingsQuery();
  const { status, pct, start, cancel } = useTranscription(meeting.id);

  // --- Reassign state ---
  const [reassignLines] = useReassignLinesMutation();
  const [createSpeaker] = useCreateSpeakerMutation();

  // --- Merge speakers (modal) state ---
  const [mergeSpeakers] = useMergeSpeakersMutation();
  const [mergeOpen, setMergeOpen] = useState(false);
  const [mergeFrom, setMergeFrom] = useState('');
  const [mergeInto, setMergeInto] = useState('');
  const closeMerge = () => {
    setMergeOpen(false);
    setMergeFrom('');
    setMergeInto('');
  };

  // --- Re-diarize (forced speaker count) state ---
  const { running: rediarizing, rediarize } = useRediarize(meeting.id);
  // null while the field is transiently empty (user is clearing/retyping it) —
  // mirrors StopConfirmModal's speakerCount so clear()+type() doesn't fight the
  // controlled input by snapping back to a clamped value on every keystroke.
  const [reN, setReN] = useState<number | null>(2);
  const reNValue = Math.max(1, Math.min(10, reN ?? 1));
  const [confirmOpen, setConfirmOpen] = useState(false);
  // undefined = still loading, null = no audio saved — either way the control
  // stays disabled until we positively confirm audio is available.
  const [audioInfo, setAudioInfo] = useState<MeetingAudioInfo | null | undefined>(undefined);
  useEffect(() => {
    let cancelled = false;
    invoke<MeetingAudioInfo | null>('get_meeting_audio', { meetingId: meeting.id })
      .then((res) => {
        if (!cancelled) setAudioInfo(res ?? null);
      })
      .catch(() => {
        if (!cancelled) setAudioInfo(null);
      });
    return () => {
      cancelled = true;
    };
  }, [meeting.id]);
  const hasAudio = Boolean(audioInfo);

  // Select-lines mode
  const [selectMode, setSelectMode] = useState(false);
  const [selected, setSelected] = useState<Set<number>>(new Set());

  // Per-line menu: which line id is open (null = none)
  const [openMenuLineId, setOpenMenuLineId] = useState<number | null>(null);
  // Bulk reassign menu open
  const [bulkMenuOpen, setBulkMenuOpen] = useState(false);

  function toggleSelectMode() {
    setSelectMode((v) => !v);
    setSelected(new Set());
    setOpenMenuLineId(null);
    setBulkMenuOpen(false);
  }

  function toggleLine(id: number) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function selectAllOf(speakerId: string) {
    setSelected(new Set(lines.filter((l) => l.speakerId === speakerId).map((l) => l.id)));
  }

  async function reassignTo(speakerId: string, lineIds: number[]) {
    await reassignLines({ lineIds, speakerId });
    setSelected(new Set());
    setOpenMenuLineId(null);
    setBulkMenuOpen(false);
  }

  async function reassignToNew(lineIds: number[]) {
    const newId = await createSpeaker({ meetingId: meeting.id }).unwrap();
    await reassignLines({ lineIds, speakerId: newId });
    setSelected(new Set());
    setOpenMenuLineId(null);
    setBulkMenuOpen(false);
  }

  const byId = useMemo(() => {
    const map = new Map<string, Participant>();
    for (const p of meeting.participants) map.set(p.id, p);
    return map;
  }, [meeting.participants]);

  const lines = meeting.transcript; // real data only — no more mock synthesis

  // Distinct speakers currently in the transcript, used as the baseline to
  // detect a no-op re-diarize (backend returned a count that didn't grow).
  const usedSpeakerCount = useMemo(() => new Set(lines.map((l) => l.speakerId)).size, [lines]);

  // Auto-trigger ONLY for a freshly-saved recording with the setting on.
  const autoStarted = useRef(false);
  useEffect(() => {
    if (autoStarted.current) return;
    if (lines.length === 0 && justRecorded && settings?.autoTranscribe && status === 'idle') {
      autoStarted.current = true;
      void start(speakerHint);
    }
  }, [lines.length, justRecorded, settings?.autoTranscribe, status, start, speakerHint]);

  return (
    <div className={styles.card}>
      <div className={styles.cardHead}>
        <div className={styles.cardHeadLeft}>
          <Icon name="mic" size={14} stroke="var(--accent)" />
          <span>{t('transcript')}</span>
          <Chip variant="accent" disabled>
            99.2% {t('fidelity')}
          </Chip>
        </div>
        {lines.length > 0 && (
          <div className={styles.cardHeadRight}>
            <input
              type="number"
              min={1}
              max={10}
              aria-label={t('rediarizeCountLabel')}
              value={reN === null ? '' : String(reN)}
              onChange={(e) => {
                const v = e.target.value.trim();
                setReN(v === '' ? null : Math.max(1, Math.min(10, Number.parseInt(v, 10) || 1)));
              }}
              disabled={!hasAudio || rediarizing}
            />
            <Button
              variant="default"
              disabled={!hasAudio || rediarizing}
              title={hasAudio ? undefined : t('rediarizeNoAudio')}
              onClick={() => setConfirmOpen(true)}
            >
              {rediarizing ? t('rediarizeRunning') : t('rediarizeCta')}
            </Button>
            {meeting.participants.length >= 2 && (
              <Button variant="default" onClick={() => setMergeOpen(true)}>
                {t('speaker.mergeCta')}
              </Button>
            )}
            <Button variant={selectMode ? 'primary' : 'default'} onClick={toggleSelectMode}>
              {t('speaker.selectLines')}
            </Button>
          </div>
        )}
      </div>

      {/* Per-speaker select-all shortcut */}
      {selectMode && (
        <div className={styles.selectAllRow}>
          {meeting.participants.map((p) => (
            <button
              key={p.id}
              type="button"
              className={styles.selectAllChip}
              title={t('speaker.selectAllOf')}
              onClick={() => selectAllOf(p.id)}
            >
              {speakerLabel(p, lang)}
            </button>
          ))}
        </div>
      )}

      {/* Bulk reassign toolbar */}
      {selectMode && selected.size > 0 && (
        <div className={styles.selectToolbar}>
          <span className={styles.selectCount}>{selected.size}</span>
          <div className={styles.bulkMenuWrapper}>
            <Button variant="primary" onClick={() => setBulkMenuOpen((v) => !v)}>
              {t('speaker.applyReassign')}
            </Button>
            {bulkMenuOpen && (
              <SpeakerMenu
                participants={meeting.participants}
                lang={lang}
                newSpeakerLabel={t('speaker.newSpeaker')}
                onSelect={(sid) => void reassignTo(sid, [...selected])}
                onNew={() => void reassignToNew([...selected])}
                onClose={() => setBulkMenuOpen(false)}
              />
            )}
          </div>
        </div>
      )}

      {lines.length > 0 ? (
        <div>
          {lines.map((l) => {
            const sp = byId.get(l.speakerId);
            const isSelected = selected.has(l.id);
            return (
              <div
                className={[
                  styles.line,
                  selectMode ? styles.lineWithCheckbox : '',
                  isSelected ? styles.lineSelected : '',
                ]
                  .filter(Boolean)
                  .join(' ')}
                key={l.id}
              >
                {selectMode && (
                  <div className={styles.checkboxCol}>
                    <input
                      type="checkbox"
                      className={styles.lineCheckbox}
                      checked={isSelected}
                      onChange={() => toggleLine(l.id)}
                      aria-label={`Select line ${l.id}`}
                    />
                  </div>
                )}
                <div className={styles.who}>
                  {sp ? (
                    <SubjectAvatar participant={sp} size={32} />
                  ) : (
                    <div style={{ width: 32, height: 32 }} />
                  )}
                  <div className={styles.time}>{l.t}</div>
                </div>
                <div>
                  <div className={styles.speakerRow}>
                    <div className={styles.speaker}>{speakerLabel(sp, lang)}</div>
                    <div className={styles.lineMenuWrapper}>
                      <button
                        className={styles.reassignBtn}
                        title={t('speaker.reassign')}
                        type="button"
                        onClick={() => setOpenMenuLineId(openMenuLineId === l.id ? null : l.id)}
                        aria-label={t('speaker.reassign')}
                      >
                        &#8942;
                      </button>
                      {openMenuLineId === l.id && (
                        <SpeakerMenu
                          participants={meeting.participants}
                          lang={lang}
                          newSpeakerLabel={t('speaker.newSpeaker')}
                          onSelect={(sid) => void reassignTo(sid, [l.id])}
                          onNew={() => void reassignToNew([l.id])}
                          onClose={() => setOpenMenuLineId(null)}
                        />
                      )}
                    </div>
                  </div>
                  <div className={styles.text}>{pickL(l.text, lang)}</div>
                </div>
              </div>
            );
          })}
        </div>
      ) : status === 'running' ? (
        <div className={styles.empty}>
          <div>
            {t('transcribe.running')} {pct}%
          </div>
          <Button variant="default" onClick={() => void cancel()}>
            {t('transcribe.cancel')}
          </Button>
        </div>
      ) : (
        <div className={styles.empty}>
          <div>
            {lang === 'es'
              ? 'Sin transcripción para esta reunión.'
              : 'No transcript for this meeting.'}
          </div>
          <Button variant="primary" onClick={() => void start()}>
            {t('transcribe.cta')}
          </Button>
        </div>
      )}

      <Modal
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        title={t('rediarizeConfirmTitle')}
        footer={
          <>
            <Button variant="default" onClick={() => setConfirmOpen(false)}>
              {t('cancel')}
            </Button>
            <Button
              variant="primary"
              onClick={() => {
                setConfirmOpen(false);
                void rediarize(reNValue, usedSpeakerCount);
              }}
            >
              {t('rediarizeConfirmCta')}
            </Button>
          </>
        }
      >
        {t('rediarizeConfirmBody')}
      </Modal>

      <Modal
        open={mergeOpen}
        onClose={closeMerge}
        title={t('speaker.mergeTitle')}
        footer={
          <>
            <Button variant="default" onClick={closeMerge}>
              {t('cancel')}
            </Button>
            <Button
              variant="primary"
              disabled={!mergeFrom || !mergeInto || mergeFrom === mergeInto}
              onClick={async () => {
                await mergeSpeakers({ into: mergeInto, from: mergeFrom });
                closeMerge();
              }}
            >
              {t('speaker.mergeCta')}
            </Button>
          </>
        }
      >
        <div className={styles.mergeField}>
          <label className={styles.mergeLabel} htmlFor="merge-from-select">
            {t('speaker.mergeFromLabel')}
          </label>
          <select
            id="merge-from-select"
            className={styles.mergeSelect}
            value={mergeFrom}
            onChange={(e) => setMergeFrom(e.target.value)}
          >
            <option value="">—</option>
            {meeting.participants.map((p) => (
              <option key={p.id} value={p.id}>
                {speakerLabel(p, lang)}
              </option>
            ))}
          </select>
        </div>
        <div className={styles.mergeField}>
          <label className={styles.mergeLabel} htmlFor="merge-into-select">
            {t('speaker.mergeIntoLabel')}
          </label>
          <select
            id="merge-into-select"
            className={styles.mergeSelect}
            value={mergeInto}
            onChange={(e) => setMergeInto(e.target.value)}
          >
            <option value="">—</option>
            {meeting.participants.map((p) => (
              <option key={p.id} value={p.id}>
                {speakerLabel(p, lang)}
              </option>
            ))}
          </select>
        </div>
      </Modal>
    </div>
  );
}
