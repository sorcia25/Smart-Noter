import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import LiveRecordingPage from './LiveRecordingPage';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_audio_devices') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
    if (cmd === 'start_recording') return { sessionId: 'sess-test', sampleRate: 48000, channels: 2 };
    if (cmd === 'pause_recording') return null;
    if (cmd === 'resume_recording') return null;
    if (cmd === 'stop_recording') return { sessionId: 'sess-test', path: '', bytes: 0, durationSec: 0 };
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
});
