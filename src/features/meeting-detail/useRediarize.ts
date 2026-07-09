import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import { errorMessage, toAppError } from '@/ipc/error';
import { baseApi } from '@/store/api/base';
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import { useDispatch } from 'react-redux';

/** Re-runs diarization on an existing meeting with a forced speaker count,
 *  keeping the transcript text. Invalidates the meeting cache on success.
 *  `prevSpeakerCount` is the number of distinct speakers already present in the
 *  transcript; if the backend's resulting speaker count doesn't exceed it, the
 *  re-diarize was a no-op and we tell the user so instead of failing silently. */
export function useRediarize(meetingId: string) {
  const dispatch = useDispatch();
  const { t } = useT();
  const [running, setRunning] = useState(false);

  const rediarize = async (speakerCount: number, prevSpeakerCount: number) => {
    if (running) return;
    setRunning(true);
    try {
      const count = await invoke<number>('rediarize_meeting', { meetingId, speakerCount });
      dispatch(baseApi.util.invalidateTags([{ type: 'Meeting', id: meetingId }]));
      if (count <= prevSpeakerCount) {
        toast.info(t('rediarizeNoChangeTitle'), {
          id: `rediarize-noop:${meetingId}`,
          description: t('rediarizeNoChangeBody'),
        });
      }
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
