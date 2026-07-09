import { toast } from '@/components/primitives/Toast/Toast';
import { store } from '@/store';
import * as tauriCore from '@tauri-apps/api/core';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { useRediarize } from './useRediarize';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));
vi.mock('@/components/primitives/Toast/Toast', () => ({
  toast: { error: vi.fn(), info: vi.fn() },
}));

function wrapper({ children }: { children: ReactNode }) {
  return <Provider store={store}>{children}</Provider>;
}

describe('useRediarize no-op toast', () => {
  it('shows an info toast when the resulting count did not increase', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(2);
    const toastInfoMock = vi.mocked(toast.info);
    toastInfoMock.mockClear();

    const { result } = renderHook(() => useRediarize('m-1'), { wrapper });

    await act(async () => {
      await result.current.rediarize(7, 2);
    });

    expect(toastInfoMock).toHaveBeenCalledTimes(1);
  });

  it('does NOT show the info toast when the count increased', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(5);
    const toastInfoMock = vi.mocked(toast.info);
    toastInfoMock.mockClear();

    const { result } = renderHook(() => useRediarize('m-1'), { wrapper });

    await act(async () => {
      await result.current.rediarize(7, 2);
    });

    expect(toastInfoMock).not.toHaveBeenCalled();
  });
});
