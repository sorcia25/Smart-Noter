import { store } from '@/store';
import { baseApi } from '@/store/api/base';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { toast } from '@/components/primitives/Toast/Toast';
import * as tauriCore from '@tauri-apps/api/core';
import PreRecordPage from './PreRecordPage';

// Spy on navigate calls so tests can assert on the state passed to navigate().
const navigateSpy = vi.fn();
vi.mock('react-router-dom', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return {
    ...actual,
    useNavigate: () => navigateSpy,
  };
});

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
    navigateSpy.mockClear();
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

  describe('captureMode derivation (settings Mix preference)', () => {
    beforeEach(() => {
      // Reset RTK Query cache so each test gets a fresh fetch from the mocked invoke.
      store.dispatch(baseApi.util.resetApiState());
    });

    // Helper: override get_settings to return a specific captureMode value and
    // optionally swap the device kind. Returns the invokeMock for further assertions.
    function setupWithSettings(
      settingsCaptureMode: string,
      deviceKind: 'loopback' | 'input' = 'loopback'
    ) {
      const invokeMock = vi.mocked(tauriCore.invoke);
      invokeMock.mockClear();
      invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === 'list_audio_devices') {
          return [
            {
              id: deviceKind === 'input' ? 'd-I-test' : 'd-L-test',
              name: deviceKind === 'input' ? 'Test Mic' : 'Test Speakers',
              kind: deviceKind,
              sampleRate: 48000,
              channels: deviceKind === 'input' ? 1 : 2,
              isDefault: true,
              recommended: true,
            },
          ];
        }
        if (cmd === 'get_settings') {
          return {
            theme: 'dark',
            accent: '#7C3AED',
            language: 'es',
            avatarStyle: 'initials',
            aiChatVisible: true,
            captureMode: settingsCaptureMode,
            defaultDevice: '',
            recordingQuality: 'WAV',
            runLocal: true,
            autoDeleteAudio: false,
            transcriptionProvider: 'local',
            transcriptionModel: 'base',
            defaultTemplate: 'tecnica',
          };
        }
        if (cmd === 'start_preview') return null;
        if (cmd === 'stop_preview') return null;
        if (cmd === 'list_templates') return [];
        return null;
      });
      render(
        <Provider store={store}>
          <MemoryRouter>
            <PreRecordPage />
          </MemoryRouter>
        </Provider>
      );
      return invokeMock;
    }

    // Case 1: settings.captureMode === 'mix' + loopback → recording navigates with captureMode 'mix'
    it('settings.captureMode mix + loopback device → start navigates with captureMode mix', async () => {
      setupWithSettings('mix', 'loopback');
      // Wait for device to auto-select (preview fires)
      await waitFor(() =>
        expect(vi.mocked(tauriCore.invoke)).toHaveBeenCalledWith('start_preview', expect.anything())
      );
      const startBtn = await screen.findByRole('button', { name: /Iniciar|Start/i });
      await userEvent.click(startBtn);
      expect(navigateSpy).toHaveBeenCalledOnce();
      const [, navOptions] = navigateSpy.mock.calls[0] as [
        string,
        { state: Record<string, unknown> },
      ];
      expect(navOptions.state.captureMode).toBe('mix');
    });

    // Case 2: settings.captureMode === 'mix' + input device → input choice wins → captureMode 'mic'
    it('settings.captureMode mix + input device → start navigates with captureMode mic', async () => {
      setupWithSettings('mix', 'input');
      await waitFor(() =>
        expect(vi.mocked(tauriCore.invoke)).toHaveBeenCalledWith('start_preview', expect.anything())
      );
      const startBtn = await screen.findByRole('button', { name: /Iniciar|Start/i });
      await userEvent.click(startBtn);
      expect(navigateSpy).toHaveBeenCalledOnce();
      const [, navOptions] = navigateSpy.mock.calls[0] as [
        string,
        { state: Record<string, unknown> },
      ];
      expect(navOptions.state.captureMode).toBe('mic');
    });

    // Case 3: settings.captureMode !== 'mix' (e.g. 'system') + loopback → unchanged 'system'
    // NOTE: The top-level 'starts preview' test already covers get_settings returning null
    // (settings undefined) with a loopback device → captureMode 'system'. This case makes
    // the non-mix explicit with settings.captureMode === 'system'.
    it('settings.captureMode system + loopback device → start navigates with captureMode system', async () => {
      setupWithSettings('system', 'loopback');
      await waitFor(() =>
        expect(vi.mocked(tauriCore.invoke)).toHaveBeenCalledWith('start_preview', expect.anything())
      );
      const startBtn = await screen.findByRole('button', { name: /Iniciar|Start/i });
      await userEvent.click(startBtn);
      expect(navigateSpy).toHaveBeenCalledOnce();
      const [, navOptions] = navigateSpy.mock.calls[0] as [
        string,
        { state: Record<string, unknown> },
      ];
      expect(navOptions.state.captureMode).toBe('system');
    });

    // Case 4: preview is NOT affected by mix preference — start_preview always uses previewMode
    it('settings.captureMode mix + loopback → start_preview invoked with captureMode system (not mix)', async () => {
      const invokeMock = setupWithSettings('mix', 'loopback');
      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith('start_preview', {
          deviceId: 'd-L-test',
          captureMode: 'system',
        })
      );
    });

    // Case 5: resolved mix mode is surfaced in the UI near the device grid
    it('settings.captureMode mix + loopback device → shows the mix recording hint', async () => {
      setupWithSettings('mix', 'loopback');
      expect(
        await screen.findByText(
          'Modo Mezcla: se grabará el audio del sistema + tu micrófono predeterminado.'
        )
      ).toBeInTheDocument();
      expect(
        screen.queryByText('Modo Mezcla ignorado: se grabará sólo el micrófono seleccionado.')
      ).not.toBeInTheDocument();
    });

    // Case 6: mix preference overridden by an input device → the override is surfaced
    it('settings.captureMode mix + input device → shows the mix override hint', async () => {
      setupWithSettings('mix', 'input');
      expect(
        await screen.findByText('Modo Mezcla ignorado: se grabará sólo el micrófono seleccionado.')
      ).toBeInTheDocument();
      expect(
        screen.queryByText(
          'Modo Mezcla: se grabará el audio del sistema + tu micrófono predeterminado.'
        )
      ).not.toBeInTheDocument();
    });

    // Case 7: non-mix settings → no mode hint at all
    it('settings.captureMode system + loopback device → shows no mix hint', async () => {
      const invokeMock = setupWithSettings('system', 'loopback');
      // Settle: device auto-selected (preview fired) — settings resolve in the same mock pass.
      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith('start_preview', expect.anything())
      );
      expect(screen.queryByText(/Modo Mezcla/)).not.toBeInTheDocument();
    });
  });
});
