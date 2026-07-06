import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import { errorMessage, toAppError } from '@/ipc/error';
import { baseApi } from '@/store/api/base';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useState } from 'react';
import { useDispatch } from 'react-redux';

export type TxStatus = 'idle' | 'running' | 'done' | 'error';

/** Subscribes to transcription events for one meeting, re-attaches to a running
 *  job on mount, and exposes start/cancel. Mirrors Sub-2's listen() pattern. */
export function useTranscription(meetingId: string) {
  const dispatch = useDispatch();
  const { t } = useT();
  const [status, setStatus] = useState<TxStatus>('idle');
  const [pct, setPct] = useState(0);

  useEffect(() => {
    let cancelled = false;
    const unsubs: Array<() => void> = [];
    const mine = (m: string) => m === meetingId;
    const sub = <T>(ev: string, cb: (p: T) => void) => {
      listen<T>(ev, (e) => {
        if (!cancelled) cb(e.payload);
      })
        .then((un) => {
          if (cancelled) un();
          else unsubs.push(un);
        })
        .catch(() => {}); // no Tauri in tests/browser
    };
    sub<{ meetingId: string; pct: number }>('transcription:progress', (p) => {
      if (mine(p.meetingId)) {
        setStatus('running');
        setPct(p.pct);
      }
    });
    sub<{ meetingId: string }>('transcription:completed', (p) => {
      if (!mine(p.meetingId)) return;
      setStatus('done');
      setPct(100);
      dispatch(baseApi.util.invalidateTags([{ type: 'Meeting', id: meetingId }]));
    });
    sub<{ meetingId: string; code: string; message: string }>('transcription:failed', (p) => {
      if (!mine(p.meetingId)) return;
      setStatus('error');
      const ae = toAppError({ code: p.code, message: p.message });
      toast.error(t('audioErrorTitle'), {
        id: `transcription-error:${ae.code}`,
        description: errorMessage(ae, t),
      });
    });
    sub<{ meetingId: string }>('transcription:cancelled', (p) => {
      if (mine(p.meetingId)) {
        setStatus('idle');
        setPct(0);
      }
    });
    sub<{ meetingId: string; code: string; message: string }>('diarization:degraded', (p) => {
      if (!mine(p.meetingId)) return;
      toast.info(
        p.code === 'ModelNotDownloaded' ? t('diarize.modelsMissing') : t('diarize.degraded')
      );
    });

    invoke<{ meetingId: string; pct: number } | null>('get_transcription_state')
      .then((s) => {
        if (!cancelled && s && s.meetingId === meetingId) {
          setStatus('running');
          setPct(s.pct);
        }
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      for (const u of unsubs) u();
    };
  }, [meetingId, dispatch, t]);

  const start = async (speakerCountHint?: number | null) => {
    setStatus('running');
    setPct(0);
    try {
      await invoke('transcribe_meeting', { meetingId, speakerCountHint: speakerCountHint ?? null });
    } catch (err) {
      const ae = toAppError(err);
      // TranscriptionBusy isn't a user error: a job is already running for this
      // meeting (a tab-switch remount re-fired the auto-trigger, or a double-click).
      // Stay 'running' so the progress listener re-attaches; don't show a toast.
      if (ae.code === 'TranscriptionBusy') {
        setStatus('running');
        return;
      }
      setStatus('idle');
      toast.error(t('audioErrorTitle'), {
        id: `transcription-error:${ae.code}`,
        description: errorMessage(ae, t),
      });
    }
  };
  const cancel = async () => {
    try {
      await invoke('cancel_transcription', { meetingId });
    } catch {
      /* ignore */
    }
  };

  return { status, pct, start, cancel };
}
