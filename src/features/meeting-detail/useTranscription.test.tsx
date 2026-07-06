import { toast } from '@/components/primitives/Toast/Toast';
import { store } from '@/store';
import * as tauriCore from '@tauri-apps/api/core';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { useTranscription } from './useTranscription';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'get_transcription_state') return null;
    return null;
  }),
}));
vi.mock('@/components/primitives/Toast/Toast', () => ({
  toast: { error: vi.fn(), info: vi.fn() },
}));

function wrapper({ children }: { children: ReactNode }) {
  return <Provider store={store}>{children}</Provider>;
}

describe('useTranscription start()', () => {
  it('treats a TranscriptionBusy rejection as a benign re-attach: no toast, stays running', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_transcription_state') return null;
      if (cmd === 'transcribe_meeting') {
        return Promise.reject({
          code: 'transcription',
          message: { code: 'TranscriptionBusy', message: 'a transcription is already running' },
        });
      }
      return null;
    });
    const toastErrorMock = vi.mocked(toast.error);
    toastErrorMock.mockClear();

    const { result } = renderHook(() => useTranscription('m-1'), { wrapper });

    await act(async () => {
      await result.current.start();
    });

    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(result.current.status).toBe('running');
  });

  it('surfaces a NON-busy error as a toast and resets status to idle', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_transcription_state') return null;
      if (cmd === 'transcribe_meeting') {
        return Promise.reject({
          code: 'transcription',
          message: { code: 'InferenceFailed', message: 'model crashed' },
        });
      }
      return null;
    });
    const toastErrorMock = vi.mocked(toast.error);
    toastErrorMock.mockClear();

    const { result } = renderHook(() => useTranscription('m-1'), { wrapper });

    await act(async () => {
      await result.current.start();
    });

    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(result.current.status).toBe('idle');
  });
});
