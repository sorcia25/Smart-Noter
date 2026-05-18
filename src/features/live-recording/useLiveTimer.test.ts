import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLiveTimer } from './useLiveTimer';

describe('useLiveTimer', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('starts at the given value', () => {
    const { result } = renderHook(() => useLiveTimer(42));
    expect(result.current.elapsed).toBe(42);
    expect(result.current.paused).toBe(false);
  });

  it('increments every second when not paused', () => {
    const { result } = renderHook(() => useLiveTimer(0));
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(result.current.elapsed).toBe(3);
  });

  it('stops incrementing when paused', () => {
    const { result } = renderHook(() => useLiveTimer(0));
    act(() => {
      result.current.togglePause();
    });
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(result.current.elapsed).toBe(0);
    expect(result.current.paused).toBe(true);
  });

  it('resets to zero and unpauses', () => {
    const { result } = renderHook(() => useLiveTimer(0));
    act(() => {
      vi.advanceTimersByTime(2000);
      result.current.togglePause();
    });
    expect(result.current.elapsed).toBe(2);
    expect(result.current.paused).toBe(true);
    act(() => {
      result.current.reset();
    });
    expect(result.current.elapsed).toBe(0);
    expect(result.current.paused).toBe(false);
  });
});
