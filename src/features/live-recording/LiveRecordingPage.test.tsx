import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
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

  it('clicking Stop invokes stop_recording and opens the save modal', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    setup();
    await waitFor(() => expect(screen.getByText(/GRABANDO/i)).toBeInTheDocument());
    await userEvent.click(screen.getByRole('button', { name: 'Stop' }));
    expect(invokeMock).toHaveBeenCalledWith('stop_recording');
    await waitFor(() => expect(screen.getByText(/Guardar grabación/i)).toBeInTheDocument());
  });
});
