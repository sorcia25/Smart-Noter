import { store } from '@/store';
import { baseApi } from '@/store/api/base';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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

  describe('Mix as a first-class device card', () => {
    beforeEach(() => {
      // Reset RTK Query cache so each test gets a fresh fetch from the mocked invoke.
      store.dispatch(baseApi.util.resetApiState());
    });

    // Helper: override get_settings to return a specific captureMode value and
    // list both a loopback and an input device (so the mic picker has an option
    // to select). Returns the invokeMock for further assertions.
    function setupWithSettings(settingsCaptureMode: string) {
      const invokeMock = vi.mocked(tauriCore.invoke);
      invokeMock.mockClear();
      invokeMock.mockImplementation(async (cmd: string) => {
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
            {
              id: 'd-I-test',
              name: 'Test Mic',
              kind: 'input',
              sampleRate: 48000,
              channels: 1,
              isDefault: true,
              recommended: false,
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

    // Case 1: the mix card renders first in the device grid.
    it('renders a "Sistema + Micrófono" card first in the device grid', async () => {
      setupWithSettings('system');
      const cards = await screen.findAllByRole('button', {
        name: /Sistema \+ Micrófono|Test Speakers|Test Mic/,
      });
      expect(cards[0]).toHaveTextContent('Sistema + Micrófono');
    });

    // Case 2: selecting the mix card reveals the mic picker + the headphones hint,
    // and starting without picking a mic navigates with micDeviceId: null.
    it('selecting the mix card shows the mic picker and headphones hint; start navigates mix/null mic', async () => {
      setupWithSettings('system');
      const mixCard = await screen.findByRole('button', { name: /Sistema \+ Micrófono/ });
      // The mix card is disabled until list_audio_devices resolves (needs a loopback
      // device) — wait for that before clicking, or the click is a silent no-op.
      await waitFor(() => expect(mixCard).toBeEnabled());
      await userEvent.click(mixCard);

      expect(await screen.findByLabelText('Micrófono de la mezcla')).toBeInTheDocument();
      expect(screen.getByText(/audífonos/i)).toBeInTheDocument();

      const startBtn = await screen.findByRole('button', { name: /Iniciar|Start/i });
      await userEvent.click(startBtn);
      expect(navigateSpy).toHaveBeenCalledOnce();
      const [, navOptions] = navigateSpy.mock.calls[0] as [
        string,
        { state: Record<string, unknown> },
      ];
      expect(navOptions.state.captureMode).toBe('mix');
      expect(navOptions.state.micDeviceId).toBeNull();
    });

    // Case 3: choosing an input device in the mic picker threads that id through to start.
    it('choosing an input device in the mic picker → start navigates with that micDeviceId', async () => {
      setupWithSettings('system');
      const mixCard = await screen.findByRole('button', { name: /Sistema \+ Micrófono/ });
      // The mix card is disabled until list_audio_devices resolves (needs a loopback
      // device) — wait for that before clicking, or the click is a silent no-op.
      await waitFor(() => expect(mixCard).toBeEnabled());
      await userEvent.click(mixCard);

      const micSelect = await screen.findByLabelText('Micrófono de la mezcla');
      fireEvent.change(micSelect, { target: { value: 'd-I-test' } });

      const startBtn = await screen.findByRole('button', { name: /Iniciar|Start/i });
      await userEvent.click(startBtn);
      expect(navigateSpy).toHaveBeenCalledOnce();
      const [, navOptions] = navigateSpy.mock.calls[0] as [
        string,
        { state: Record<string, unknown> },
      ];
      expect(navOptions.state.micDeviceId).toBe('d-I-test');
    });

    // Case 4: selecting a plain input device card (not the mix card) is mic-only.
    it('selecting a plain input device card → start navigates captureMode mic, micDeviceId null', async () => {
      setupWithSettings('system');
      const micCard = await screen.findByRole('button', { name: /Test Mic/ });
      await userEvent.click(micCard);

      const startBtn = await screen.findByRole('button', { name: /Iniciar|Start/i });
      await userEvent.click(startBtn);
      expect(navigateSpy).toHaveBeenCalledOnce();
      const [, navOptions] = navigateSpy.mock.calls[0] as [
        string,
        { state: Record<string, unknown> },
      ];
      expect(navOptions.state.captureMode).toBe('mic');
      expect(navOptions.state.micDeviceId).toBeNull();
    });

    // Case 5: settings.captureMode 'mix' preselects the card — starting without any click
    // still navigates 'mix'.
    it('settings captureMode mix preselects the card; start navigates mix without clicking it', async () => {
      setupWithSettings('mix');
      await screen.findByLabelText('Micrófono de la mezcla');

      const startBtn = await screen.findByRole('button', { name: /Iniciar|Start/i });
      await userEvent.click(startBtn);
      expect(navigateSpy).toHaveBeenCalledOnce();
      const [, navOptions] = navigateSpy.mock.calls[0] as [
        string,
        { state: Record<string, unknown> },
      ];
      expect(navOptions.state.captureMode).toBe('mix');
    });

    // Case 6: with the mix card selected, start_preview previews the loopback lane only.
    it('with the mix card selected, start_preview is invoked with the loopback device id', async () => {
      const invokeMock = setupWithSettings('mix');
      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith('start_preview', {
          deviceId: 'd-L-test',
          captureMode: 'system',
        })
      );
    });

    // v1.0.1 F2: the preview effect must not restart on re-renders that don't change
    // previewDeviceId/previewMode (e.g. a background store update re-rendering the
    // page while `t` churns identity). Re-running would fire an un-awaited
    // stop_preview + an immediate start_preview, racing the backend into
    // AlreadyRecording. Clicking the already-selected device re-renders the page
    // without changing device/mode — start_preview must stay called exactly once.
    it('preview_does_not_restart_on_unrelated_rerenders', async () => {
      const invokeMock = setupWithSettings('mix');
      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith('start_preview', {
          deviceId: 'd-L-test',
          captureMode: 'system',
        })
      );
      const startPreviewCallsBefore = invokeMock.mock.calls.filter(
        ([cmd]) => cmd === 'start_preview'
      );
      expect(startPreviewCallsBefore).toHaveLength(1);

      // Re-click the already-selected mix card: same deviceId ('__mix__') and same
      // previewMode, so previewDeviceId/previewMode are referentially unchanged,
      // but the component re-renders (setDeviceId to its current value).
      const mixCard = await screen.findByRole('button', { name: /Sistema \+ Micrófono/ });
      await userEvent.click(mixCard);

      // invoke() is async (mocked), so give any (incorrect) effect re-run a real
      // macrotask to fire and resolve before asserting — a plain synchronous check
      // right after the click could pass even if a spurious start_preview is
      // still in flight.
      await new Promise((resolve) => setTimeout(resolve, 0));
      const startPreviewCallsAfter = invokeMock.mock.calls.filter(
        ([cmd]) => cmd === 'start_preview'
      );
      expect(startPreviewCallsAfter).toHaveLength(1);
    });
  });
});
