import { store } from '@/store';
import { baseApi } from '@/store/api/base';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StrictMode } from 'react';
import { Provider } from 'react-redux';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import * as tauriCore from '@tauri-apps/api/core';
import LiveRecordingPage from './LiveRecordingPage';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_audio_devices') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
    if (cmd === 'start_recording')
      return { sessionId: 'sess-test', sampleRate: 48000, channels: 2 };
    if (cmd === 'pause_recording') return null;
    if (cmd === 'resume_recording') return null;
    if (cmd === 'stop_recording')
      return { sessionId: 'sess-test', path: 'C:/tmp.wav', bytes: 2048, durationSec: 10 };
    if (cmd === 'discard_recording') return null;
    return null;
  }),
}));

function setup() {
  return render(
    <Provider store={store}>
      <MemoryRouter initialEntries={['/record/live/sess-test']}>
        <Routes>
          <Route path="/record/live/:sessionId" element={<LiveRecordingPage />} />
        </Routes>
      </MemoryRouter>
    </Provider>
  );
}

describe('LiveRecordingPage', () => {
  it('renders the GRABANDO pill', async () => {
    setup();
    await waitFor(() => expect(screen.getByText(/GRABANDO/i)).toBeInTheDocument());
  });

  it('exposes data-screen-label', async () => {
    const { container } = setup();
    await waitFor(() =>
      expect(container.querySelector('[data-screen-label="04 Live recording"]')).toBeTruthy()
    );
  });

  it('survives StrictMode double-mount: starts recording once and does not discard the live session', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    // Re-establish a known mock (earlier tests may have overridden the impl).
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_audio_devices') return [];
      if (cmd === 'list_templates') return [];
      if (cmd === 'get_settings') return null;
      if (cmd === 'start_recording')
        return { sessionId: 'sess-test', sampleRate: 48000, channels: 2 };
      if (cmd === 'stop_recording')
        return { sessionId: 'sess-test', path: 'C:/tmp.wav', bytes: 2048, durationSec: 10 };
      return null;
    });
    // Drain deferred teardown timers left by earlier tests' unmounts before we
    // start counting, so their discard calls aren't attributed to this test.
    await new Promise((r) => setTimeout(r, 5));
    invokeMock.mockClear();
    render(
      <StrictMode>
        <Provider store={store}>
          <MemoryRouter initialEntries={['/record/live/sess-test']}>
            <Routes>
              <Route path="/record/live/:sessionId" element={<LiveRecordingPage />} />
            </Routes>
          </MemoryRouter>
        </Provider>
      </StrictMode>
    );
    const startCalls = () => invokeMock.mock.calls.filter((c) => c[0] === 'start_recording');
    const discardCalls = () => invokeMock.mock.calls.filter((c) => c[0] === 'discard_recording');
    await waitFor(() => expect(startCalls()).toHaveLength(1));
    // Let any deferred teardown macrotask fire; the StrictMode remount must have
    // cancelled it, so the live session is never discarded.
    await new Promise((r) => setTimeout(r, 20));
    expect(startCalls()).toHaveLength(1);
    expect(discardCalls()).toHaveLength(0);
  });

  it('clicking Stop invokes stop_recording and opens the save modal', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    setup();
    await waitFor(() => expect(screen.getByText(/GRABANDO/i)).toBeInTheDocument());
    await userEvent.click(screen.getByRole('button', { name: 'Stop' }));
    expect(invokeMock).toHaveBeenCalledWith('stop_recording');
    await waitFor(() => expect(screen.getByText(/Guardar grabación/i)).toBeInTheDocument());
  });

  describe('Fuente meta block (capture mode surfacing)', () => {
    // Helper: mount with a known loopback device and a navState capture mode.
    // Resets the RTK Query cache so list_audio_devices re-fetches from this mock.
    function setupWithCaptureMode(captureMode: string) {
      store.dispatch(baseApi.util.resetApiState());
      const invokeMock = vi.mocked(tauriCore.invoke);
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
          ];
        }
        if (cmd === 'list_templates') return [];
        if (cmd === 'get_settings') return null;
        if (cmd === 'start_recording')
          return { sessionId: 'sess-test', sampleRate: 48000, channels: 2 };
        if (cmd === 'stop_recording')
          return { sessionId: 'sess-test', path: 'C:/tmp.wav', bytes: 2048, durationSec: 10 };
        if (cmd === 'discard_recording') return null;
        return null;
      });
      return render(
        <Provider store={store}>
          <MemoryRouter
            initialEntries={[
              {
                pathname: '/record/live/sess-test',
                state: { deviceId: 'd-L-test', captureMode },
              },
            ]}
          >
            <Routes>
              <Route path="/record/live/:sessionId" element={<LiveRecordingPage />} />
            </Routes>
          </MemoryRouter>
        </Provider>
      );
    }

    it('captureMode mix → source shows device name + micrófono', async () => {
      setupWithCaptureMode('mix');
      await waitFor(() =>
        expect(screen.getByText('Test Speakers + micrófono')).toBeInTheDocument()
      );
    });

    it('captureMode system → source shows device name without mic suffix', async () => {
      setupWithCaptureMode('system');
      await waitFor(() => expect(screen.getByText('Test Speakers')).toBeInTheDocument());
      expect(screen.queryByText('Test Speakers + micrófono')).not.toBeInTheDocument();
    });
  });
});
