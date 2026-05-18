import { store } from '@/store';
import { render, screen } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import LiveRecordingPage from './LiveRecordingPage';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_audio_devices') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
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
  it('renders the GRABANDO pill', () => {
    setup();
    expect(screen.getByText(/GRABANDO/i)).toBeInTheDocument();
  });

  it('exposes data-screen-label', () => {
    const { container } = setup();
    expect(container.querySelector('[data-screen-label="04 Live recording"]')).toBeTruthy();
  });
});
