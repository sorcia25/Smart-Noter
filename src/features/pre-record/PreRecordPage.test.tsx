import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { toast } from '@/components/primitives/Toast/Toast';
import * as tauriCore from '@tauri-apps/api/core';
import PreRecordPage from './PreRecordPage';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_audio_devices') {
      return [
        {
          id: 'd-L-test',
          name: 'Test Speakers',
          kind: 'loopback',
          sampleRate: 48000,
          channels: 2,
          isDefault: true,
          recommended: true,
        },
      ];
    }
    if (cmd === 'start_preview') return null;
    if (cmd === 'stop_preview') return null;
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
    return null;
  }),
}));

vi.mock('@/components/primitives/Toast/Toast', () => ({
  toast: { error: vi.fn() },
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

function setup() {
  return render(
    <Provider store={store}>
      <MemoryRouter>
        <PreRecordPage />
      </MemoryRouter>
    </Provider>
  );
}

describe('PreRecordPage', () => {
  beforeEach(() => {
    vi.mocked(toast.error).mockClear();
  });

  it('renders the pre-record title', async () => {
    setup();
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Nueva grabación' })).toBeInTheDocument();
    });
  });

  it('exposes data-screen-label', () => {
    const { container } = setup();
    expect(container.querySelector('[data-screen-label="03 Pre-record"]')).toBeTruthy();
  });

  it('starts preview on device auto-select and stops on unmount', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockClear();
    const { unmount } = setup();
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('start_preview', {
        deviceId: 'd-L-test',
        captureMode: 'system',
      })
    );
    unmount();
    expect(invokeMock).toHaveBeenCalledWith('stop_preview');
  });

  it('start_preview rejection shows toast.error', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockImplementationOnce(async (cmd: string) => {
      if (cmd === 'list_audio_devices') {
        return [
          {
            id: 'd-L-test',
            name: 'Test Speakers',
            kind: 'loopback',
            sampleRate: 48000,
            channels: 2,
            isDefault: true,
            recommended: true,
          },
        ];
      }
      if (cmd === 'start_preview')
        throw { code: 'audio', message: { code: 'DeviceNotFound', message: 'no device' } };
      if (cmd === 'stop_preview') return null;
      if (cmd === 'list_templates') return [];
      if (cmd === 'get_settings') return null;
      return null;
    });
    setup();
    await waitFor(() => expect(toast.error).toHaveBeenCalled());
    expect(vi.mocked(toast.error).mock.calls[0]?.[0]).toBe('Error de captura de audio');
  });
});
