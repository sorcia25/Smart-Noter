import { useCallback, useEffect, useState } from 'react';

export interface UseLiveTimer {
  elapsed: number;
  paused: boolean;
  togglePause: () => void;
  reset: () => void;
}

export function useLiveTimer(startAt = 0): UseLiveTimer {
  const [elapsed, setElapsed] = useState(startAt);
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    if (paused) return;
    const id = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(id);
  }, [paused]);

  const togglePause = useCallback(() => setPaused((p) => !p), []);
  const reset = useCallback(() => {
    setElapsed(0);
    setPaused(false);
  }, []);

  return { elapsed, paused, togglePause, reset };
}
