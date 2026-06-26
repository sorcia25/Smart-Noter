import { useAskMeetingMutation, useListChatQuery } from '@/store/api/ai.api';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useRef, useState } from 'react';

export interface ChatMsg {
  role: 'user' | 'assistant';
  content: string;
  error?: boolean;
}

/** Subscribes to chat:token / chat:done / chat:error events for one meeting,
 *  hydrates prior history via list_chat, and exposes ask / busy / messages. */
export function useChatStream(meetingId: string) {
  const [messages, setMessages] = useState<ChatMsg[]>([]);
  const [busy, setBusy] = useState(false);
  const historyLoaded = useRef(false);

  // RTK hooks
  const { data: history } = useListChatQuery({ meetingId }, { skip: !meetingId });
  const [askMeeting] = useAskMeetingMutation();

  // Hydrate history once on mount (before any ask)
  useEffect(() => {
    if (history && !historyLoaded.current) {
      historyLoaded.current = true;
      setMessages(history.map((m) => ({ role: m.role, content: m.content })));
    }
  }, [history]);

  // Subscribe to streaming events
  useEffect(() => {
    let cancelled = false;
    const unsubs: Array<() => void> = [];
    const mine = (id: string) => id === meetingId;

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

    // Append streamed token to last (in-progress) assistant message
    sub<{ meetingId: string; token: string }>('chat:token', (p) => {
      if (!mine(p.meetingId)) return;
      setMessages((prev) => {
        const last = prev[prev.length - 1];
        if (!last || last.role !== 'assistant') return prev;
        return [...prev.slice(0, -1), { ...last, content: last.content + p.token }];
      });
    });

    // Mark generation complete
    sub<{ meetingId: string }>('chat:done', (p) => {
      if (!mine(p.meetingId)) return;
      setBusy(false);
    });

    // Mark last assistant message as error
    sub<{ meetingId: string; message: string }>('chat:error', (p) => {
      if (!mine(p.meetingId)) return;
      setBusy(false);
      setMessages((prev) => {
        const last = prev[prev.length - 1];
        if (!last || last.role !== 'assistant') return prev;
        return [...prev.slice(0, -1), { ...last, error: true }];
      });
    });

    return () => {
      cancelled = true;
      for (const u of unsubs) u();
    };
  }, [meetingId]);

  const ask = useCallback(
    async (question: string) => {
      if (busy || !question.trim()) return;

      // Seed user message + empty assistant placeholder
      setMessages((prev) => [
        ...prev,
        { role: 'user', content: question },
        { role: 'assistant', content: '' },
      ]);
      setBusy(true);

      try {
        await askMeeting({ meetingId, question }).unwrap();
      } catch {
        // Backend error before streaming starts → mark assistant message as error
        setBusy(false);
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (!last || last.role !== 'assistant') return prev;
          return [...prev.slice(0, -1), { ...last, error: true }];
        });
      }
    },
    [busy, meetingId, askMeeting]
  );

  return { messages, ask, busy };
}
