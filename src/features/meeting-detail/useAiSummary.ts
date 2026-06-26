import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import { baseApi } from '@/store/api/base';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useState } from 'react';
import { useDispatch } from 'react-redux';

export type SummaryStatus = 'idle' | 'running' | 'failed';

/** Subscribes to summary events for one meeting, re-attaches to a running
 *  job on mount, and exposes the current status + progress. */
export function useAiSummary(meetingId: string) {
  const dispatch = useDispatch();
  const { t } = useT();
  const [status, setStatus] = useState<SummaryStatus>('idle');
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

    sub<{ meetingId: string; pct: number }>('summary:progress', (p) => {
      if (mine(p.meetingId)) {
        setStatus('running');
        setPct(p.pct);
      }
    });

    sub<{ meetingId: string }>('summary:completed', (p) => {
      if (!mine(p.meetingId)) return;
      setStatus('idle');
      setPct(100);
      dispatch(baseApi.util.invalidateTags([{ type: 'Meeting', id: meetingId }]));
    });

    sub<{ meetingId: string; code?: string; message?: string }>('summary:failed', (p) => {
      if (!mine(p.meetingId)) return;
      setStatus('failed');
      toast.error(t('summaryFailed'));
    });

    // Re-hydrate if a summary job is already running for this meeting
    invoke<string | null>('get_summary_state')
      .then((runningId) => {
        if (!cancelled && runningId && runningId === meetingId) {
          setStatus('running');
        }
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      for (const u of unsubs) u();
    };
  }, [meetingId, dispatch, t]);

  return { status, pct };
}
