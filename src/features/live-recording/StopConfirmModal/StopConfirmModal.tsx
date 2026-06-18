import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { Input } from '@/components/primitives/Input/Input';
import { Modal } from '@/components/primitives/Modal/Modal';
import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import type { CaptureResult, MeetingDetail } from '@/ipc/bindings';
import { errorMessage, toAppError } from '@/ipc/error';
import { Paths } from '@/router/paths';
import { baseApi } from '@/store/api/base';
import { useAppDispatch } from '@/store/hooks';
import { fmtDuration } from '@/utils/format';
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import styles from './StopConfirmModal.module.css';

export interface StopConfirmModalProps {
  open: boolean;
  onClose: () => void;
  capture: CaptureResult;
  suggestedTitle: string;
  templateId: string;
  speakerHint: number | null;
}

function fmtBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1024 ** 3) return `${(b / 1024 / 1024).toFixed(1)} MB`;
  return `${(b / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

export function StopConfirmModal({
  open,
  onClose,
  capture,
  suggestedTitle,
  templateId,
  speakerHint,
}: StopConfirmModalProps) {
  const { t } = useT();
  const navigate = useNavigate();
  const dispatch = useAppDispatch();
  const [title, setTitle] = useState(suggestedTitle);
  const [busy, setBusy] = useState(false);

  const onSave = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const meeting = await invoke<MeetingDetail>('finalize_recording', {
        sessionId: capture.sessionId,
        title: title.trim(),
        templateId,
      });
      dispatch(baseApi.util.invalidateTags(['Meeting']));
      onClose();
      navigate(Paths.MeetingDetail(meeting.id), { state: { justRecorded: true, speakerHint } });
    } catch (err) {
      // Modal stays open — Discard remains the exit. Surface the error via toast.
      const ae = toAppError(err);
      toast.error(t('audioErrorTitle'), {
        id: `audio-error:${ae.code}`,
        description: errorMessage(ae, t),
      });
    } finally {
      setBusy(false);
    }
  };

  const onDiscard = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await invoke('discard_recording');
    } catch (err) {
      // Even on failure we close and navigate: the page's unmount discard and
      // the startup tmp-sweep reclaim the file, so the user is never soft-locked.
      // Surface the error via toast for visibility.
      const ae = toAppError(err);
      toast.error(t('audioErrorTitle'), {
        id: `audio-error:${ae.code}`,
        description: errorMessage(ae, t),
      });
    } finally {
      setBusy(false);
      onClose();
      navigate(Paths.Dashboard);
    }
  };

  return (
    <Modal
      open={open}
      onClose={busy ? () => {} : onDiscard}
      title={t('saveRecording')}
      subtitle={t('saveRecordingSub')}
      footer={
        <>
          <Button variant="danger" onClick={onDiscard} disabled={busy}>
            {t('discard')}
          </Button>
          <Button variant="primary" onClick={onSave} loading={busy} disabled={title.trim() === ''}>
            {t('save')}
          </Button>
        </>
      }
    >
      <Input
        label={t('meetingNameLabel')}
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        placeholder={t('meetingNamePh')}
        autoFocus
      />
      <div className={styles.summary}>
        <Icon name="clock" size={14} />
        <span>{fmtDuration(capture.durationSec)}</span>
        <span className={styles.sep} />
        <Icon name="download" size={14} />
        <span>{fmtBytes(capture.bytes)}</span>
      </div>
    </Modal>
  );
}
