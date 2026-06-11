import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { Input } from '@/components/primitives/Input/Input';
import { Modal } from '@/components/primitives/Modal/Modal';
import { useT } from '@/i18n/useT';
import type { CaptureResult, MeetingDetail } from '@/ipc/bindings';
import { Paths } from '@/router/paths';
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
}

function fmtBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${(b / 1024 / 1024).toFixed(1)} MB`;
}

export function StopConfirmModal({
  open,
  onClose,
  capture,
  suggestedTitle,
  templateId,
}: StopConfirmModalProps) {
  const { t } = useT();
  const navigate = useNavigate();
  const [title, setTitle] = useState(suggestedTitle);

  const onSave = async () => {
    const meeting = await invoke<MeetingDetail>('finalize_recording', {
      sessionId: capture.sessionId,
      title: title.trim(),
      templateId,
    });
    onClose();
    navigate(Paths.MeetingDetail(meeting.id));
  };

  const onDiscard = async () => {
    await invoke('discard_recording');
    onClose();
    navigate(Paths.Dashboard);
  };

  return (
    <Modal
      open={open}
      onClose={onDiscard}
      title={t('saveRecording')}
      subtitle={t('saveRecordingSub')}
      footer={
        <>
          <Button variant="danger" onClick={onDiscard}>
            {t('discard')}
          </Button>
          <Button variant="primary" onClick={onSave} disabled={title.trim() === ''}>
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
