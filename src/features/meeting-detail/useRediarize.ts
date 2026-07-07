import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import { errorMessage, toAppError } from '@/ipc/error';
import { baseApi } from '@/store/api/base';
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import { useDispatch } from 'react-redux';

/** Re-runs diarization on an existing meeting with a forced speaker count,
 *  keeping the transcript text. Invalidates the meeting cache on success. */
export function useRediarize(meetingId: string) {
  const dispatch = useDispatch();
  const { t } = useT();
  const [running, setRunning] = useState(false);

  const rediarize = async (speakerCount: number) => {
    if (running) return;
    setRunning(true);
    try {
      await invoke('rediarize_meeting', { meetingId, speakerCount });
      dispatch(baseApi.util.invalidateTags([{ type: 'Meeting', id: meetingId }]));
    } catch (err) {
      const ae = toAppError(err);
      toast.error(t('audioErrorTitle'), {
        id: `rediarize-error:${ae.code}`,
        description: errorMessage(ae, t),
      });
    } finally {
      setRunning(false);
    }
  };
  return { running, rediarize };
}
